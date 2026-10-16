//! Pagination cursors, as opaque strings for URLs.

use base64::Engine;
use serde::{Serialize, de::DeserializeOwned};

pub(crate) fn encode_cursor(cursor: &impl Serialize) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).unwrap())
}

/// `None` if `s` isn't a cursor of this type.
pub(crate) fn decode_cursor<T: DeserializeOwned>(s: &str) -> Option<T> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}
