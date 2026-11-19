//! Development tool: attach to the live Feed as the app does (the Colour Setup read through the scripting helper,
//! unless `--no-scripting`), measure the frames Resolve renders with the app's own waveform, and print, for a few
//! picture columns, the value the Sender received at one row and the codes the waveform put that column's pixels
//! on. Used to compare the app's readings with Resolve's own waveform (#18). Only run it while Resolve is not
//! playing, and move the playhead while it listens so that Resolve renders a frame.
//!
//!   cargo run -p scopecreep --example measure_feed -- [--seconds 8] [--row 100] [--columns 0,480,960,1440,1919]
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep::measurement::{LumaWeights, Measurement};
use scopecreep::waveform::Waveform;
use std::time::{Duration, Instant};

fn main() {
    let (mut seconds, mut row, mut columns, mut scripting) = (8.0, 100usize, vec![0u32, 480, 960, 1440, 1919], true);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().expect("a value after each option");
        match arg.as_str() {
            "--seconds" => seconds = value().parse().expect("seconds"),
            "--row" => row = value().parse().expect("row from the top"),
            "--columns" => columns = value().split(',').map(|c| c.parse().expect("column")).collect(),
            "--no-scripting" => scripting = false,
            other => panic!("unknown argument {other}"),
        }
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("a GPU");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("a GPU device");
    let waveform = Waveform::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut uploader = FrameUploader::new(&device, &queue);
    let feed = scopecreep_feed::default_path();
    let mut monitor = if scripting { Monitor::with_project_reader(&feed, Box::new(ScriptHelper::from_env())) } else { Monitor::new(&feed) };

    let start = Instant::now();
    let mut values: Vec<[f32; 3]> = Vec::new();
    let mut frames = 0u64;
    while start.elapsed() < Duration::from_secs_f64(seconds) {
        waveform.set_measurement(Measurement::for_colour_setup(monitor.status().colour_setup(), LumaWeights::FromPrimaries));
        monitor.poll(scopecreep_feed::now_ns(), &mut |frame| {
            let (width, height) = (frame.width() as usize, frame.height() as usize);
            let y = if frame.bottom_up() { height - 1 - row.min(height - 1) } else { row.min(height - 1) };
            let px = frame.pixels();
            values = columns.iter().map(|&x| {
                let i = y * frame.row_floats() + (x as usize).min(width - 1) * 4;
                [px[i], px[i + 1], px[i + 2]]
            }).collect();
            uploader.upload(frame, |encoder, gpu_frame| waveform.measure(encoder, gpu_frame));
            frames += 1;
        });
        std::thread::sleep(Duration::from_millis(1));
    }
    let status = monitor.status();
    println!("frames measured: {frames}");
    println!("colour setup: {:?}", status.colour_setup().map(|c| c.label()));
    println!("waveform label: {}", waveform.label());
    if frames == 0 {
        println!("no frame: was the playhead moved, and is no other app attached to the Feed? ({:?})", status.feed);
        return;
    }
    let counts = waveform.counts();
    for (i, &x) in columns.iter().enumerate() {
        let x = x.min(counts.columns() - 1);
        let mut codes: Vec<(u32, u32)> = (0..1024).map(|c| (counts.at(x, c), c)).filter(|(n, _)| *n > 0).collect();
        codes.sort_by(|a, b| b.cmp(a));
        let codes: Vec<String> = codes.iter().take(4).map(|(n, c)| format!("{c} ({n} px)")).collect();
        println!(
            "column {x}: value at row {row} = {:?}; codes: {}; below 0: {}; above 1023: {}",
            values[i],
            codes.join(", "),
            counts.below(x),
            counts.above(x)
        );
    }
}
