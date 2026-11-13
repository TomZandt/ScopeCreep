//! scopecreep accuracy suite (seed): Stage 0 check 4, published maths against Resolve's own renders.
//!
//! The transforms here are the *published* definitions (see each module for citations). They exist to be
//! compared with frames Resolve itself rendered; nothing in this crate is shipped in the product yet.

pub mod colour;
pub mod transfer;
pub mod imageio;
pub mod data;
pub mod aces2;
pub mod pipeline;
pub mod compare;
pub mod setups;
pub mod report;
