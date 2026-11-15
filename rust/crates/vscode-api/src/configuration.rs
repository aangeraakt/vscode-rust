//! Configuration model: dotted keys, value trees, merging, and `[language]` overrides.
//!
//! Ported from:
//! - `src/vs/platform/configuration/common/configurationModels.ts` (`ConfigurationModel`:
//!   `getValue`, `merge`, `override`, `addValue`, `setValue`, `removeValue`;
//!   `ConfigurationModelParser.doParseRaw` / `toOverrides` without registry filtering)
//! - `src/vs/platform/configuration/common/configuration.ts` (`ConfigurationTarget`,
//!   `toValuesTree`, `addToValueTree`, `removeFromValueTree`, `getConfigurationValue`)
//! - `src/vs/platform/configuration/common/configurationRegistry.ts`
//!   (`OVERRIDE_PROPERTY_REGEX`, `overrideIdentifiersFromKey`)
//!
//! Tests follow `src/vs/platform/configuration/test/common/configurationModels.test.ts`.
//! Filtering raw settings against the configuration registry (scopes, restricted and
//! unregistered keys) belongs to the `settings` crate, which feeds the filtered map to
//! [`ConfigurationModel::parse`].
//!
//! Divergences: when merging an override into the base, TypeScript merges two *arrays*
//! index by index (a side effect of `typeof [] === 'object'`). Here only two objects are merged
//! and anything else is replaced. Conflicts that TypeScript logs are returned to the caller.

use serde_json::{Map, Value};

/// A JSON object, the shape of every configuration tree.
pub type Object = Map<String, Value>;

/// Where a configuration value comes from. Numeric values match VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigurationTarget {
    /// Application-wide settings, shared by all profiles.
    Application = 1,
    /// User settings.
    User = 2,
    /// User settings stored locally.
    UserLocal = 3,
    /// User settings stored on the remote.
    UserRemote = 4,
    /// Workspace settings.
    Workspace = 5,
    /// Settings of one folder in a multi-root workspace.
    WorkspaceFolder = 6,
    /// Registered defaults.
    Default = 7,
    /// In-memory values.
    Memory = 8,
}

/// The contents of one `"[id1][id2]": { ... }` block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigurationOverride {
    /// Override identifiers (usually language ids), in key order.
    pub identifiers: Vec<String>,
    /// The dotted keys set inside the block.
    pub keys: Vec<String>,
    /// The block as a value tree.
    pub contents: Object,
}

/// A configuration layer: a value tree, the dotted keys that produced it, and override blocks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigurationModel {
    contents: Object,
    keys: Vec<String>,
    overrides: Vec<ConfigurationOverride>,
}

/// Whether `key` is an override key such as `[typescript]` or `[js][ts]`
/// (`OVERRIDE_PROPERTY_REGEX` = `^(\[([^\]]+)\])+$`).
pub fn is_override_key(key: &str) -> bool {
    let mut rest = key;
    if rest.is_empty() {
        return false;
    }
    while !rest.is_empty() {
        let Some(inner) = rest.strip_prefix('[') else {
            return false;
        };
        match inner.find(']') {
            Some(0) | None => return false,
            Some(end) => rest = &inner[end + 1..],
        }
    }
    true
}

/// The distinct, trimmed, non-empty identifiers of an override key (`overrideIdentifiersFromKey`).
pub fn override_identifiers_from_key(key: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if is_override_key(key) {
        for part in key.split('[').skip(1) {
            let id = part.trim_end_matches(']').trim();
            if !id.is_empty() && !out.iter().any(|o| o == id) {
                out.push(id.to_owned());
            }
        }
    }
    out
}

/// Sets `key` (dotted) in `root`, creating intermediate objects (`addToValueTree`). If a prefix
/// of the key holds a non-object value, nothing is set and the conflict is returned.
pub fn add_to_value_tree(root: &mut Object, key: &str, value: Value) -> Result<(), String> {
    let segments: Vec<&str> = key.split('.').collect();
    let (last, parents) = segments
        .split_last()
        .expect("split yields at least one segment");
    let mut curr = root;
    for (i, segment) in parents.iter().enumerate() {
        let entry = curr
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Object::new()));
        curr = match entry {
            Value::Object(obj) => obj,
            other => {
                return Err(format!(
                    "Ignoring {key} as {} is {other}",
                    segments[..=i].join(".")
                ));
            }
        };
    }
    curr.insert((*last).to_owned(), value);
    Ok(())
}

/// Builds a value tree from a map of dotted keys (`toValuesTree`). Conflicts are collected.
pub fn to_values_tree(properties: &Object, conflicts: &mut Vec<String>) -> Object {
    let mut root = Object::new();
    for (key, value) in properties {
        if let Err(e) = add_to_value_tree(&mut root, key, value.clone()) {
            conflicts.push(e);
        }
    }
    root
}

/// Removes `key` (dotted) and prunes objects left empty (`removeFromValueTree`).
pub fn remove_from_value_tree(tree: &mut Object, key: &str) {
    fn go(tree: &mut Object, segments: &[&str]) {
        let (first, rest) = segments.split_first().expect("non-empty");
        if rest.is_empty() {
            tree.shift_remove(*first);
            return;
        }
        if let Some(Value::Object(child)) = tree.get_mut(*first) {
            go(child, rest);
            if child.is_empty() {
                tree.shift_remove(*first);
            }
        }
    }
    go(tree, &key.split('.').collect::<Vec<_>>());
}

/// Looks up a dotted path in a value tree (`getConfigurationValue`).
pub fn get_configuration_value<'a>(tree: &'a Object, path: &str) -> Option<&'a Value> {
    let mut segments = path.split('.');
    let mut current = tree.get(segments.next()?)?;
    for segment in segments {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

/// Deep-merges `source` into `target`: objects merge recursively, everything else is replaced
/// by a copy (`ConfigurationModel.mergeContents`).
fn merge_contents(target: &mut Object, source: &Object) {
    for (key, value) in source {
        if let (Some(Value::Object(t)), Value::Object(s)) = (target.get_mut(key), value) {
            merge_contents(t, s);
            continue;
        }
        target.insert(key.clone(), value.clone());
    }
}

impl ConfigurationModel {
    /// An empty model.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a model from raw settings (top-level dotted keys and `[id]` blocks), as
    /// `ConfigurationModelParser.doParseRaw` does after registry filtering. Returns the conflicts
    /// that TypeScript logs.
    pub fn parse(raw: &Object) -> (Self, Vec<String>) {
        let mut conflicts = Vec::new();
        let contents = to_values_tree(raw, &mut conflicts);
        let keys = raw.keys().cloned().collect();
        let mut overrides = Vec::new();
        for (key, value) in raw {
            if is_override_key(key) {
                let block = value.as_object().cloned().unwrap_or_default();
                overrides.push(ConfigurationOverride {
                    identifiers: override_identifiers_from_key(key),
                    keys: block.keys().cloned().collect(),
                    contents: to_values_tree(&block, &mut conflicts),
                });
            }
        }
        let model = Self {
            contents,
            keys,
            overrides,
        };
        (model, conflicts)
    }

    /// The whole value tree.
    pub fn contents(&self) -> &Object {
        &self.contents
    }

    /// The dotted keys that were set, in insertion order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// The override blocks.
    pub fn overrides(&self) -> &[ConfigurationOverride] {
        &self.overrides
    }

    /// Whether the model has no contents, keys or overrides.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty() && self.keys.is_empty() && self.overrides.is_empty()
    }

    /// The value at a dotted `section`, or the whole tree for `None` (`getValue`).
    pub fn get_value(&self, section: Option<&str>) -> Option<Value> {
        match section {
            None | Some("") => Some(Value::Object(self.contents.clone())),
            Some(s) => get_configuration_value(&self.contents, s).cloned(),
        }
    }

    /// All distinct override identifiers, in order of appearance.
    pub fn override_identifiers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for o in &self.overrides {
            for id in &o.identifiers {
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
        }
        out
    }

    /// The model as seen for `identifier` (usually a language id), with matching override blocks
    /// merged over the base (`override`). Blocks naming only this identifier win over blocks that
    /// name several. Like TypeScript, a top-level override value that is falsy (`false`, `0`, `""`,
    /// `null`) is ignored.
    pub fn override_for(&self, identifier: &str) -> ConfigurationModel {
        let mut merged: Option<Object> = None;
        let mut only_this: Option<&Object> = None;
        for o in &self.overrides {
            if o.identifiers.len() == 1 && o.identifiers[0] == identifier {
                only_this = Some(&o.contents);
            } else if o.identifiers.iter().any(|i| i == identifier) {
                merge_into(&mut merged, &o.contents);
            }
        }
        if let Some(c) = only_this {
            merge_into(&mut merged, c);
        }
        let Some(override_contents) = merged.filter(|m| !m.is_empty()) else {
            return self.clone();
        };

        let mut contents = self.contents.clone();
        for (key, value) in override_contents {
            if is_falsy(&value) {
                continue;
            }
            match (contents.get_mut(&key), &value) {
                (Some(Value::Object(base)), Value::Object(over)) => merge_contents(base, over),
                _ => {
                    contents.insert(key, value);
                }
            }
        }
        ConfigurationModel {
            contents,
            keys: self.keys.clone(),
            overrides: self.overrides.clone(),
        }
    }

    /// A new model with `others` layered over `self`, later models winning (`merge`).
    pub fn merge(&self, others: &[&ConfigurationModel]) -> ConfigurationModel {
        let mut result = self.clone();
        for other in others {
            if other.is_empty() {
                continue;
            }
            merge_contents(&mut result.contents, &other.contents);
            for other_override in &other.overrides {
                match result
                    .overrides
                    .iter_mut()
                    .find(|o| o.identifiers == other_override.identifiers)
                {
                    Some(existing) => {
                        merge_contents(&mut existing.contents, &other_override.contents);
                        for k in &other_override.keys {
                            if !existing.keys.contains(k) {
                                existing.keys.push(k.clone());
                            }
                        }
                    }
                    None => result.overrides.push(other_override.clone()),
                }
            }
            for k in &other.keys {
                if !result.keys.contains(k) {
                    result.keys.push(k.clone());
                }
            }
        }
        result
    }

    /// Sets `key` and records it as a new key even if already present (`addValue`).
    pub fn add_value(&mut self, key: &str, value: Value) -> Result<(), String> {
        self.update_value(key, value, true)
    }

    /// Sets `key`, recording it only if not yet present (`setValue`).
    pub fn set_value(&mut self, key: &str, value: Value) -> Result<(), String> {
        self.update_value(key, value, false)
    }

    /// Removes `key`, and its override block if `key` is an override key (`removeValue`).
    pub fn remove_value(&mut self, key: &str) {
        let Some(index) = self.keys.iter().position(|k| k == key) else {
            return;
        };
        self.keys.remove(index);
        remove_from_value_tree(&mut self.contents, key);
        if is_override_key(key) {
            let ids = override_identifiers_from_key(key);
            if let Some(i) = self.overrides.iter().position(|o| o.identifiers == ids) {
                self.overrides.remove(i);
            }
        }
    }

    fn update_value(&mut self, key: &str, value: Value, add: bool) -> Result<(), String> {
        add_to_value_tree(&mut self.contents, key, value)?;
        if add || !self.keys.iter().any(|k| k == key) {
            self.keys.push(key.to_owned());
        }
        if is_override_key(key) {
            let block = self
                .contents
                .get(key)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let mut conflicts = Vec::new();
            let identifiers = override_identifiers_from_key(key);
            let new = ConfigurationOverride {
                keys: block.keys().cloned().collect(),
                contents: to_values_tree(&block, &mut conflicts),
                identifiers,
            };
            match self
                .overrides
                .iter_mut()
                .find(|o| o.identifiers == new.identifiers)
            {
                Some(existing) => *existing = new,
                None => self.overrides.push(new),
            }
            if let Some(c) = conflicts.into_iter().next() {
                return Err(c);
            }
        }
        Ok(())
    }
}

fn merge_into(acc: &mut Option<Object>, source: &Object) {
    match acc {
        Some(a) => merge_contents(a, source),
        None => *acc = Some(source.clone()),
    }
}

fn is_falsy(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Object {
        v.as_object().unwrap().clone()
    }

    fn model(v: Value) -> ConfigurationModel {
        let (m, conflicts) = ConfigurationModel::parse(&obj(v));
        assert!(conflicts.is_empty(), "{conflicts:?}");
        m
    }

    #[test]
    fn override_keys() {
        assert!(is_override_key("[typescript]"));
        assert!(is_override_key("[js][ts]"));
        assert!(!is_override_key("[]"));
        assert!(!is_override_key("editor.[x]"));
        assert!(!is_override_key(""));
        assert_eq!(override_identifiers_from_key("[a][ b ][a]"), ["a", "b"]);
    }

    #[test]
    fn value_tree_round_trip() {
        let mut tree = Object::new();
        add_to_value_tree(&mut tree, "a.b.c", json!(1)).unwrap();
        add_to_value_tree(&mut tree, "a.d", json!(2)).unwrap();
        assert_eq!(
            Value::Object(tree.clone()),
            json!({"a": {"b": {"c": 1}, "d": 2}})
        );
        assert_eq!(get_configuration_value(&tree, "a.b.c"), Some(&json!(1)));
        assert_eq!(get_configuration_value(&tree, "a.b.x"), None);
        assert_eq!(
            add_to_value_tree(&mut tree, "a.d.e", json!(3)),
            Err("Ignoring a.d.e as a.d is 2".to_owned())
        );
        remove_from_value_tree(&mut tree, "a.b.c");
        assert_eq!(Value::Object(tree), json!({"a": {"d": 2}}));
    }

    #[test]
    fn set_add_remove_values() {
        // configurationModels.test.ts: "setValue for a key that has no sections", "removeValue"
        let mut m = model(json!({"a": {"b": 1}}));
        m.set_value("a.b", json!(2)).unwrap();
        m.set_value("c", json!(3)).unwrap();
        assert_eq!(m.get_value(None), Some(json!({"a": {"b": 2}, "c": 3})));
        assert_eq!(m.keys(), ["a", "a.b", "c"]);
        m.remove_value("c");
        assert_eq!(m.get_value(Some("c")), None);
        assert_eq!(m.keys(), ["a", "a.b"]);
    }

    #[test]
    fn merge_layers_later_wins() {
        // configurationModels.test.ts: "merge"
        let base = model(json!({"a": 1, "b": {"c": 1, "d": 1}}));
        let user = model(json!({"b.c": 2, "e": 3}));
        let merged = base.merge(&[&user]);
        assert_eq!(
            merged.get_value(None),
            Some(json!({"a": 1, "b": {"c": 2, "d": 1}, "e": 3}))
        );
        assert_eq!(merged.keys(), ["a", "b", "b.c", "e"]);
    }

    #[test]
    fn merge_combines_override_blocks() {
        let a = model(json!({"[ts]": {"x": 1}}));
        let b = model(json!({"[ts]": {"y": 2}, "[md]": {"z": 3}}));
        let merged = a.merge(&[&b]);
        assert_eq!(merged.overrides().len(), 2);
        assert_eq!(
            Value::Object(merged.overrides()[0].contents.clone()),
            json!({"x": 1, "y": 2})
        );
        assert_eq!(merged.overrides()[0].keys, ["x", "y"]);
    }

    #[test]
    fn language_override_merges_over_base() {
        // configurationModels.test.ts: "override"
        let m = model(json!({
            "editor.tabSize": 4,
            "editor.wordWrap": "off",
            "[markdown]": {"editor.wordWrap": "on"}
        }));
        let md = m.override_for("markdown");
        assert_eq!(md.get_value(Some("editor.wordWrap")), Some(json!("on")));
        assert_eq!(md.get_value(Some("editor.tabSize")), Some(json!(4)));
        assert_eq!(m.override_for("rust"), m);
        assert_eq!(m.override_identifiers(), ["markdown"]);
    }

    #[test]
    fn single_identifier_block_wins_over_multi() {
        // configurationModels.test.ts: "override when there are multiple language overrides"
        let m = model(json!({
            "a": {"x": 0},
            "[ts]": {"a.x": 1},
            "[js][ts]": {"a.x": 2, "a.y": 2}
        }));
        let ts = m.override_for("ts");
        assert_eq!(ts.get_value(Some("a")), Some(json!({"x": 1, "y": 2})));
        let js = m.override_for("js");
        assert_eq!(js.get_value(Some("a")), Some(json!({"x": 2, "y": 2})));
    }

    #[test]
    fn falsy_top_level_override_is_ignored_like_typescript() {
        let m = model(json!({"flag": true, "[ts]": {"flag": false}}));
        assert_eq!(
            m.override_for("ts").get_value(Some("flag")),
            Some(json!(true))
        );
    }

    #[test]
    fn set_value_of_override_key_updates_block() {
        let mut m = ConfigurationModel::empty();
        m.set_value("[ts]", json!({"editor.tabSize": 2})).unwrap();
        assert_eq!(m.overrides()[0].identifiers, ["ts"]);
        assert_eq!(
            m.override_for("ts").get_value(Some("editor.tabSize")),
            Some(json!(2))
        );
        m.remove_value("[ts]");
        assert!(m.overrides().is_empty());
    }

    #[test]
    fn parse_reports_conflicts() {
        let (m, conflicts) = ConfigurationModel::parse(&obj(json!({"a": 1, "a.b": 2})));
        assert_eq!(conflicts, ["Ignoring a.b as a is 1"]);
        assert_eq!(m.get_value(Some("a")), Some(json!(1)));
    }
}
