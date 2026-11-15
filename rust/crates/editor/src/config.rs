//! Editor configuration: reads `editor.*` settings and reports which options changed.
//!
//! Ported from `src/vs/editor/browser/config/editorConfiguration.ts`, the parts that do not touch
//! the DOM: `EditorConfiguration.updateOptions` and `_recomputeOptions`, and
//! `EditorOptionsUtil.applyUpdate` (as [`apply_update`]). The change check is
//! [`ConfigurationChangedEvent::between`]. The container size, font measurement, accessibility
//! mode and tab focus are inputs that the view supplies through [`Environment`]; wiring them is
//! not done here.
//!
//! Settings come from the `vscode-api` configuration model. `[language]` overrides are applied
//! with [`ConfigurationModel::override_for`], as the TypeScript code does for an editor's language.
//!
//! Tests follow `src/vs/editor/test/browser/config/editorConfiguration.test.ts`
//! ('does not emit event when nothing changes', 'issue #151926: Untyped editor options apply').

use serde_json::{Map, Value};
use vscode_api::configuration::ConfigurationModel;

use crate::environment::Environment;
use crate::option::ConfigurationChangedEvent;
use crate::options::{ComputedEditorOptions, ValidatedEditorOptions};

/// The `editor.*` settings of a configuration model, as the raw option map.
pub fn editor_settings(model: &ConfigurationModel) -> Map<String, Value> {
    model
        .get_value(Some("editor"))
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// The raw settings and the computed options of one editor.
#[derive(Debug, Clone)]
pub struct EditorConfiguration {
    raw: Map<String, Value>,
    environment: Environment,
    options: ComputedEditorOptions,
}

impl EditorConfiguration {
    /// Creates the configuration from raw `editor.*` settings.
    pub fn new(raw: Map<String, Value>, environment: Environment) -> Self {
        let options = compute(&raw, environment);
        Self {
            raw,
            environment,
            options,
        }
    }

    /// Creates the configuration from a configuration model. For a language, the `[language]`
    /// overrides are applied over the base settings.
    pub fn from_configuration(
        model: &ConfigurationModel,
        language: Option<&str>,
        environment: Environment,
    ) -> Self {
        let settings = match language {
            Some(language) => model.override_for(language),
            None => model.clone(),
        };
        Self::new(editor_settings(&settings), environment)
    }

    /// The computed options.
    pub fn options(&self) -> &ComputedEditorOptions {
        &self.options
    }

    /// The raw settings, with every update applied.
    pub fn raw_options(&self) -> &Map<String, Value> {
        &self.raw
    }

    /// The host inputs the options are computed for.
    pub fn environment(&self) -> Environment {
        self.environment
    }

    /// Applies `update` over the raw settings and recomputes (`updateOptions`). Returns the change,
    /// or `None` when nothing changed.
    pub fn update_options(
        &mut self,
        update: &Map<String, Value>,
    ) -> Option<ConfigurationChangedEvent> {
        if !apply_update(&mut self.raw, update) {
            return None;
        }
        self.recompute()
    }

    /// Replaces the host inputs and recomputes (`_recomputeOptions`). Returns the change, or
    /// `None` when nothing changed.
    pub fn set_environment(
        &mut self,
        environment: Environment,
    ) -> Option<ConfigurationChangedEvent> {
        self.environment = environment;
        self.recompute()
    }

    fn recompute(&mut self) -> Option<ConfigurationChangedEvent> {
        let options = compute(&self.raw, self.environment);
        let event = ConfigurationChangedEvent::between(&self.options, &options);
        self.options = options;
        event
    }
}

fn compute(raw: &Map<String, Value>, environment: Environment) -> ComputedEditorOptions {
    let values = ValidatedEditorOptions::validate(raw, environment.platform);
    ComputedEditorOptions::compute(values, &environment)
}

/// Merges the keys of `update` into `target` (`EditorOptionsUtil.applyUpdate`). Objects merge key
/// by key. Other values, including arrays, are replaced when they differ. Returns whether
/// anything changed.
pub fn apply_update(target: &mut Map<String, Value>, update: &Map<String, Value>) -> bool {
    let mut changed = false;
    for (key, value) in update {
        changed |= apply_key(target, key, value);
    }
    changed
}

fn apply_key(target: &mut Map<String, Value>, key: &str, update: &Value) -> bool {
    match (target.get_mut(key), update) {
        (Some(Value::Object(current)), Value::Object(update)) => {
            let mut changed = false;
            for (inner_key, inner_value) in update {
                changed |= apply_key(current, inner_key, inner_value);
            }
            changed
        }
        (Some(current), update) if *current == *update => false,
        _ => {
            target.insert(key.to_owned(), update.clone());
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Platform;
    use crate::option::EditorOption;
    use serde_json::json;

    fn linux() -> Environment {
        Environment {
            platform: Platform::Linux,
            zoom_level: 0.0,
        }
    }

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    // editorConfiguration.test.ts: 'does not emit event when nothing changes'
    #[test]
    fn does_not_emit_event_when_nothing_changes() {
        let mut config = EditorConfiguration::new(object(json!({ "readOnly": true })), linux());
        assert!(config.options().values.read_only);

        assert_eq!(
            config.update_options(&object(json!({ "readOnly": true }))),
            None
        );
        assert_eq!(
            config.update_options(&object(json!({ "wordWrap": "off" }))),
            None
        );

        let event = config
            .update_options(&object(json!({ "readOnly": false })))
            .expect("read only changed");
        assert!(event.has_changed(EditorOption::ReadOnly));
        assert!(!event.has_changed(EditorOption::WordWrap));
    }

    // editorConfiguration.test.ts: 'issue #151926: Untyped editor options apply'. The values are
    // untyped JSON, and the validators coerce them.
    #[test]
    fn untyped_updates_are_validated() {
        let mut config = EditorConfiguration::new(object(json!({})), linux());
        config.update_options(&object(json!({ "fontSize": "20", "lineNumbers": false })));
        assert_eq!(config.options().values.font_size, 20.0);
        assert_eq!(
            config.options().values.line_numbers,
            crate::options::LineNumbers::Off
        );
    }

    #[test]
    fn zoom_changes_the_computed_font_options() {
        let mut config = EditorConfiguration::new(object(json!({ "fontSize": 14 })), linux());
        let event = config
            .set_environment(Environment {
                platform: Platform::Linux,
                zoom_level: 1.0,
            })
            .expect("zoom changed");
        assert!(event.has_changed(EditorOption::FontSize));
        assert!(event.has_changed(EditorOption::LineHeight));
        assert!(!event.has_changed(EditorOption::ReadOnly));
        assert!((config.options().font_size - 15.4).abs() < 1e-9);
    }

    #[test]
    fn apply_update_merges_objects_and_reports_changes() {
        let mut target = object(json!({ "a": { "b": 1, "c": 2 }, "d": [1] }));

        assert!(!apply_update(
            &mut target,
            &object(json!({ "a": { "b": 1 }, "d": [1] }))
        ));
        assert!(apply_update(
            &mut target,
            &object(json!({ "a": { "b": 3 } }))
        ));
        assert_eq!(target, object(json!({ "a": { "b": 3, "c": 2 }, "d": [1] })));

        assert!(apply_update(&mut target, &object(json!({ "d": [2] }))));
        assert!(apply_update(&mut target, &object(json!({ "a": "flat" }))));
        assert_eq!(target, object(json!({ "a": "flat", "d": [2] })));
    }

    #[test]
    fn language_overrides_apply_over_the_base_settings() {
        let (model, conflicts) = ConfigurationModel::parse(&object(json!({
            "editor.wordWrap": "off",
            "[typescript]": { "editor.wordWrap": "on" }
        })));
        assert!(conflicts.is_empty());

        let base = EditorConfiguration::from_configuration(&model, None, linux());
        assert_eq!(
            base.options().values.word_wrap,
            crate::options::WordWrap::Off
        );

        let typescript =
            EditorConfiguration::from_configuration(&model, Some("typescript"), linux());
        assert_eq!(
            typescript.options().values.word_wrap,
            crate::options::WordWrap::On
        );
    }
}
