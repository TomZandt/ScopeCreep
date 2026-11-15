//! The Colour Setup block of a Feed slot (protocol 1.1, feed/PROTOCOL.md): a provenance and a few lines of
//! `key=value` text. This crate only carries the block; what the keys mean is the app's business.
use crate::layout::{S_COLOUR_SETUP, S_COLOUR_SETUP_BYTES, S_COLOUR_SETUP_PROVENANCE};
use crate::layout::{COLOUR_SETUP_CAPACITY, Mapping};

/// Where a Colour Setup came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Read from the Resolve project.
    Project,
    /// Set by hand on the Sender node. Wins over the project.
    Manual,
}

impl Provenance {
    fn code(self) -> u32 {
        match self {
            Provenance::Project => 1,
            Provenance::Manual => 2,
        }
    }
    fn from_code(code: u32) -> Option<Provenance> {
        match code {
            1 => Some(Provenance::Project),
            2 => Some(Provenance::Manual),
            _ => None,
        }
    }
}

/// A Colour Setup as it travels in a Feed slot.
#[derive(Clone, Debug, PartialEq)]
pub struct SentColourSetup {
    pub provenance: Provenance,
    /// UTF-8 lines of `key=value`.
    pub text: String,
}

/// Bytes of `text` that go in the block: all of it if it fits, else up to its last whole line that fits.
fn cut(text: &str) -> usize {
    if text.len() <= COLOUR_SETUP_CAPACITY {
        return text.len();
    }
    text.as_bytes()[..COLOUR_SETUP_CAPACITY].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1)
}

impl SentColourSetup {
    pub fn new(provenance: Provenance, text: impl Into<String>) -> SentColourSetup {
        SentColourSetup { provenance, text: text.into() }
    }

    /// The `key=value` lines in order, trimmed. Lines without `=` are skipped.
    pub fn entries(&self) -> Vec<(String, String)> {
        self.text
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| !k.is_empty())
            .collect()
    }

    /// The value of `key`, if the block has it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.text.lines().filter_map(|line| line.split_once('=')).find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim())
    }

    /// Write into the slot whose header starts at `slot`. The caller owns the slot (WRITING).
    pub(crate) fn store(setup: Option<&SentColourSetup>, map: &Mapping, slot: usize) {
        match setup {
            None => {
                map.set_u32(slot + S_COLOUR_SETUP_PROVENANCE, 0);
                map.set_u32(slot + S_COLOUR_SETUP_BYTES, 0);
            }
            Some(s) => {
                let n = cut(&s.text);
                map.set_bytes(slot + S_COLOUR_SETUP, &s.text.as_bytes()[..n]);
                map.set_u32(slot + S_COLOUR_SETUP_BYTES, n as u32);
                map.set_u32(slot + S_COLOUR_SETUP_PROVENANCE, s.provenance.code());
            }
        }
    }

    /// Read from the slot whose header starts at `slot`. The caller owns the slot (READING). Anything this reader
    /// cannot make sense of (an unknown provenance, an impossible size) reads as no Colour Setup.
    pub(crate) fn load(map: &Mapping, slot: usize) -> Option<SentColourSetup> {
        let provenance = Provenance::from_code(map.u32(slot + S_COLOUR_SETUP_PROVENANCE))?;
        let n = map.u32(slot + S_COLOUR_SETUP_BYTES) as usize;
        if n > COLOUR_SETUP_CAPACITY {
            return None;
        }
        let text = String::from_utf8_lossy(&map.bytes(slot + S_COLOUR_SETUP, n)).into_owned();
        Some(SentColourSetup { provenance, text })
    }
}
