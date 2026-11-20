//! The synthetic frame writer as a command: plays a moving test card into the Feed the way the Sender would, so
//! the app can be run and looked at with no Resolve.
//!
//!   feed-synth [--size 1920x1080] [--fps 24] [--frames N] [--path /dev/shm/scopecreep-feed]
//!              [--instances N] [--host NAME] [--out-of-range]
//!
//! Like the Sender, it publishes a Feed with no ring, does nothing while no app is attached, and makes the ring
//! the first time an app is listening. `--instances 2` plays two Sender instances rendering every frame in step
//! (two Senders on one timeline). Started while another feed-synth (or Resolve) owns the Feed, it joins as a
//! guest, as a second Resolve would: its renders are registered but no frames are sent. `--out-of-range` makes
//! the bottom twelfth of the card a grey ramp from -0.25 to 1.25, so the left and right sixths of the columns are
//! below 0 and above 1; on the video levels the app assumes without a Colour Setup, the waveform's edge rows
//! light over about the left eighth and the right tenth.
use scopecreep_feed::{
    FeedWriter, FrameMeta, SenderIdentity, WriteError, default_path, now_ns, synthetic,
};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut width, mut height, mut fps, mut frames, mut path) =
        (1920u32, 1080u32, 24.0f64, u64::MAX, default_path());
    let mut instances = 1u32;
    let mut out_of_range = false;
    let mut identity = SenderIdentity::default();
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
            "--instances" => instances = value()?.parse::<u32>()?.max(1),
            "--host" => identity.host = value()?,
            "--out-of-range" => out_of_range = true,
            _ => {
                return Err(format!("unknown argument {arg}; see the top of feed-synth.rs").into());
            }
        }
    }
    let slot_bytes = width as usize * height as usize * 16;
    let mut writer = FeedWriter::start(&path, identity)?;
    for i in 1..=instances {
        writer.register(i, fps);
    }
    let role = if writer.is_guest() {
        "guest in another Sender's Feed: registering renders, sending nothing"
    } else {
        "waiting for an app"
    };
    println!(
        "synthetic Sender on {}: {width}x{height} at {fps} fps, {instances} instance(s){} ({role})",
        path.display(),
        if out_of_range {
            ", values out of range along the bottom"
        } else {
            ""
        }
    );
    let start = Instant::now();
    let (mut frame, mut written, mut idle) = (0u64, 0u64, 0u64);
    let mut had_reader = false;
    let mut next_refresh = Instant::now();
    let mut card = if out_of_range {
        synthetic::test_card_out_of_range(width, height, 0)
    } else {
        synthetic::test_card(width, height, 0)
    };
    while frame < frames {
        if Instant::now() >= next_refresh {
            next_refresh += Duration::from_secs(1);
            let was_guest = writer.is_guest();
            writer.refresh();
            if was_guest && !writer.is_guest() {
                println!("the Feed's owner has gone: this Sender now owns the Feed");
            }
        }
        let render_ns = now_ns();
        let present = writer.reader_present(render_ns);
        if present != had_reader {
            println!(
                "{}",
                if present {
                    "app attached: sending frames"
                } else {
                    "app gone: doing nothing"
                }
            );
            had_reader = present;
        }
        for instance in 1..=instances {
            let meta = FrameMeta {
                width,
                height,
                frame_time: 86400.0 + frame as f64,
                frame_rate: fps,
                render_ns: now_ns(),
                sender_instance: instance,
            };
            if !present {
                writer.note_render(instance, meta.frame_time, meta.render_ns);
                idle += 1;
                continue;
            }
            synthetic::move_test_card_marker(
                &mut card,
                width,
                height,
                frame + 7 * (instance as u64 - 1),
            );
            match writer.write_frame(&meta, &card) {
                Ok(_) => written += 1,
                Err(WriteError::TooLarge) => writer.replace(4, slot_bytes)?, // first frame: make the ring
                Err(WriteError::Dropped) | Err(WriteError::Guest) => {}
            }
        }
        frame += 1;
        let due = start + Duration::from_secs_f64(frame as f64 / fps);
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
    }
    println!("{written} frames written, {idle} idle");
    Ok(())
}
