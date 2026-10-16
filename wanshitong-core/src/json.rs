//! Values stored in SQL as versioned JSON text.
//!
//! JSON names enum variants and struct fields, so a stored value can still be
//! read after its type gains a variant or an optional field. A change that
//! can't be read that way, e.g. renaming or restructuring a field, adds a
//! version: the stored value is tagged with its version, e.g. `{"V1": ...}`,
//! and older versions convert into the current one when they're read.

use crate::{CoreError, CoreErrorContext, error::DecodeError};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;

/// A type stored in SQL as versioned JSON.
pub(crate) trait Stored: Serialize + Sized {
    /// An enum of every version the type has been stored as, e.g.
    /// `enum Versioned { V1(Old), V2(Current) }`. Each converts into the
    /// current type.
    type Versions: DeserializeOwned + Into<Self>;
    /// The variant of [`Self::Versions`] that is the current type, e.g. `V2`.
    const VERSION: &'static str;
}

/// The value tagged with its current version.
pub(crate) fn encode<T: Stored>(value: &T) -> String {
    serde_json::to_string(&BTreeMap::from([(T::VERSION, value)]))
        .expect("stored values serialize to JSON")
}

/// Decodes a value stored in any of its versions.
pub(crate) fn decode<T: Stored>(json: &str) -> Result<T, CoreError> {
    let versioned: T::Versions = serde_json::from_str(json)
        .map_err(|e| DecodeError(e.into()))
        .map_err(CoreErrorContext::from)?;
    Ok(versioned.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Old {
        name: String,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Current {
        title: String,
    }

    mod written {
        use super::*;

        #[derive(Deserialize)]
        pub enum Versions {
            V1(Old),
        }

        impl From<Versions> for Old {
            fn from(Versions::V1(old): Versions) -> Self {
                old
            }
        }

        impl Stored for Old {
            type Versions = Versions;
            const VERSION: &'static str = "V1";
        }
    }

    /// `Old` after a field was renamed, which needs a new version.
    #[derive(Deserialize)]
    enum Versions {
        V1(Old),
        V2(Current),
    }

    impl From<Versions> for Current {
        fn from(versioned: Versions) -> Self {
            match versioned {
                Versions::V1(Old { name }) => Current { title: name },
                Versions::V2(current) => current,
            }
        }
    }

    impl Stored for Current {
        type Versions = Versions;
        const VERSION: &'static str = "V2";
    }

    #[test]
    fn reads_values_stored_in_older_versions() {
        let old = encode(&Old {
            name: "Heat".into(),
        });
        assert_eq!(old, r#"{"V1":{"name":"Heat"}}"#);

        assert_eq!(
            decode::<Current>(&old).unwrap(),
            Current {
                title: "Heat".into()
            }
        );
        let current = encode(&Current {
            title: "Heat".into(),
        });
        assert_eq!(
            decode::<Current>(&current).unwrap(),
            Current {
                title: "Heat".into()
            }
        );
    }
}
