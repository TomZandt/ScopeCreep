//! The Feed: the shared-memory ring of frames between the Sender (inside Resolve) and the scopecreep app.
//!
//! The protocol is specified in `feed/PROTOCOL.md`. This crate is its Rust implementation: [`FeedReader`] is what
//! the app uses, [`FeedWriter`] is the synthetic frame writer that stands in for Resolve and the Sender in tests.
//! The Sender itself uses the C implementation in `feed/include/scopecreep_feed.h`.
mod colour_setup;
mod layout;
mod platform;
mod reader;
pub mod synthetic;
mod writer;

pub use colour_setup::{Provenance, SentColourSetup};
pub use platform::{now_ns, process_alive, remove as remove_feed};
pub use reader::{FeedReader, FeedStats, Frame, OpenError, RegisteredSender, SenderInfo};
pub use writer::{FeedWriter, FrameMeta, SenderIdentity, WriteError};

use std::path::PathBuf;

/// The protocol version this crate speaks.
pub const VERSION_MAJOR: u32 = 1;
/// 1.1: the slot's Colour Setup block has a defined format (ticket #16).
/// 1.2: the Sender registry in the header, and guest Senders (ticket #19).
pub const VERSION_MINOR: u32 = 2;

/// Entries in the Sender registry of a version 1.2 Feed header.
pub const REGISTRY_ENTRIES: usize = 24;

/// A reader whose heartbeat is older than this is gone.
pub const READER_TIMEOUT_NS: u64 = 1_000_000_000;
/// How often a reader must renew its heartbeat.
pub const HEARTBEAT_INTERVAL_NS: u64 = 250_000_000;

pub const SENDER_NAME: &str = "ScopeCreep";

/// Where the Feed lives: `SCOPECREEP_FEED` if set, else the platform's location (on Linux
/// `/dev/shm/scopecreep-feed`, on macOS the shared-memory object `/scopecreep-feed`; see `feed/PROTOCOL.md`).
pub fn default_path() -> PathBuf {
    match std::env::var_os("SCOPECREEP_FEED") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(platform::DEFAULT_LOCATION),
    }
}
