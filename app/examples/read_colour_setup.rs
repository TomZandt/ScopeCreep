//! Development tool: read the current Resolve project once, through the same helper the app uses, and print what
//! the app makes of it. Only run it while Resolve is not playing (scripting during playback starves Resolve).
//!
//!   cargo run -p scopecreep --example read_colour_setup
use scopecreep::colour_setup::{KnownColourSetup, Provenance};
use scopecreep::project::{Outcome, ProjectReader};
use scopecreep::resolve_helper::ScriptHelper;
use std::time::{Duration, Instant};

fn main() {
    let mut helper = ScriptHelper::from_env();
    let start = Instant::now();
    helper.request();
    let answer = loop {
        if let Some(a) = helper.take_answer() {
            break a;
        }
        if start.elapsed() > Duration::from_secs(10) {
            eprintln!("no answer in 10 s");
            helper.abandon(); // exit() skips Drop: kill the helper first
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let read_ms = answer.iter().find(|(k, _)| k == "read_ms").map_or("?", |(_, v)| v.as_str()).to_string();
    match Outcome::from_answer(&answer) {
        Outcome::Read(r) => {
            let label = r.colour_setup.map(|setup| KnownColourSetup { setup, provenance: Provenance::Project }.label());
            println!(
                "project={} timeline={} product={} studio={} read_ms={} total_ms={:.0}\nlabel={}",
                r.project,
                r.timeline.unwrap_or_default(),
                r.product,
                r.studio,
                read_ms,
                start.elapsed().as_secs_f64() * 1000.0,
                label.unwrap_or_else(|| "Colour Setup unknown".into())
            );
        }
        Outcome::Gone(why) | Outcome::Failed(why) => println!("problem={why}"),
    }
}
