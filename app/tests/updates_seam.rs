//! The Update check at the pure seam (#245, spec #234): which Release the chooser offers, the Help entry that
//! shows it, the start-up setting, and what the check does on its own thread. No network: the fetch is a closure.
use scopecreep::commands::{Command, Hotkeys};
use scopecreep::components::*;
use scopecreep::links;
use scopecreep::menu_bar::{self, MenuAction};
use scopecreep::updates::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// One entry of GitHub's Releases list: a tag, whether it is a draft, and the names of its assets.
struct Rel {
    tag: String,
    draft: bool,
    assets: Vec<String>,
}

/// A Release of `version` with the Linux Installer and the checksums.
fn linux(version: &str) -> Rel {
    Rel {
        tag: format!("v{version}"),
        draft: false,
        assets: vec![
            format!("scopecreep-{version}-linux-x86_64.tar.gz"),
            "SHA256SUMS".to_string(),
        ],
    }
}

fn windows(version: &str) -> Rel {
    Rel {
        tag: format!("v{version}"),
        draft: false,
        assets: vec![
            format!("scopecreep-{version}-windows-x86_64-setup.exe"),
            "SHA256SUMS".to_string(),
        ],
    }
}

fn json(releases: &[Rel]) -> String {
    let items: Vec<String> = releases
        .iter()
        .map(|r| {
            let assets: Vec<String> = r
                .assets
                .iter()
                .map(|a| {
                    format!(
                        r#"{{"name":"{a}","browser_download_url":"https://dl.example/{}/{a}"}}"#,
                        r.tag
                    )
                })
                .collect();
            format!(
                r#"{{"tag_name":"{}","draft":{},"html_url":"https://rel.example/{}","assets":[{}]}}"#,
                r.tag,
                r.draft,
                r.tag,
                assets.join(",")
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn offer(
    running: &str,
    platform: Platform,
    releases: &[Rel],
    skipped: Option<&str>,
) -> Option<Offer> {
    choose(running, platform, &parse_releases(&json(releases)), skipped)
}

#[test]
fn a_newer_release_with_the_platforms_installer_is_offered() {
    let got = offer("0.3.1", Platform::Linux, &[linux("0.4.0")], None).unwrap();
    assert_eq!(got.version, "0.4.0");
    assert_eq!(got.release_url, "https://rel.example/v0.4.0");
    assert_eq!(
        got.installer_url,
        "https://dl.example/v0.4.0/scopecreep-0.4.0-linux-x86_64.tar.gz"
    );
    assert_eq!(got.sums_url, "https://dl.example/v0.4.0/SHA256SUMS");
}

#[test]
fn the_newest_version_wins_whatever_the_order_of_the_list() {
    let all = [linux("0.4.0"), linux("0.5.0"), linux("0.4.1")];
    let got = offer("0.3.1", Platform::Linux, &all, None).unwrap();
    assert_eq!(got.version, "0.5.0");
}

#[test]
fn versions_compare_as_numbers_so_0_10_0_is_above_0_9_0() {
    let got = offer(
        "0.9.0",
        Platform::Linux,
        &[linux("0.9.0"), linux("0.10.0")],
        None,
    );
    assert_eq!(got.unwrap().version, "0.10.0");
    assert!(offer("0.10.0", Platform::Linux, &[linux("0.9.0")], None).is_none());
}

#[test]
fn equal_and_older_releases_offer_nothing() {
    let all = [linux("0.3.1"), linux("0.2.0")];
    assert!(offer("0.3.1", Platform::Linux, &all, None).is_none());
    assert!(offer("0.4.0", Platform::Linux, &all, None).is_none());
}

#[test]
fn a_release_without_this_platforms_installer_is_ignored() {
    let all = [linux("0.5.0"), linux("0.4.0")];
    assert!(offer("0.3.1", Platform::Windows, &all, None).is_none());
    assert!(offer("0.3.1", Platform::MacOs, &all, None).is_none());
    // A newer Release with only another platform: the newest that has this one.
    let got = offer(
        "0.3.1",
        Platform::Linux,
        &[windows("0.5.0"), linux("0.4.0")],
        None,
    );
    assert_eq!(got.unwrap().version, "0.4.0");
}

#[test]
fn each_platform_looks_for_its_own_installer_name() {
    assert_eq!(
        installer_name(Platform::Linux, "1.2.3"),
        "scopecreep-1.2.3-linux-x86_64.tar.gz"
    );
    assert_eq!(
        installer_name(Platform::Windows, "1.2.3"),
        "scopecreep-1.2.3-windows-x86_64-setup.exe"
    );
    let got = offer("0.4.0", Platform::Windows, &[windows("0.5.0")], None).unwrap();
    assert!(got.installer_url.ends_with("windows-x86_64-setup.exe"));
}

#[test]
fn a_release_with_no_checksums_is_ignored() {
    let mut bare = linux("0.5.0");
    bare.assets.pop();
    assert!(offer("0.4.0", Platform::Linux, &[bare], None).is_none());
}

#[test]
fn drafts_and_test_versions_are_ignored() {
    let mut draft = linux("0.5.0");
    draft.draft = true;
    assert!(offer("0.4.0", Platform::Linux, &[draft], None).is_none());
    assert!(offer("0.4.0", Platform::Linux, &[linux("0.5.0-rc.1")], None).is_none());
}

#[test]
fn a_skipped_version_is_ignored_unless_a_newer_one_exists() {
    assert!(offer("0.4.0", Platform::Linux, &[linux("0.5.0")], Some("0.5.0")).is_none());
    let two = [linux("0.5.0"), linux("0.6.0")];
    let got = offer("0.4.0", Platform::Linux, &two, Some("0.5.0")).unwrap();
    assert_eq!(got.version, "0.6.0");
    // Skipping a version also skips the older ones.
    assert!(offer("0.4.0", Platform::Linux, &[linux("0.5.0")], Some("0.6.0")).is_none());
}

#[test]
fn a_malformed_or_empty_list_offers_nothing() {
    for text in [
        "",
        "not json",
        "{}",
        "[]",
        "null",
        r#"[{"tag_name":5}]"#,
        "[1,2]",
    ] {
        let got = choose("0.1.0", Platform::Linux, &parse_releases(text), None);
        assert!(got.is_none(), "{text}");
    }
    // An unreadable running version offers nothing, and a bad entry does not spoil the good ones.
    let good = json(&[linux("0.5.0")]);
    assert!(choose("beta", Platform::Linux, &parse_releases(&good), None).is_none());
    let mixed = format!(r#"[{{"tag_name":"x"}},42,{}"#, &good[1..]);
    let got = choose("0.1.0", Platform::Linux, &parse_releases(&mixed), None);
    assert_eq!(got.map(|o| o.version), Some("0.5.0".to_string()));
}

#[test]
fn the_user_agent_is_generic_and_the_addresses_are_in_links() {
    assert_eq!(user_agent("0.4.0"), "ScopeCreep/0.4.0");
    assert_eq!(ACCEPT, "application/vnd.github+json");
    assert_eq!(
        links::releases_api(),
        "https://api.github.com/repos/TomZandt/ScopeCreep/releases?per_page=100"
    );
    assert_eq!(
        links::releases_page(),
        "https://github.com/TomZandt/ScopeCreep/releases"
    );
}

// The preferences.

#[test]
fn checking_at_start_up_is_on_by_default_and_the_preferences_round_trip() {
    let prefs = UpdatePrefs::default();
    assert!(prefs.check_at_startup);
    assert_eq!(prefs.skipped, None);
    let changed = UpdatePrefs {
        check_at_startup: false,
        skipped: Some("0.5.0".to_string()),
    };
    assert_eq!(UpdatePrefs::from_text(&changed.to_text()), changed);
    assert_eq!(UpdatePrefs::from_text(&prefs.to_text()), prefs);
    assert_eq!(UpdatePrefs::from_text("garbage\n\n=\nskipped=\n"), prefs);
    assert_eq!(UpdatePrefs::from_text("check_at_startup=maybe"), prefs);
}

#[test]
fn the_preferences_persist_in_a_file() {
    let dir = std::env::temp_dir().join(format!("scopecreep-updates-{}", std::process::id()));
    let path = dir.join("updates");
    assert_eq!(UpdatePrefs::load(&path), UpdatePrefs::default());
    let off = UpdatePrefs {
        check_at_startup: false,
        skipped: None,
    };
    off.save(&path).unwrap();
    assert_eq!(UpdatePrefs::load(&path), off);
    let _ = std::fs::remove_dir_all(dir);
}

// The check on its thread.

fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A fetch that answers `text` and counts how often it was asked.
fn counting(
    text: Option<String>,
) -> (
    Arc<AtomicUsize>,
    impl Fn() -> Option<String> + Send + 'static,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    (calls, move || {
        seen.fetch_add(1, Ordering::SeqCst);
        text.clone()
    })
}

fn newer() -> String {
    json(&[linux("0.5.0")])
}

fn off() -> UpdatePrefs {
    UpdatePrefs {
        check_at_startup: false,
        skipped: None,
    }
}

#[test]
fn with_the_setting_off_no_check_starts_at_start_up() {
    let (calls, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(off(), "0.4.0", Platform::Linux);
    updates.start_up(fetch, || {});
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(updates.poll(), None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn the_start_up_check_makes_the_help_entry_offer_the_update_without_opening_anything() {
    let (calls, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
    assert_eq!(updates.help_label(), "Check for updates");
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the Window is woken");
    assert_eq!(updates.poll(), None, "start-up opens no page");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(updates.help_label(), "Update to 0.5.0\u{2026}");
    assert_eq!(updates.offer().map(|o| o.version.as_str()), Some("0.5.0"));
    // The Help entry opens the Update dialog, without another check.
    let (more, fetch) = counting(None);
    assert_eq!(updates.help_click(fetch, || {}), None);
    assert_eq!(more.load(Ordering::SeqCst), 0);
    assert_eq!(
        updates.take_dialog().map(|o| o.release_url),
        Some("https://rel.example/v0.5.0".to_string())
    );
}

// The Update dialog's state (#246).

#[test]
fn the_start_up_check_shows_the_dialog_once_per_start() {
    let (_, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
    assert!(updates.take_dialog().is_none(), "nothing before the check");
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    updates.poll();
    assert_eq!(
        updates.take_dialog().map(|o| o.version),
        Some("0.5.0".to_string())
    );
    assert!(updates.take_dialog().is_none(), "once per start");
    // Later changes nothing: the entry still offers the update for the Help menu.
    assert_eq!(updates.help_label(), "Update to 0.5.0\u{2026}");
}

#[test]
fn no_dialog_when_up_to_date_failed_or_the_check_is_off() {
    for text in [None, Some("[]".to_string()), Some(json(&[linux("0.4.0")]))] {
        let (_, fetch) = counting(text);
        let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
        let (tx, rx) = mpsc::channel();
        updates.start_up(fetch, move || tx.send(()).unwrap());
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        updates.poll();
        assert!(updates.take_dialog().is_none());
    }
    let (_, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(off(), "0.4.0", Platform::Linux);
    updates.start_up(fetch, || {});
    std::thread::sleep(Duration::from_millis(50));
    updates.poll();
    assert!(updates.take_dialog().is_none());
}

#[test]
fn skipping_remembers_the_version_and_the_chooser_honours_it() {
    let (_, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    updates.poll();
    updates.skip();
    assert_eq!(updates.prefs().skipped.as_deref(), Some("0.5.0"));
    assert!(updates.offer().is_none());
    assert_eq!(updates.help_label(), "Check for updates");
    // It persists with the other preferences, and a restart does not offer it again...
    let again = UpdatePrefs::from_text(&updates.prefs().to_text());
    assert_eq!(again.skipped.as_deref(), Some("0.5.0"));
    let (_, fetch) = counting(Some(newer()));
    let mut restarted = Updates::new(again.clone(), "0.4.0", Platform::Linux);
    let (tx, rx) = mpsc::channel();
    restarted.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    restarted.poll();
    assert!(restarted.take_dialog().is_none());
    // ...but a newer one is.
    let (_, fetch) = counting(Some(json(&[linux("0.5.0"), linux("0.6.0")])));
    let mut newer_one = Updates::new(again, "0.4.0", Platform::Linux);
    let (tx, rx) = mpsc::channel();
    newer_one.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    newer_one.poll();
    assert_eq!(
        newer_one.take_dialog().map(|o| o.version),
        Some("0.6.0".to_string())
    );
}

#[test]
fn a_failed_or_offline_check_says_nothing() {
    let (_, fetch) = counting(None);
    let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(updates.poll(), None);
    assert_eq!(updates.help_label(), "Check for updates");
    assert!(updates.offer().is_none());
}

/// Run one more check on `updates` (a start-up one) with `text` as the answer and take it in.
fn recheck(updates: &mut Updates, text: Option<String>) {
    let (_, fetch) = counting(text);
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    updates.poll();
}

#[test]
fn a_failed_or_empty_recheck_keeps_the_known_offer_and_opens_no_dialog() {
    for text in [None, Some("[]".to_string())] {
        let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
        recheck(&mut updates, Some(newer()));
        assert!(updates.take_dialog().is_some());
        recheck(&mut updates, text);
        assert_eq!(updates.offer().map(|o| o.version.as_str()), Some("0.5.0"));
        assert_eq!(updates.help_label(), "Update to 0.5.0\u{2026}");
        assert!(updates.take_dialog().is_none(), "nothing new to show");
    }
}

#[test]
fn a_recheck_finding_the_same_offer_does_not_show_the_dialog_again_but_a_newer_one_does() {
    let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
    recheck(&mut updates, Some(newer()));
    assert!(updates.take_dialog().is_some());
    recheck(&mut updates, Some(newer()));
    assert!(updates.take_dialog().is_none());
    recheck(&mut updates, Some(json(&[linux("0.6.0")])));
    assert_eq!(
        updates.take_dialog().map(|o| o.version),
        Some("0.6.0".to_string())
    );
}

#[test]
fn a_skipped_version_is_not_offered_at_start_up() {
    let (_, fetch) = counting(Some(newer()));
    let prefs = UpdatePrefs {
        check_at_startup: true,
        skipped: Some("0.5.0".to_string()),
    };
    let mut updates = Updates::new(prefs, "0.4.0", Platform::Linux);
    let (tx, rx) = mpsc::channel();
    updates.start_up(fetch, move || tx.send(()).unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    updates.poll();
    assert!(updates.offer().is_none());
}

#[test]
fn the_help_entry_checks_on_demand_then_shows_the_dialog_or_opens_the_releases_page() {
    // Something newer: the dialog.
    let (calls, fetch) = counting(Some(newer()));
    let mut updates = Updates::new(off(), "0.4.0", Platform::Linux);
    assert_eq!(
        updates.help_click(fetch, || {}),
        None,
        "the answer comes later"
    );
    let shown = wait(|| {
        assert_eq!(updates.poll(), None, "an offer opens no page");
        updates.take_dialog()
    });
    assert_eq!(shown.release_url, "https://rel.example/v0.5.0");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(updates.help_label(), "Update to 0.5.0\u{2026}");
    // Nothing newer, or the check failed: the Releases page.
    for text in [None, Some("[]".to_string())] {
        let (_, fetch) = counting(text);
        let mut updates = Updates::new(UpdatePrefs::default(), "0.4.0", Platform::Linux);
        assert_eq!(updates.help_click(fetch, || {}), None);
        assert_eq!(wait(|| updates.poll()), links::releases_page());
    }
}

#[test]
fn the_help_entry_is_one_entry_that_runs_check_for_updates() {
    for (label, want) in [
        (None, "Check for updates"),
        (Some("Update to 0.5.0\u{2026}"), "Update to 0.5.0\u{2026}"),
    ] {
        let menu = menu_bar::help_menu_with(&Hotkeys::default(), label);
        let labels: Vec<String> = menu
            .drop
            .entries
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item(i) => Some(i.label.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(labels.len(), Command::HELP.len());
        let at = labels.iter().position(|l| l == want).expect("the entry");
        assert_eq!(
            menu.action(at),
            Some(MenuAction::Run(Command::CheckForUpdates))
        );
    }
}

#[test]
fn the_settings_menu_has_the_start_up_check_marked_when_on() {
    for on in [true, false] {
        let menu = menu_bar::settings_menu_with(on);
        let entry = menu
            .drop
            .entries
            .iter()
            .enumerate()
            .find_map(|(i, e)| match e {
                MenuEntry::Item(item) if item.label == "Check for updates at start-up" => {
                    Some((i, item.clone()))
                }
                _ => None,
            })
            .expect("the entry");
        assert_eq!(entry.1.on, on);
        assert_eq!(menu.action(entry.0), Some(MenuAction::ToggleUpdateCheck));
    }
}
