//! Accuracy suite tool (ticket #17): attach to the real Feed as the app does, read the Colour Setup from the Resolve
//! project as the app does, convert each Viewer Frame on the GPU as the app does, and save the frames the Picture
//! View and the waveform would show, so they can be compared with Resolve's own renders
//! (`cargo run -p scopecreep-accuracy -- feed`).
//!
//!   capture_converted --out <dir> --project <name> --frame <clip>=<frame time> [--frame ...] [--timeout <s>]
//!                     [--feed <path>]
//!
//! A frame is saved as `<dir>/<clip>.exr` (RGB float, top row first) with the Picture View's label beside it in
//! `<dir>/<clip>.label`, once the picture on screen is that frame time and the Colour Setup has been read from
//! the named project. Driven by `python3 harness/harness.py feed-frames`, which moves Resolve's playhead. Like the
//! app, it reads the project by script only while the Sender is quiet (ADR 0006).
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::{Monitor, ProjectState};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep_accuracy::imageio::{Image, write_exr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    if let Err(e) = run() {
        eprintln!("capture_converted: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut out: Option<PathBuf> = None;
    let mut project: Option<String> = None;
    let mut wanted: Vec<(String, f64)> = vec![];
    let mut timeout = Duration::from_secs(120);
    let mut feed = scopecreep_feed::default_path();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--out" => out = Some(value()?.into()),
            "--project" => project = Some(value()?),
            "--feed" => feed = value()?.into(),
            "--timeout" => {
                timeout =
                    Duration::from_secs_f64(value()?.parse().map_err(|_| "--timeout is seconds")?)
            }
            "--frame" => {
                let v = value()?;
                let (clip, time) = v.split_once('=').ok_or("--frame is <clip>=<frame time>")?;
                wanted.push((
                    clip.to_string(),
                    time.parse().map_err(|_| format!("bad frame time in {v}"))?,
                ));
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    let out = out.ok_or("--out is required")?;
    let project = project.ok_or("--project is required")?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("capture_converted"),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        ..Default::default()
    }))
    .map_err(|e| format!("no GPU device: {e}"))?;

    let mut monitor = Monitor::with_project_reader(&feed, Box::new(ScriptHelper::from_env()));
    let mut uploader = FrameUploader::new(&device, &queue);
    let mut conversion = OutputConversion::new(&device, &queue);
    let start = Instant::now();
    let mut seen = vec![];
    while !wanted.is_empty() {
        if start.elapsed() > timeout {
            let names: Vec<_> = wanted.iter().map(|(c, t)| format!("{c}={t}")).collect();
            return Err(format!(
                "timed out waiting for {}; frame times seen: {seen:?}; Feed {:?}; project {:?}; {:?}",
                names.join(", "),
                monitor.status().feed,
                monitor.status().project.as_ref().map(|p| p.project.clone()),
                monitor.status().project_state
            ));
        }
        monitor.poll(scopecreep_feed::now_ns(), &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                conversion.convert(encoder, gpu_frame);
            })
        });
        let status = monitor.status();
        let setup = status.colour_setup().map(|c| c.setup);
        conversion.follow(setup.as_ref(), uploader.frame(), |_, _| {});
        std::thread::sleep(Duration::from_millis(1));

        let Some(picture) = &status.picture else {
            continue;
        };
        if !seen.contains(&picture.frame_time) {
            seen.push(picture.frame_time);
        }
        let read = status.project_state == ProjectState::Read
            && status
                .project
                .as_ref()
                .is_some_and(|p| p.project == project);
        let Some(i) = wanted
            .iter()
            .position(|(_, t)| (t - picture.frame_time).abs() < 1e-6)
        else {
            continue;
        };
        let (Some(frame), true) = (uploader.frame(), read) else {
            continue;
        };
        let label = scopecreep::picture_view::PictureView::label_for(
            Some([picture.width, picture.height]),
            &status.display_context(),
        );
        let rgb = conversion.read_shown(frame);
        let (clip, _) = wanted.remove(i);
        let path = out.join(format!("{clip}.exr"));
        write_exr(
            &path,
            &Image {
                width: frame.width as usize,
                height: frame.height as usize,
                rgb,
            },
        )?;
        std::fs::write(out.join(format!("{clip}.label")), format!("{label}\n"))
            .map_err(|e| e.to_string())?;
        println!(
            "captured {clip} t={} {}x{}: {label}",
            picture.frame_time, frame.width, frame.height
        );
    }
    Ok(())
}
