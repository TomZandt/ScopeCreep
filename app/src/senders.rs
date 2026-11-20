//! Which Sender drives the Feed when more than one is active.
//!
//! A Resolve process can hold many Sender instances: one per timeline that has the Sender, and more if the user
//! puts it on several nodes. Only one may drive the Scopes. The rule (feed/PROTOCOL.md, "More than one Sender"):
//!
//! - **One after another** (a timeline switch: one Sender stops rendering, another starts): the Feed follows the
//!   Sender that rendered last, at once. That is not a conflict and nothing is flagged.
//! - **In step** (two Senders rendering the same frames, e.g. one on a clip node and one on the timeline node):
//!   the Sender created first in its Resolve process (the lowest instance number) drives, whichever of the two
//!   renders first. The other is flagged.
//!
//! Two Senders are in step when each of their last two renders came within half a frame period of the other's,
//! like two people walking through a door side by side twice in a row, rather than one after the other. Once in
//! step they stay so while both keep rendering, so the choice cannot flicker from frame to frame.
//! A Sender whose frame arrives within half a period of the driver's, before the two have been seen in step
//! (a second Sender's very first frame), does not take the Feed over: the next frame settles it.
//!
//! Senders in another Resolve process never drive: they are guests in the Feed's registry and send no frames
//! (feed/PROTOCOL.md). The monitor flags them from the registry while they render.

/// Two Senders found in step stay so for this long after they were last seen in step.
pub const IN_STEP_HOLD_NS: u64 = 1_000_000_000;
/// A Sender found in step with the driver stays flagged this long, so the notice does not blink.
pub const FLAG_HOLD_NS: u64 = 2_000_000_000;

/// One Sender instance, as far as the Feed can tell: the process it runs in and its number there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SenderKey {
    pub pid: u64,
    pub instance: u32,
}

struct Seen {
    key: SenderKey,
    prev_ns: Option<u64>,
    last_ns: u64,
    frame_rate: f64,
    in_step_until: u64,
    flagged_until: u64,
}

/// What the arbiter decided about one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    /// The frame comes from the Sender driving the Feed.
    pub drives: bool,
    /// The driving Sender changed with this frame: forget the pacing history of the previous one.
    pub driver_changed: bool,
}

#[derive(Default)]
pub struct Arbiter {
    seen: Vec<Seen>,
    driver: Option<SenderKey>,
}

impl Arbiter {
    pub fn reset(&mut self) {
        self.seen.clear();
        self.driver = None;
    }

    pub fn driver(&self) -> Option<SenderKey> {
        self.driver
    }

    /// The timeline rate the Sender last reported.
    pub fn frame_rate(&self, key: SenderKey) -> Option<f64> {
        self.seen
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.frame_rate)
    }

    /// A frame from `key`, whose render call began at `render_ns`.
    pub fn frame(&mut self, key: SenderKey, render_ns: u64, frame_rate: f64) -> Decision {
        let idx = match self.seen.iter().position(|s| s.key == key) {
            Some(i) => {
                let s = &mut self.seen[i];
                s.prev_ns = Some(s.last_ns);
                s.last_ns = render_ns;
                s.frame_rate = frame_rate;
                i
            }
            None => {
                self.seen.push(Seen {
                    key,
                    prev_ns: None,
                    last_ns: render_ns,
                    frame_rate,
                    in_step_until: 0,
                    flagged_until: 0,
                });
                self.seen.len() - 1
            }
        };
        let period = (1e9 / if frame_rate >= 1.0 { frame_rate } else { 24.0 }) as u64;
        let near = |a: u64, b: u64| a.abs_diff(b) <= period / 2;

        // Is this Sender in step with any other?
        let (prev, last) = (self.seen[idx].prev_ns, render_ns);
        let mut in_step = false;
        for s in self.seen.iter_mut().filter(|s| s.key != key) {
            if let (Some(p), Some(sp)) = (prev, s.prev_ns)
                && near(last, s.last_ns)
                && near(p, sp)
            {
                s.in_step_until = render_ns + IN_STEP_HOLD_NS;
                in_step = true;
            }
        }
        if in_step {
            self.seen[idx].in_step_until = render_ns + IN_STEP_HOLD_NS;
        }

        // The driver: this Sender, unless it is in step with others still rendering; then the lowest of them.
        let driver = if self.seen[idx].in_step_until > render_ns {
            let group: Vec<SenderKey> = self
                .seen
                .iter()
                .filter(|s| {
                    s.key == key
                        || (s.in_step_until > render_ns
                            && render_ns.saturating_sub(s.last_ns) <= 2 * period)
                })
                .map(|s| s.key)
                .collect();
            let driver = *group.iter().min().unwrap();
            for s in self
                .seen
                .iter_mut()
                .filter(|s| group.contains(&s.key) && s.key != driver)
            {
                s.flagged_until = render_ns + FLAG_HOLD_NS;
            }
            driver
        } else if let Some(d) = self.driver
            && d != key
            && self
                .seen
                .iter()
                .any(|s| s.key == d && near(render_ns, s.last_ns))
        {
            // Rendering the same moment as the driver, but not yet seen in step (a second Sender's first frame):
            // the driver keeps the Feed until the next frame settles it.
            d
        } else {
            key
        };
        if let Some(s) = self.seen.iter_mut().find(|s| s.key == driver) {
            s.flagged_until = 0;
        }
        let driver_changed = self.driver != Some(driver);
        self.driver = Some(driver);
        Decision {
            drives: key == driver,
            driver_changed,
        }
    }

    /// Senders flagged at `now_ns` (rendering in step with the driver recently), lowest first.
    pub fn flagged(&self, now_ns: u64) -> Vec<SenderKey> {
        let mut out: Vec<SenderKey> = self
            .seen
            .iter()
            .filter(|s| s.flagged_until > now_ns && Some(s.key) != self.driver)
            .map(|s| s.key)
            .collect();
        out.sort();
        out
    }
}
