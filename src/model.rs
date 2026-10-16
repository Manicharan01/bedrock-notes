use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

pub const MAX_NOTES: usize = 1000;
pub const MAX_BODY: usize = 2_000_000;
pub const MAX_TOTAL: usize = 8_000_000;
pub const MAX_TITLE: usize = 180;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub updated: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Link {
    pub target: String,
    pub anchor: String,
    pub label: String,
    pub embed: bool,
    pub note_id: Option<String>,
    #[serde(skip)]
    pub range: std::ops::Range<usize>,
    #[serde(skip)]
    pub target_range: std::ops::Range<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub id: String,
    /// UTF-16 offset, matching textarea.selectionStart rather than Rust byte offsets.
    pub index: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Metadata {
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
    pub html: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub notebook_id: String,
    pub revision: i64,
    pub notes: Vec<Note>,
    pub metadata: BTreeMap<String, Metadata>,
    pub backlinks: BTreeMap<String, Vec<String>>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Serialize)]
pub struct Version {
    pub notebook_id: String,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub notebook_id: String,
    pub revision: i64,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    Create {
        title: String,
        #[serde(default)]
        body: String,
    },
    Update {
        id: String,
        body: String,
    },
    Rename {
        id: String,
        title: String,
    },
    Delete {
        id: String,
    },
    Import {
        notes: Vec<ImportNote>,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ImportNote {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub notebook: Snapshot,
    pub selected_id: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    Invalid(String),
    NotFound,
    Conflict,
    TitleExists,
    Database(rusqlite::Error),
    Internal(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Internal(message) => f.write_str(message),
            Self::NotFound => f.write_str("Note not found"),
            Self::Conflict => f.write_str("Notebook changed; reload before writing"),
            Self::TitleExists => f.write_str("A note with that title already exists"),
            Self::Database(error) => write!(f, "Database error: {error}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

pub fn canonical(title: &str) -> String {
    let normalized: String = title.trim().nfc().collect();
    let without_extension = if normalized.to_ascii_lowercase().ends_with(".md") {
        &normalized[..normalized.len() - 3]
    } else {
        &normalized
    };
    without_extension.trim().to_lowercase().nfc().collect()
}

pub fn validate_title(title: &str) -> Result<String, Error> {
    if title.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "Note titles cannot contain control characters".into(),
        ));
    }
    let title: String = title.trim().nfc().collect();
    if title.is_empty()
        || title.chars().count() > MAX_TITLE
        || title
            .chars()
            .any(|c| c.is_control() || "<>:\"\\|?*#[]".contains(c))
        || canonical(&title).is_empty()
        || title
            .split('/')
            .any(|p| p.is_empty() || p.trim() != p || p == "." || p == ".." || p.ends_with('.'))
    {
        return Err(Error::Invalid("Use a title of 1–180 characters without reserved filename characters or empty path segments".into()));
    }
    Ok(title)
}

pub fn validate_notes(notes: &[Note]) -> Result<(), Error> {
    if notes.len() > MAX_NOTES {
        return Err(Error::Invalid("Notebook limit is 1000 notes".into()));
    }
    let mut ids = std::collections::HashSet::new();
    let mut titles = std::collections::HashSet::new();
    let mut total = 0usize;
    for note in notes {
        validate_title(&note.title)?;
        if note.id.is_empty() || !ids.insert(&note.id) || !titles.insert(canonical(&note.title)) {
            return Err(Error::Invalid(
                "Duplicate or empty note identity/title".into(),
            ));
        }
        if note.body.len() > MAX_BODY {
            return Err(Error::Invalid("A note exceeds the 2 MB body limit".into()));
        }
        total = total
            .checked_add(note.body.len())
            .ok_or_else(|| Error::Invalid("Notebook too large".into()))?;
        if total > MAX_TOTAL {
            return Err(Error::Invalid(
                "Notebook exceeds the 8 MB content limit".into(),
            ));
        }
    }
    Ok(())
}

pub fn resolve_note<'a>(notes: &'a [Note], target: &str) -> Option<&'a Note> {
    let key = canonical(target);
    if key.is_empty() {
        return None;
    }
    if let Some(note) = notes.iter().find(|n| canonical(&n.title) == key) {
        return Some(note);
    }
    let mut candidates = notes
        .iter()
        .filter(|n| canonical(n.title.rsplit('/').next().unwrap_or_default()) == key);
    let first = candidates.next()?;
    if candidates.next().is_some() {
        None
    } else {
        Some(first)
    }
}
