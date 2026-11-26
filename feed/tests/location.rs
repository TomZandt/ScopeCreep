//! Where the Feed lives (feed/PROTOCOL.md, ADR 0043): the platform's location unless `SCOPECREEP_FEED` says
//! otherwise. One test, since it changes the environment.
use scopecreep_feed::default_path;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
const PLATFORM_LOCATION: &str = "/dev/shm/scopecreep-feed";
#[cfg(windows)]
const PLATFORM_LOCATION: &str = "Local\\scopecreep-feed";

#[test]
fn the_feed_lives_at_the_platform_location_unless_scopecreep_feed_overrides_it() {
    // SAFETY: this test binary has only this test, so nothing reads the environment meanwhile.
    unsafe { std::env::remove_var("SCOPECREEP_FEED") };
    assert_eq!(default_path(), PathBuf::from(PLATFORM_LOCATION));

    unsafe { std::env::set_var("SCOPECREEP_FEED", "") };
    assert_eq!(
        default_path(),
        PathBuf::from(PLATFORM_LOCATION),
        "an empty override is no override"
    );

    unsafe { std::env::set_var("SCOPECREEP_FEED", "elsewhere-feed") };
    assert_eq!(default_path(), PathBuf::from("elsewhere-feed"));
}
