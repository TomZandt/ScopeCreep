//! The synthetic frame writer as a command: plays a moving test card into the Feed the way the Sender would, so
//! the app can be run and looked at with no Resolve.
//!
//!   feed-synth [--size 1920x1080] [--fps 24] [--frames N] [--path /dev/shm/scopecreep-feed]
//!
//! Like the Sender, it publishes a Feed with no ring, does nothing while no app is attached, and makes the ring
//! the first time an app is listening.
use scopecreep_feed::{FeedWriter, FrameMeta, WriteError, default_path, now_ns, synthetic};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut width, mut height, mut fps, mut frames, mut path) = (1920u32, 1080u32, 24.0f64, u64::MAX, default_path());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                (width, height) = (w.parse()?, h.parse()?);
            }
            "--fps" => fps = value()?.parse()?,
            "--frames" => frames = value()?.parse()?,
            "--path" => path = value()?.into(),
            _ => return Err(format!("unknown argument {arg}; see the top of feed-synth.rs").into()),
        }
    }
    let slot_bytes = width as usize * height as usize * 16;
    let mut writer = FeedWriter::create(&path, 0, 0)?;
    println!("synthetic Sender on {}: {width}x{height} at {fps} fps (waiting for an app)", path.display());
    let start = Instant::now();
    let (mut frame, mut written, mut idle) = (0u64, 0u64, 0u64);
    let mut had_reader = false;
    let mut card = synthetic::test_card(width, height, 0);
    while frame < frames {
        let render_ns = now_ns();
        let present = writer.reader_present(render_ns);
        if present != had_reader {
            println!("{}", if present { "app attached: sending frames" } else { "app gone: doing nothing" });
            had_reader = present;
        }
        if !present {
            idle += 1;
        } else {
            let meta = FrameMeta { width, height, frame_time: 86400.0 + frame as f64, frame_rate: fps, render_ns, sender_instance: 1 };
            synthetic::move_test_card_marker(&mut card, width, height, frame);
            match writer.write_frame(&meta, &card) {
                Ok(_) => written += 1,
                Err(WriteError::TooLarge) => writer = FeedWriter::create(&path, 4, slot_bytes)?, // first frame: make the ring
                Err(WriteError::Dropped) => {}
            }
        }
        frame += 1;
        let due = start + Duration::from_secs_f64(frame as f64 / fps);
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
    }
    println!("{written} frames written, {idle} idle");
    Ok(())
}
