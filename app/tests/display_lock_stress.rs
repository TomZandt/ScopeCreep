//! The Window thread and the Feed thread share one DisplaySet (spec #196 freeze report): the Window makes, removes,
//! shows and hides Panels and draws them while the Feed thread measures frames. This drives both at once and fails if
//! either stops making progress. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;
use scopecreep::feed_loop::FeedLoop;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn the_window_and_the_feed_thread_keep_going_while_panels_are_made_hidden_drawn_and_removed() {
    let stop = Arc::new(AtomicBool::new(false));
    let feed_steps = Arc::new(AtomicU64::new(0));
    let window_loops = Arc::new(AtomicU64::new(0));
    let settings = Arc::new(Mutex::new(PanelSettings::default()));
    // The Rig holds the GPU turn and cannot cross threads: the Feed thread makes it and hands the rest back.
    let (tx, rx) = std::sync::mpsc::channel();
    let feed_thread = {
        let (stop, steps, settings) = (stop.clone(), feed_steps.clone(), settings.clone());
        std::thread::spawn(move || {
            let mut rig = Rig::new("lock-stress");
            let set = DisplaySet::standard(&rig.device, &rig.queue, TARGET, false);
            let mut feed = FeedLoop::new(
                set.clone(),
                settings,
                OutputConversion::new(&rig.device, &rig.queue),
            );
            tx.send((set, rig.device.clone(), rig.queue.clone()))
                .unwrap();
            let (w, h) = (48u32, 27u32);
            let pixels: Vec<f32> = (0..w * h)
                .flat_map(|i| [i as f32 / 1296.0, 0.4, 0.2, 1.0])
                .collect();
            while !stop.load(Ordering::Relaxed) {
                rig.write(w, h, &pixels);
                for _ in 0..3 {
                    rig.now += common::MS;
                    rig.step(&mut feed);
                    steps.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
    };
    let (set, device, queue) = rx.recv().unwrap();
    let window_thread = {
        let (stop, loops, set, settings) = (
            stop.clone(),
            window_loops.clone(),
            set.clone(),
            settings.clone(),
        );
        std::thread::spawn(move || {
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 256,
                    height: 144,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = target.create_view(&Default::default());
            let mut round = 0usize;
            while !stop.load(Ordering::Relaxed) {
                round += 1;
                // The settings lock is held while Panels are made or removed, as the Window does.
                {
                    let _held = settings.lock().unwrap();
                    set.duplicate(round % 7);
                    if set.len() > 14 {
                        set.remove(set.len() - 1);
                    }
                }
                let shown: Vec<bool> = (0..set.len())
                    .map(|i| !(i + round).is_multiple_of(3))
                    .collect();
                set.set_shown(&shown);
                let mut encoder = device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    for i in 0..set.len() {
                        set.draw(&mut pass, i, [0.0, 0.0, 200.0, 120.0], [256.0, 144.0], 1.0);
                    }
                }
                queue.submit([encoder.finish()]);
                let _ = set.keys();
                let _ = set.generation();
                loops.fetch_add(1, Ordering::Relaxed);
            }
        })
    };

    // Both threads must keep counting for the whole run.
    let mut last = (0, 0);
    for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(500));
        let now = (
            feed_steps.load(Ordering::Relaxed),
            window_loops.load(Ordering::Relaxed),
        );
        assert!(now.0 > last.0, "the Feed thread stopped at {now:?}");
        assert!(now.1 > last.1, "the Window thread stopped at {now:?}");
        last = now;
    }
    stop.store(true, Ordering::Relaxed);
    let deadline = Instant::now() + Duration::from_secs(10);
    for t in [feed_thread, window_thread] {
        while !t.is_finished() {
            assert!(Instant::now() < deadline, "a thread did not stop");
            std::thread::sleep(Duration::from_millis(20));
        }
        t.join().unwrap();
    }
}
