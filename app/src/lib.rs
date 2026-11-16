//! The scopecreep app. The window and drawing live in the binary (`main.rs`, `picture_view.rs`); this library is
//! the part with behaviour worth testing: what the app makes of the Feed.
pub mod colour_setup;
pub mod monitor;
pub mod output_conversion;
pub mod playback_rate;
pub mod senders;
pub mod project;
pub mod resolve_helper;
pub mod viewer_gate;
pub mod gpu_frame;
pub mod layout;
pub mod waveform;
