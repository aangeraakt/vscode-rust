//! Text model indentation options read from `editor.*` settings.
//!
//! Ported from `src/vs/editor/common/core/misc/textModelDefaults.ts` (`EDITOR_MODEL_DEFAULTS`)
//! and `src/vs/editor/common/services/modelService.ts` (`_readModelOptions`). These options
//! configure a text model, not an editor widget, so they are kept apart from
//! [`ValidatedEditorOptions`](crate::options::ValidatedEditorOptions). `indentSize` is not ported
//! yet.

use serde_json::{Map, Value};

use crate::value::{boolean, clamped_int};

/// Default tab size (`EDITOR_MODEL_DEFAULTS.tabSize`).
pub const DEFAULT_TAB_SIZE: i64 = 4;

/// Indentation settings of a text model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelOptions {
    /// `editor.tabSize`, between 1 and 100.
    pub tab_size: u32,
    /// `editor.insertSpaces`: whether Tab inserts spaces.
    pub insert_spaces: bool,
    /// `editor.detectIndentation`: whether indentation is detected from the file content.
    pub detect_indentation: bool,
}

impl ModelOptions {
    /// Reads the model options from raw `editor.*` settings (`_readModelOptions`). A missing or
    /// invalid value gives its default.
    pub fn validate(raw: &Map<String, Value>) -> Self {
        Self {
            tab_size: clamped_int(raw.get("tabSize"), DEFAULT_TAB_SIZE, 1, 100) as u32,
            insert_spaces: boolean(raw.get("insertSpaces"), true),
            detect_indentation: boolean(raw.get("detectIndentation"), true),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Cases follow the defaults and coercions of `modelService._readModelOptions`. No upstream
    //! test covers this function directly.

    use super::*;
    use serde_json::json;

    fn validate(settings: Value) -> ModelOptions {
        ModelOptions::validate(&settings.as_object().cloned().unwrap_or_default())
    }

    #[test]
    fn defaults() {
        assert_eq!(
            validate(json!({})),
            ModelOptions {
                tab_size: 4,
                insert_spaces: true,
                detect_indentation: true,
            }
        );
    }

    #[test]
    fn tab_size_is_clamped_to_one_to_one_hundred() {
        assert_eq!(validate(json!({ "tabSize": 0 })).tab_size, 1);
        assert_eq!(validate(json!({ "tabSize": 500 })).tab_size, 100);
        assert_eq!(validate(json!({ "tabSize": "2" })).tab_size, 2);
    }

    #[test]
    fn insert_spaces_and_detect_indentation_accept_the_string_false() {
        let v = validate(json!({ "insertSpaces": "false", "detectIndentation": false }));
        assert!(!v.insert_spaces);
        assert!(!v.detect_indentation);
    }
}
