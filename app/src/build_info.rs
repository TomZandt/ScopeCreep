//! Which build this is, as one value: the product version and the Feed protocol it speaks. `--version`, the About
//! popover and the Report a problem issue all read it, so they can never disagree (#230).

/// The product version and the Feed protocol version ("1.2").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    pub version: String,
    pub protocol: String,
}

impl Build {
    pub fn new(version: &str, protocol: &str) -> Build {
        Build {
            version: version.to_string(),
            protocol: protocol.to_string(),
        }
    }

    /// This build: the app's version and the Feed crate's protocol.
    pub fn current() -> Build {
        Build::new(
            env!("CARGO_PKG_VERSION"),
            &format!(
                "{}.{}",
                scopecreep_feed::VERSION_MAJOR,
                scopecreep_feed::VERSION_MINOR
            ),
        )
    }

    /// The version and Feed protocol: "1.2.3 (Feed protocol 1.2)". The About popover shows this under the
    /// wordmark, which is the name.
    pub fn details(&self) -> String {
        format!("{} (Feed protocol {})", self.version, self.protocol)
    }

    /// The name, version and Feed protocol: what `--version` prints.
    pub fn line(&self) -> String {
        format!("ScopeCreep {}", self.details())
    }
}
