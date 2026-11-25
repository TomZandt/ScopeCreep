//! The Update check (#245, spec #234): at start-up the app looks at GitHub's Releases list once, in the background,
//! and works out whether a newer Release has an Installer for this platform. The decision ([`choose`]) is a pure
//! function of the running version, the platform, the Releases list and the skipped version; the fetch is a thin,
//! separate thread whose every failure means "nothing", so offline, blocked and rate-limited checks stay silent.
//! The check sends only a generic User-Agent, "ScopeCreep/X.Y.Z", and GitHub's Accept header.
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use crate::links;

/// How long a check waits for GitHub before it gives up.
const TIMEOUT: Duration = Duration::from_secs(5);
/// The most of a reply that is read (the list of Releases is a few kilobytes per Release).
const REPLY_LIMIT: u64 = 4 * 1024 * 1024;
/// The checksums asset every Release carries beside its Installers.
const SUMS_NAME: &str = "SHA256SUMS";

/// The platforms Installers are made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    MacOs,
}

impl Platform {
    /// The platform this build runs on.
    pub fn current() -> Platform {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

/// Each platform's Installer name, `{v}` standing for the version: the names the packaging scripts make
/// (`scripts/package-linux.sh`; the Windows one is `scopecreep-X.Y.Z-windows-x86_64-setup.exe`, #235). The macOS
/// package is not defined yet (#236): this is a placeholder, so no Release has one and none is offered on a Mac.
const INSTALLERS: [(Platform, &str); 3] = [
    (Platform::Linux, "scopecreep-{v}-linux-x86_64.tar.gz"),
    (Platform::Windows, "scopecreep-{v}-windows-x86_64-setup.exe"),
    (Platform::MacOs, "scopecreep-{v}-macos-arm64.pkg"),
];

/// The name of `platform`'s Installer for `version`.
pub fn installer_name(platform: Platform, version: &str) -> String {
    INSTALLERS
        .iter()
        .find(|(p, _)| *p == platform)
        .map_or_else(String::new, |(_, name)| name.replace("{v}", version))
}

/// A Semantic Version, `major.minor.patch`, compared as numbers (0.10.0 is above 0.9.0). A pre-release or any
/// other shape is not one: only Releases are offered, never Test builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u64, u64, u64);

impl Version {
    /// `0.4.0` or `v0.4.0`.
    fn parse(text: &str) -> Option<Version> {
        let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
        let mut next = || {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse::<u64>().ok()
        };
        let version = Version(next()?, next()?, next()?);
        parts.next().is_none().then_some(version)
    }
}

/// One file attached to a Release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

/// One entry of GitHub's list of Releases, as far as the chooser needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub draft: bool,
    pub page: String,
    pub assets: Vec<Asset>,
}

/// The Releases in `json` (GitHub's list). Anything that is not a list gives none; an entry that is not a Release
/// is skipped without spoiling the others.
pub fn parse_releases(json: &str) -> Vec<Release> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return Vec::new();
    };
    let text = |v: &serde_json::Value, key: &str| v.get(key)?.as_str().map(str::to_string);
    items
        .iter()
        .filter_map(|item| {
            let assets = item
                .get("assets")
                .and_then(|a| a.as_array())
                .map(|assets| {
                    assets
                        .iter()
                        .filter_map(|a| {
                            Some(Asset {
                                name: text(a, "name")?,
                                url: text(a, "browser_download_url")?,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Release {
                tag: text(item, "tag_name")?,
                draft: item.get("draft").and_then(|d| d.as_bool()).unwrap_or(false),
                page: text(item, "html_url")?,
                assets,
            })
        })
        .collect()
}

/// A newer Release worth telling the user about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// "0.5.0".
    pub version: String,
    /// The Release's page: what "What's new" and the Help entry open.
    pub release_url: String,
    pub installer_url: String,
    pub sums_url: String,
}

/// The newest Release above `running` (and above `skipped`, when the user skipped one) that has `platform`'s
/// Installer and the checksums. Drafts, Test builds, equal, older and skipped versions are ignored. `None` when
/// there is none, or `running` is not a version.
pub fn choose(
    running: &str,
    platform: Platform,
    releases: &[Release],
    skipped: Option<&str>,
) -> Option<Offer> {
    let running = Version::parse(running)?;
    let skipped = skipped.and_then(Version::parse);
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = Version::parse(&r.tag)?;
            if version <= running || skipped.is_some_and(|s| version <= s) {
                return None;
            }
            let text = r.tag.strip_prefix('v').unwrap_or(&r.tag);
            let find = |name: &str| r.assets.iter().find(|a| a.name == name);
            let installer = find(&installer_name(platform, text))?;
            let sums = find(SUMS_NAME)?;
            Some((
                version,
                Offer {
                    version: text.to_string(),
                    release_url: r.page.clone(),
                    installer_url: installer.url.clone(),
                    sums_url: sums.url.clone(),
                },
            ))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, offer)| offer)
}

/// The User-Agent the check sends, and the only thing it says about the app.
pub fn user_agent(version: &str) -> String {
    format!("ScopeCreep/{version}")
}

/// The Accept header the check sends, as GitHub asks of its API. The only header besides the User-Agent.
pub const ACCEPT: &str = "application/vnd.github+json";

/// Fetch the list of Releases: one HTTPS GET with a short timeout. `None` on any failure.
pub fn fetch_releases(version: &str) -> Option<String> {
    use std::io::Read;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(user_agent(version))
        .build()
        .into();
    let mut reply = agent
        .get(links::releases_api())
        .header("Accept", ACCEPT)
        .call()
        .ok()?;
    let mut text = String::new();
    reply
        .body_mut()
        .as_reader()
        .take(REPLY_LIMIT)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// What the user chose about updates, kept in its own file next to the accent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePrefs {
    /// "Check for updates at start-up" (on by default).
    pub check_at_startup: bool,
    /// The version the user chose Skip this version for, if any.
    pub skipped: Option<String>,
}

impl Default for UpdatePrefs {
    fn default() -> UpdatePrefs {
        UpdatePrefs {
            check_at_startup: true,
            skipped: None,
        }
    }
}

impl UpdatePrefs {
    pub fn to_text(&self) -> String {
        let on = if self.check_at_startup { "on" } else { "off" };
        let mut text = format!("check_at_startup={on}\n");
        if let Some(skipped) = &self.skipped {
            text += &format!("skipped={skipped}\n");
        }
        text
    }

    /// Preferences from [`UpdatePrefs::to_text`]; anything unreadable is the default.
    pub fn from_text(text: &str) -> UpdatePrefs {
        let mut prefs = UpdatePrefs::default();
        for line in text.lines() {
            match line.split_once('=') {
                Some(("check_at_startup", "off")) => prefs.check_at_startup = false,
                Some(("skipped", v)) if Version::parse(v).is_some() => {
                    prefs.skipped = Some(v.to_string());
                }
                _ => {}
            }
        }
        prefs
    }

    pub fn load(path: &std::path::Path) -> UpdatePrefs {
        std::fs::read_to_string(path)
            .map_or_else(|_| UpdatePrefs::default(), |t| UpdatePrefs::from_text(&t))
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        crate::settings::save(path, &self.to_text())
    }
}

/// The Update check as the app uses it: what is known, and a check that may be running on its own thread.
pub struct Updates {
    prefs: UpdatePrefs,
    running: String,
    platform: Platform,
    offer: Option<Offer>,
    check: Option<Receiver<Option<Offer>>>,
    /// The Help entry asked for the check that is running: open a page when it finishes.
    open_when_done: bool,
    /// An offer the Update dialog has yet to show (#246).
    dialog_due: bool,
}

impl Updates {
    pub fn new(prefs: UpdatePrefs, running: &str, platform: Platform) -> Updates {
        Updates {
            prefs,
            running: running.to_string(),
            platform,
            offer: None,
            check: None,
            open_when_done: false,
            dialog_due: false,
        }
    }

    pub fn prefs(&self) -> &UpdatePrefs {
        &self.prefs
    }

    /// Turn the start-up check on or off.
    pub fn set_check_at_startup(&mut self, on: bool) {
        self.prefs.check_at_startup = on;
    }

    /// The newer Release known, if any.
    pub fn offer(&self) -> Option<&Offer> {
        self.offer.as_ref()
    }

    /// Run the check once at start-up, unless the setting is off. `fetch` answers the Releases list (or `None`);
    /// `wake` is called when the answer is ready, from the check's thread.
    pub fn start_up(
        &mut self,
        fetch: impl FnOnce() -> Option<String> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) {
        if self.prefs.check_at_startup {
            self.start(fetch, wake);
        }
    }

    fn start(
        &mut self,
        fetch: impl FnOnce() -> Option<String> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) {
        if self.check.is_some() {
            return;
        }
        let (running, platform, skipped) = (
            self.running.clone(),
            self.platform,
            self.prefs.skipped.clone(),
        );
        let (tx, rx) = channel();
        self.check = Some(rx);
        std::thread::spawn(move || {
            let offer = fetch().and_then(|text| {
                choose(
                    &running,
                    platform,
                    &parse_releases(&text),
                    skipped.as_deref(),
                )
            });
            let _ = tx.send(offer);
            wake();
        });
    }

    /// What the Help menu calls the update entry: "Update to X.Y.Z…" while a newer Release is known.
    pub fn help_label(&self) -> String {
        self.offer.as_ref().map_or_else(
            || "Check for updates".to_string(),
            |o| format!("Update to {}\u{2026}", o.version),
        )
    }

    /// The Help entry was clicked. With a newer Release known, the Update dialog is due ([`Updates::take_dialog`]).
    /// Without one, a check runs on demand: [`Updates::poll`] says when to open the Releases page, or the dialog
    /// becomes due. Nothing opens now, so this always returns `None`.
    pub fn help_click(
        &mut self,
        fetch: impl FnOnce() -> Option<String> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Option<String> {
        if self.offer.is_some() {
            self.dialog_due = true;
            return None;
        }
        self.open_when_done = true;
        self.start(fetch, wake);
        None
    }

    /// Take in a finished check. A check that found a newer Release than the one known makes the Update dialog due; one that found nothing keeps the offer. Returns the
    /// Releases page's address when the Help entry asked for the check and nothing is newer (or it failed).
    pub fn poll(&mut self) -> Option<String> {
        let rx = self.check.as_ref()?;
        let offer = match rx.try_recv() {
            Ok(offer) => offer,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => None,
        };
        self.check = None;
        // A check that found nothing (or failed) leaves a known offer alone; only a newly found one is shown.
        if let Some(found) = offer {
            let new = self
                .offer
                .as_ref()
                .is_none_or(|o| o.version != found.version);
            self.dialog_due |= new;
            self.offer = Some(found);
        }
        let asked = std::mem::take(&mut self.open_when_done);
        (asked && self.offer.is_none()).then(links::releases_page)
    }

    /// The offer the Update dialog should show now, once: after a check found one, or the Help entry asked.
    pub fn take_dialog(&mut self) -> Option<Offer> {
        if std::mem::take(&mut self.dialog_due) {
            self.offer.clone()
        } else {
            None
        }
    }

    /// Skip this version: remember it (save [`Updates::prefs`] after) and stop offering it.
    pub fn skip(&mut self) {
        if let Some(offer) = self.offer.take() {
            self.prefs.skipped = Some(offer.version);
        }
    }

    /// The version this build runs.
    pub fn running(&self) -> &str {
        &self.running
    }
}
