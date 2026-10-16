use std::path::{Component, Path};
use thiserror::Error;

/// Why a name (of a data source, interpretation or view) was rejected. Reads
/// as the end of a sentence about the name: "interpretation name is empty".
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NameError {
    #[error("is empty")]
    Empty,
    #[error("is longer than {MAX_NAME_LEN} bytes")]
    TooLong,
    #[error("has leading or trailing whitespace")]
    Untrimmed,
    #[error("has no letters")]
    NoLetters,
    #[error("contains a control character")]
    ControlCharacter,
}

pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        Err(NameError::Empty)
    } else if name.len() > MAX_NAME_LEN {
        Err(NameError::TooLong)
    } else if name != name.trim() {
        Err(NameError::Untrimmed)
    } else if !name.chars().any(|c| c.is_alphabetic()) {
        Err(NameError::NoLetters)
    } else if name.chars().any(|c| c.is_control()) {
        Err(NameError::ControlCharacter)
    } else {
        Ok(())
    }
}

pub const MAX_NAME_LEN: usize = 127;

/// Whether one path is equal to or inside the other, compared by components.
pub fn paths_overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// An absolute path without `..`, so that comparing components is meaningful.
pub fn is_normal_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| component != Component::ParentDir)
}
