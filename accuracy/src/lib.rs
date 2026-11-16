//! scopecreep accuracy suite: published maths, and the app's Output Conversion of the real Feed, against Resolve's
//! own renders (Stage 0 check 4, ticket #10; promoted in ticket #17).
//!
//! The transforms here are the *published* definitions in f64 (see each module for citations). They are the single
//! definition of each Output Conversion: the app builds its GPU conversion's matrices, constants and ACES 2.0
//! tables from them (app/src/output_conversion.rs), and the accuracy suite proves both against Resolve.

pub mod colour;
pub mod transfer;
pub mod imageio;
pub mod data;
pub mod aces2;
pub mod pipeline;
pub mod compare;
pub mod setups;
pub mod report;
pub mod state;
