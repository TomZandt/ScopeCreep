//! Locating the test data the harness produced (never inside git):
//!   <root>/dumps-feed/scopecreep-test-<key>/<clip>.exr        probe Sender frame (Timeline Colour Space)
//!   <root>/renders/scopecreep-test-<key>/<clip>-exr/*.exr     Resolve reference, float, Output Colour Space
//!   <root>/renders/scopecreep-test-<key>/<clip>-tif/*.tif     Resolve reference, 16-bit, Output Colour Space

use crate::imageio::{self, Image};
use std::path::{Path, PathBuf};

pub const PROJECT_PREFIX: &str = "scopecreep-test-";
pub const CLIPS: [&str; 4] = ["ramp", "chart", "outofrange", "natural"];

pub fn testdata_root() -> PathBuf {
    if let Some(p) = std::env::var_os("SCOPECREEP_TESTDATA") {
        return PathBuf::from(p);
    }
    PathBuf::from(std::env::var_os("HOME").expect("HOME not set")).join("scopecreep-testdata")
}

fn first_file(dir: &Path, ext: &str) -> Result<PathBuf, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    v.sort();
    v.into_iter().next().ok_or_else(|| format!("no .{ext} in {}", dir.display()))
}

pub fn feed_path(root: &Path, key: &str, clip: &str) -> PathBuf {
    root.join("dumps-feed").join(format!("{PROJECT_PREFIX}{key}")).join(format!("{clip}.exr"))
}

pub fn read_feed(root: &Path, key: &str, clip: &str) -> Result<Image, String> {
    imageio::read_exr(&feed_path(root, key, clip))
}

pub fn read_reference_exr(root: &Path, key: &str, clip: &str) -> Result<Image, String> {
    let dir = root.join("renders").join(format!("{PROJECT_PREFIX}{key}")).join(format!("{clip}-exr"));
    imageio::read_exr(&first_file(&dir, "exr")?)
}

pub fn read_reference_tiff(root: &Path, key: &str, clip: &str) -> Result<Image, String> {
    let dir = root.join("renders").join(format!("{PROJECT_PREFIX}{key}")).join(format!("{clip}-tif"));
    imageio::read_tiff16(&first_file(&dir, "tif")?)
}
