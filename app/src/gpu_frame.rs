//! Putting each Feed frame on the GPU once, as it is (32-bit float RGBA, full resolution), for every display to
//! read: the Picture View draws it and the Scopes measure it.
//!
//! A UHD float frame is 133 MB. The frame goes through one of two staging buffers that stay mapped between
//! frames, so the only CPU work per frame is one copy out of the Feed. At most two frames are on their way to
//! the GPU: if the GPU is further behind than that, [`FrameUploader::upload`] waits, the app stops taking frames
//! from the Feed for that moment, and the Feed drops (and counts) frames instead. The Sender never waits.
//!
//! When the GPU has no memory for the staging buffers or the frame texture, the frame is left out and the next one
//! tries again ([`crate::gpu_memory`]).
use crate::gpu_memory::{self, Part};
use scopecreep_feed::Frame;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const STAGING_BUFFERS: usize = 2;

/// The latest frame on the GPU.
#[derive(Clone, Debug)]
pub struct GpuFrame {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    /// The texture's first row is the bottom of the picture (as the Sender writes it).
    pub bottom_up: bool,
    /// Changes whenever a new texture is made (the frame size changed), so displays know to bind it again.
    /// Unique across every [`GpuFrame`] the app makes (see [`new_texture_id`]).
    pub texture_id: u64,
}

/// A `texture_id` no other [`GpuFrame`] has had.
pub fn new_texture_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

struct Staging {
    buffer: wgpu::Buffer,
    /// Mapped and ready to be written.
    ready: Arc<AtomicBool>,
    /// The submission that last read it.
    submission: Option<wgpu::SubmissionIndex>,
}

pub struct FrameUploader {
    device: wgpu::Device,
    queue: wgpu::Queue,
    staging: Vec<Staging>,
    next: usize,
    frame: Option<GpuFrame>,
}

impl FrameUploader {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> FrameUploader {
        FrameUploader {
            device: device.clone(),
            queue: queue.clone(),
            staging: Vec::new(),
            next: 0,
            frame: None,
        }
    }

    /// The latest frame put on the GPU, if any.
    pub fn frame(&self) -> Option<&GpuFrame> {
        self.frame.as_ref()
    }

    /// Put `frame` on the GPU. `then` records the work that reads it (the Scopes) into the same submission.
    pub fn upload(
        &mut self,
        frame: &Frame,
        then: impl FnOnce(&mut wgpu::CommandEncoder, &GpuFrame),
    ) {
        let (width, height) = (frame.width(), frame.height());
        if width == 0 || height == 0 {
            return;
        }
        // Rows in the staging buffer are padded to what a buffer-to-texture copy needs.
        let row_bytes = width as usize * 16;
        let padded_row = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        let size = (padded_row * height as usize) as u64;
        if self.staging.first().is_none_or(|s| s.buffer.size() != size) {
            // The old buffers go first: their memory may be what the new ones need.
            self.staging.clear();
            let device = &self.device;
            let made =
                gpu_memory::allocate(device, Part::Picture, size * STAGING_BUFFERS as u64, || {
                    (0..STAGING_BUFFERS)
                        .map(|_| Staging {
                            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("feed frame staging"),
                                size,
                                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                                mapped_at_creation: true,
                            }),
                            ready: Arc::new(AtomicBool::new(true)),
                            submission: None,
                        })
                        .collect()
                });
            let Some(staging) = made else {
                return; // short of GPU memory: this frame is left out, the next one tries again
            };
            self.staging = staging;
            self.next = 0;
        }
        if self
            .frame
            .as_ref()
            .is_none_or(|f| f.width != width || f.height != height)
        {
            self.frame = None;
            let device = &self.device;
            let bytes = width as u64 * height as u64 * 16;
            let made = gpu_memory::allocate(device, Part::Picture, bytes, || {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("feed frame"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            });
            let Some(texture) = made else {
                return;
            };
            self.frame = Some(GpuFrame {
                texture,
                width,
                height,
                bottom_up: frame.bottom_up(),
                texture_id: new_texture_id(),
            });
        }
        let gpu_frame = self.frame.as_mut().unwrap();
        gpu_frame.bottom_up = frame.bottom_up();

        let staging = &mut self.staging[self.next];
        self.next = (self.next + 1) % STAGING_BUFFERS;
        // Two frames already on their way: wait for the older one (bounded; see the module notes).
        for _ in 0..3 {
            if staging.ready.load(Ordering::Acquire) {
                break;
            }
            let wait = wgpu::PollType::Wait {
                submission_index: staging.submission.clone(),
                timeout: Some(std::time::Duration::from_secs(1)),
            };
            let _ = self.device.poll(wait);
        }
        if !staging.ready.load(Ordering::Acquire) {
            return; // the GPU is lost or hung: show nothing new rather than hold the Feed up
        }
        {
            let mut mapped = staging
                .buffer
                .get_mapped_range_mut(..)
                .expect("staging buffer is mapped");
            let pixels: &[u8] = bytemuck::cast_slice(frame.pixels());
            let source_row = frame.row_floats() * 4;
            if source_row == padded_row {
                mapped.copy_from_slice(&pixels[..size as usize]);
            } else {
                for row in 0..height as usize {
                    let to = row * padded_row;
                    mapped
                        .slice(to..to + row_bytes)
                        .copy_from_slice(&pixels[row * source_row..row * source_row + row_bytes]);
                }
            }
        }
        staging.buffer.unmap();
        staging.ready.store(false, Ordering::Release);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("feed frame"),
            });
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &staging.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &gpu_frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        then(&mut encoder, gpu_frame);
        staging.submission = Some(self.queue.submit([encoder.finish()]));
        let ready = staging.ready.clone();
        staging
            .buffer
            .map_async(wgpu::MapMode::Write, .., move |result| {
                ready.store(result.is_ok(), Ordering::Release)
            });
        // Let finished work report back (and the mapping complete) without waiting.
        let _ = self.device.poll(wgpu::PollType::Poll);
    }
}
