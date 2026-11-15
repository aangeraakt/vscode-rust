//! Typed string identifiers and sequential id generation.
//!
//! Ported from:
//! - `src/vs/base/common/idGenerator.ts` (`IdGenerator`)
//! - the string-id conventions of `src/vs/platform/commands/common/commands.ts` (command ids) and
//!   `src/vs/editor/common/languages/language.ts` (language ids)

use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Creates an id from any string.
            pub fn new(id: impl Into<Arc<str>>) -> Self {
                Self(id.into())
            }

            /// The id as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.into())
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id.into())
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), &*self.0)
            }
        }
    };
}

string_id!(
    /// A command identifier such as `editor.action.selectAll`. Command ids are plain strings in
    /// VS Code, so extensions and keybinding files can name them.
    CommandId
);

string_id!(
    /// A language identifier such as `typescript` or `rust`.
    LanguageId
);

/// Generates ids of the form `<prefix><n>` with `n` starting at 1, like VS Code's `IdGenerator`.
/// Thread-safe.
#[derive(Debug)]
pub struct IdGenerator {
    prefix: String,
    last_id: AtomicU64,
}

impl IdGenerator {
    /// Creates a generator for the given prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            last_id: AtomicU64::new(0),
        }
    }

    /// Returns the next id: `<prefix>1`, `<prefix>2`, ...
    pub fn next_id(&self) -> String {
        let n = self.last_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{}{n}", self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn id_generator_counts_from_one() {
        let generator = IdGenerator::new("id#");
        assert_eq!(generator.next_id(), "id#1");
        assert_eq!(generator.next_id(), "id#2");
    }

    #[test]
    fn string_ids_compare_and_look_up_by_str() {
        let mut map = HashMap::new();
        map.insert(CommandId::from("a.b"), 1);
        assert_eq!(map.get("a.b"), Some(&1));
        assert_eq!(CommandId::from("x").to_string(), "x");
        assert_eq!(
            format!("{:?}", LanguageId::from("rust")),
            "LanguageId(\"rust\")"
        );
    }
}
