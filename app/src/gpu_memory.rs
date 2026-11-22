//! Running short of GPU memory (#35). Resolve can hold nearly all of an 8 GB card while it plays UHD
//! (docs/findings/12-uhd-copy-cost.md), so the app's own large allocations (the frame textures and the waveform's
//! buffers) may fail. Each is made through [`allocate`], which catches the failure with a wgpu error scope instead
//! of letting it through as an invalid resource. The caller leaves the work out for that frame and tries again
//! with the next one; the Feed loop collects what could not be had with [`take`] and says so in the notice.
//!
//! Like a café that runs out of cups: it tells the queue it is out and how many it needs, rather than pouring onto
//! the counter, and serves again as soon as cups come back.
//!
//! Error scopes are per thread, and so is the record kept here: the Feed thread does all the allocating it covers.
use std::cell::Cell;

/// What the memory was for, as the notice names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The frame on the GPU and its converted copy (the Picture View and every Scope read them).
    Picture,
    /// A Scope's own buffers.
    Scopes,
}

/// Bytes that could not be had since the last [`take`], by [`Part`]; 0 = none short.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shortfall {
    pub picture: u64,
    pub scopes: u64,
}

impl Shortfall {
    pub fn is_none(&self) -> bool {
        self.picture == 0 && self.scopes == 0
    }

    /// One sentence for the Window, or nothing when nothing was short.
    pub fn notice(&self) -> Option<String> {
        let what = match (self.picture != 0, self.scopes != 0) {
            (false, false) => return None,
            (true, false) => "the picture",
            (false, true) => "the scopes",
            (true, true) => "the picture and the scopes",
        };
        let mb = (self.picture + self.scopes).div_ceil(1_000_000);
        Some(format!(
            "Not enough GPU memory for {what} (needs about {mb} MB). Close another GPU app; ScopeCreep will carry on when memory frees up."
        ))
    }
}

/// The environment variable that caps how big one allocation may be, in MB (decimal), so the notice can be seen
/// without filling the GPU: an allocation over the cap fails as if the GPU were full. For tests and checks only.
pub const LIMIT_ENV: &str = "SCOPECREEP_GPU_MEMORY_LIMIT_MB";

thread_local! {
    static SHORT: Cell<Shortfall> = Cell::new(Shortfall::default());
    static LIMIT: Cell<Option<u64>> = Cell::new(
        std::env::var(LIMIT_ENV)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|mb| mb * 1_000_000),
    );
}

/// Cap one allocation on this thread at `bytes` (None: no cap). The test hook; see [`LIMIT_ENV`].
pub fn set_limit(bytes: Option<u64>) {
    LIMIT.with(|l| l.set(bytes));
}

/// Make a GPU resource of about `bytes` for `part` with `make`. None if the GPU did not have the memory (or the
/// cap says so): the shortfall is recorded for [`take`] and nothing invalid is handed back.
pub fn allocate<T>(
    device: &wgpu::Device,
    part: Part,
    bytes: u64,
    make: impl FnOnce() -> T,
) -> Option<T> {
    let capped = LIMIT.with(Cell::get).is_some_and(|limit| bytes > limit);
    let made = if capped {
        None
    } else {
        let scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let made = make();
        // Native wgpu reports the scope's error at once; the future is ready when polled.
        match pollster::block_on(scope.pop()) {
            None => Some(made),
            Some(_) => None,
        }
    };
    if made.is_none() {
        SHORT.with(|s| {
            let mut short = s.get();
            match part {
                Part::Picture => short.picture += bytes,
                Part::Scopes => short.scopes += bytes,
            }
            s.set(short);
        });
    }
    made
}

/// What could not be had on this thread since the last call, and start again.
pub fn take() -> Shortfall {
    SHORT.with(|s| s.replace(Shortfall::default()))
}
