//! What the app learns about the Resolve project by script: its Colour Setup, and its project and timeline
//! names (the Sender cannot see them). The reading is done by a helper process (`resolve_helper.rs`, ADR 0006);
//! the [`ProjectReader`] trait is the seam, so the scheduling and labels can be tested with a fake.
use crate::colour_setup::ColourSetup;

/// One answer from the helper: its `key=value` lines, in order (see app/resolve/colour_setup_helper.py).
pub type Answer = Vec<(String, String)>;

/// Something that can read the project when asked. Every method returns at once: a read that takes time is
/// collected later with [`ProjectReader::take_answer`].
pub trait ProjectReader: Send {
    /// Start one read.
    fn request(&mut self);
    /// The answer to the last request, once it has arrived.
    fn take_answer(&mut self) -> Option<Answer>;
    /// The last request has taken too long: give up on it.
    fn abandon(&mut self);
}

/// The project as last read.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectReading {
    /// Resolve's product name and version, e.g. "DaVinci Resolve Studio 21.2.0.6".
    pub product: String,
    pub studio: bool,
    pub project: String,
    pub timeline: Option<String>,
    /// `None` if the settings gave nothing to go on.
    pub colour_setup: Option<ColourSetup>,
}

/// What one answer means.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Read(Box<ProjectReading>),
    /// The project cannot be read at all (Resolve not reachable by script, no project open): what was known is
    /// no longer true.
    Gone(String),
    /// This read failed; what was known may still be true.
    Failed(String),
}

impl Outcome {
    pub fn from_answer(answer: &Answer) -> Outcome {
        let get = |key: &str| answer.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let message = get("message").unwrap_or("").to_string();
        match get("result") {
            Some("ok") => {
                let settings: Vec<(String, String)> =
                    answer.iter().filter_map(|(k, v)| k.strip_prefix("setting.").map(|k| (k.to_string(), v.clone()))).collect();
                let product = [get("product"), get("version")].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
                Outcome::Read(Box::new(ProjectReading {
                    product,
                    studio: get("studio") == Some("1"),
                    project: get("project").unwrap_or("").to_string(),
                    timeline: get("timeline").map(str::to_string),
                    colour_setup: ColourSetup::from_resolve_settings(&settings),
                }))
            }
            Some("unreachable") | Some("no-project") | Some("unavailable") => Outcome::Gone(message),
            _ => Outcome::Failed(if message.is_empty() { "the scripting helper gave no answer".to_string() } else { message }),
        }
    }
}
