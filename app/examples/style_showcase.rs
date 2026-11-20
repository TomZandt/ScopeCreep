//! The style showcase (#115): draws every component in every state (`scopecreep::showcase`) at 1x and 2x to PNG
//! images outside the repo, for review in one place. Needs a GPU. The drawing is the app's own: the same flat
//! rectangles and glyphon text.
//!
//!   cargo run -p scopecreep --example style_showcase [-- <output folder>]
//!
//! Writes `style-showcase-1x.png` and `style-showcase-2x.png` (default folder: ~/scopecreep-testdata). PNG is made
//! with ImageMagick's `convert`.
use glyphon::{
    Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use scopecreep::components::{Align, Prim};
use scopecreep::flat_rects::FlatRects;
use scopecreep::style;
use std::io::Write;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// FlatRects holds this many rectangles per draw.
const BATCH: usize = 64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join("scopecreep-testdata")
        });
    std::fs::create_dir_all(&folder)?;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("style showcase"),
        ..Default::default()
    }))?;
    let mut font_system = FontSystem::new();
    for scale in [1.0f32, 2.0] {
        let sheet = scopecreep::showcase::showcase(scale);
        let (width, height) = (sheet.size[0].ceil() as u32, sheet.size[1].ceil() as u32);
        let pixels = render(
            &device,
            &queue,
            &mut font_system,
            &sheet.prims,
            width,
            height,
            scale,
        );
        let ppm = folder.join(format!("style-showcase-{}x.ppm", scale as u32));
        let png = folder.join(format!("style-showcase-{}x.png", scale as u32));
        let mut file = std::fs::File::create(&ppm)?;
        write!(file, "P6\n{width} {height}\n255\n")?;
        let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        file.write_all(&rgb)?;
        drop(file);
        let status = std::process::Command::new("convert")
            .arg(&ppm)
            .arg(&png)
            .status()?;
        std::fs::remove_file(&ppm)?;
        if !status.success() {
            return Err("convert (ImageMagick) could not write the PNG".into());
        }
        println!("{} ({width} x {height})", png.display());
    }
    Ok(())
}

/// The prims drawn to a `width` x `height` image, RGBA8 rows top first.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    font_system: &mut FontSystem,
    prims: &[Prim],
    width: u32,
    height: u32,
    scale: f32,
) -> Vec<u8> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("showcase"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let pass_over = |encoder: &mut wgpu::CommandEncoder,
                     load: wgpu::LoadOp<wgpu::Color>,
                     draw: &mut dyn FnMut(&mut wgpu::RenderPass<'_>)| {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("showcase"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        draw(&mut pass);
    };

    // Rectangles, a batch at a time (each batch is written and submitted before the next is set).
    let rects: Vec<([f32; 4], [f32; 4])> = prims
        .iter()
        .filter_map(|p| {
            if let Prim::Rect { rect, colour } = p {
                Some((*rect, *colour))
            } else {
                None
            }
        })
        .collect();
    let mut flat = FlatRects::new(device, queue, FORMAT);
    let mut load = wgpu::LoadOp::Clear(style::clear(style::SURFACE_WINDOW));
    for batch in rects.chunks(BATCH) {
        flat.set([width as f32, height as f32], batch);
        let mut encoder = device.create_command_encoder(&Default::default());
        pass_over(&mut encoder, load, &mut |pass| flat.draw(pass));
        queue.submit([encoder.finish()]);
        load = wgpu::LoadOp::Load;
    }

    // Text, over all of it.
    let cache = Cache::new(device);
    let mut viewport = Viewport::new(device, &cache);
    viewport.update(queue, Resolution { width, height });
    let mut atlas = TextAtlas::with_color_mode(device, queue, &cache, FORMAT, ColorMode::Web);
    let mut renderer =
        TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
    let mut swash = SwashCache::new();
    let mut buffers: Vec<(Buffer, [f32; 4], [u8; 3], f32)> = Vec::new();
    for prim in prims {
        let Prim::Text {
            text,
            rect,
            face,
            colour,
            align,
        } = prim
        else {
            continue;
        };
        let (size, line) = face.metrics();
        let mut buffer = Buffer::new(font_system, Metrics::new(size * scale, size * scale * line));
        buffer.set_size(Some(rect[2].max(1.0)), None);
        buffer.set_wrap(glyphon::cosmic_text::Wrap::None);
        let align = match align {
            Align::Left => None,
            Align::Centre => Some(glyphon::cosmic_text::Align::Center),
            Align::Right => Some(glyphon::cosmic_text::Align::Right),
        };
        buffer.set_text(
            text,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            align,
        );
        buffer.shape_until_scroll(font_system, false);
        buffers.push((buffer, *rect, *colour, size * scale * line));
    }
    let areas: Vec<TextArea> = buffers
        .iter()
        .map(|(buffer, rect, colour, line)| TextArea {
            buffer,
            left: rect[0],
            top: rect[1] + (rect[3] - line) / 2.0,
            scale: 1.0,
            bounds: TextBounds {
                left: rect[0] as i32,
                top: rect[1] as i32,
                right: (rect[0] + rect[2]).ceil() as i32,
                bottom: (rect[1] + rect[3]).ceil() as i32,
            },
            default_color: Color::rgb(colour[0], colour[1], colour[2]),
            custom_glyphs: &[],
        })
        .collect();
    renderer
        .prepare(
            device,
            queue,
            font_system,
            &mut atlas,
            &viewport,
            areas,
            &mut swash,
        )
        .expect("the text prepares");
    let row = (width * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("showcase readback"),
        size: (row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    pass_over(&mut encoder, wgpu::LoadOp::Load, &mut |pass| {
        renderer
            .render(&atlas, &viewport, pass)
            .expect("the text draws")
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let data = readback
        .slice(..)
        .get_mapped_range()
        .expect("the read-back maps");
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        pixels.extend_from_slice(&data[(y * row) as usize..(y * row + width * 4) as usize]);
    }
    pixels
}
