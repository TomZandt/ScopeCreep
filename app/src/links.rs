//! The web addresses the Help menu opens (#231, spec #230), in one place: changing the website or the Buy Me a
//! Coffee page is a one-line change here (and in LICENSE.md and LICENSING.md). The app never goes online itself; it
//! hands an address to the system's browser.
use crate::commands::Command;

const REPO: &str = "https://github.com/TomZandt/ScopeCreep";
const WEBSITE: &str = "https://www.tomzandtvalentine.co.uk/";
const BUY_ME_A_COFFEE: &str = "https://www.buymeacoffee.com/tomzandtvalentine";

/// The address a link Command opens, or `None` for a Command that is not a link. `version` and `protocol` (the Feed
/// protocol, "1.2") fill in the Report a problem issue.
pub fn address(command: Command, version: &str, protocol: &str) -> Option<String> {
    Some(match command {
        Command::Licence => format!("{REPO}/blob/main/LICENSING.md"),
        Command::Changelog => format!("{REPO}/blob/main/CHANGELOG.md"),
        Command::Website => WEBSITE.to_string(),
        Command::ReportProblem => {
            let body =
                format!("ScopeCreep {version}\nFeed protocol {protocol}\n\nWhat happened:\n");
            format!("{REPO}/issues/new?body={}", encode(&body))
        }
        Command::CheckForUpdates => format!("{REPO}/releases"),
        Command::BuyMeACoffee => BUY_ME_A_COFFEE.to_string(),
        _ => return None,
    })
}

/// Percent-encode `text` for a query value: everything but letters, digits and `-_.~`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Hand `url` to the system's opener. Never waits and never fails the app: a failure only logs.
pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let opener = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(url)
        .spawn();
    #[cfg(target_os = "macos")]
    let opener = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let opener = std::process::Command::new("xdg-open").arg(url).spawn();
    match opener {
        // Reap it in the background so it does not linger as a zombie.
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => eprintln!("ScopeCreep: cannot open {url}: {e}"),
    }
}
