//! The scopecreep app. The window and drawing live in the binary (`main.rs`, `picture_view.rs`); this library is
//! the part with behaviour worth testing: what the app makes of the Feed.
pub mod monitor;
pub mod viewer_gate;
pub mod gpu_frame;
pub mod waveform;
