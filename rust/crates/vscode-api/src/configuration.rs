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
    if !is_override_key(key) {
        return out;
    }
    // Same scan as `is_override_key`: each `[` opens a group that runs to the next `]`, so
    // `[a[b]` is the single identifier `a[b`, as with OVERRIDE_IDENTIFIER_REGEX.
    let mut rest = key;
    while let Some(inner) = rest.strip_prefix('[') {
        let end = inner.find(']').expect("validated by is_override_key");
        let id = inner[..end].trim();
        if !id.is_empty() && !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
        rest = &inner[end + 1..];
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

    /// Builds a model from its parts, like the TypeScript constructor. `contents` is a value
    /// tree; `keys` are the dotted keys that produced it.
    pub fn new(contents: Object, keys: Vec<String>, overrides: Vec<ConfigurationOverride>) -> Self {
        Self {
            contents,
            keys,
            overrides,
        }
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

    /// The distinct keys set by every block that names `identifier`
    /// (`getKeysForOverrideIdentifier`).
    pub fn keys_for_override_identifier(&self, identifier: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for o in self
            .overrides
            .iter()
            .filter(|o| o.identifiers.iter().any(|i| i == identifier))
        {
            for k in &o.keys {
                if !out.contains(k) {
                    out.push(k.clone());
                }
            }
        }
        out
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

    /// Sets `key` and records it as a new key even if already present (`addValue`). On a
    /// conflict the value is not set, but the key is still recorded and the conflict returned,
    /// as TypeScript logs and continues. The same holds for [`ConfigurationModel::set_value`].
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
        let mut result = add_to_value_tree(&mut self.contents, key, value);
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
            if let (Ok(()), Some(c)) = (&result, conflicts.into_iter().next()) {
                result = Err(c);
            }
        }
        result
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

    fn cm(
        contents: Value,
        keys: &[&str],
        overrides: Vec<ConfigurationOverride>,
    ) -> ConfigurationModel {
        ConfigurationModel::new(
            obj(contents),
            keys.iter().map(|k| k.to_string()).collect(),
            overrides,
        )
    }

    fn ov(ids: &[&str], contents: Value, keys: &[&str]) -> ConfigurationOverride {
        ConfigurationOverride {
            identifiers: ids.iter().map(|k| k.to_string()).collect(),
            keys: keys.iter().map(|k| k.to_string()).collect(),
            contents: obj(contents),
        }
    }

    fn contents(m: &ConfigurationModel) -> Value {
        Value::Object(m.contents().clone())
    }

    #[test]
    fn parse_override_identifiers() {
        // "parse configuration model with single / multiple / multiple duplicate override identifiers"
        let cases = [
            ("[x]", vec!["x"]),
            ("[x][y]", vec!["x", "y"]),
            ("[x][y][x][z]", vec!["x", "y", "z"]),
        ];
        for (key, ids) in cases {
            let m = model(json!({ key: {"a": 1} }));
            assert_eq!(m.overrides(), [ov(&ids, json!({"a": 1}), &["a"])]);
        }
        assert_eq!(override_identifiers_from_key("[a[b]"), ["a[b"]);
    }

    #[test]
    fn set_value_cases() {
        // "setValue for a key that has ..." (all seven cases)
        let cases = [
            (
                json!({"a": {"b": 1}}),
                vec!["a.b"],
                "f",
                1,
                json!({"a": {"b": 1}, "f": 1}),
                vec!["a.b", "f"],
            ),
            (
                json!({"a": {"b": 1}, "f": 1}),
                vec!["a.b", "f"],
                "f",
                3,
                json!({"a": {"b": 1}, "f": 3}),
                vec!["a.b", "f"],
            ),
            (
                json!({"a": {"b": 1}, "f": 1}),
                vec!["a.b", "f"],
                "b.c",
                1,
                json!({"a": {"b": 1}, "f": 1, "b": {"c": 1}}),
                vec!["a.b", "f", "b.c"],
            ),
            (
                json!({"a": {"b": 1}, "b": {"c": 1}, "f": 1}),
                vec!["a.b", "b.c", "f"],
                "b.c",
                3,
                json!({"a": {"b": 1}, "b": {"c": 3}, "f": 1}),
                vec!["a.b", "b.c", "f"],
            ),
            (
                json!({"a": {"b": 1}, "f": 1}),
                vec!["a.b", "f"],
                "a.c",
                1,
                json!({"a": {"b": 1, "c": 1}, "f": 1}),
                vec!["a.b", "f", "a.c"],
            ),
            (
                json!({"a": {"b": 1, "c": 1}, "f": 1}),
                vec!["a.b", "a.c", "f"],
                "a.c",
                3,
                json!({"a": {"b": 1, "c": 3}, "f": 1}),
                vec!["a.b", "a.c", "f"],
            ),
            (
                json!({"a": {"b": {}}, "f": 1}),
                vec!["a.b", "f"],
                "a.b.c",
                1,
                json!({"a": {"b": {"c": 1}}, "f": 1}),
                vec!["a.b", "f", "a.b.c"],
            ),
        ];
        for (start, keys, key, value, expected, expected_keys) in cases {
            let mut m = cm(start, &keys, vec![]);
            m.set_value(key, json!(value)).unwrap();
            assert_eq!(contents(&m), expected, "{key}");
            assert_eq!(m.keys(), expected_keys, "{key}");
        }
    }

    #[test]
    fn remove_value_cases() {
        // "removeValue: remove a non existing key / a single segmented key / a multi segmented key"
        let mut m = cm(json!({"a": {"b": 2}}), &["a.b"], vec![]);
        m.remove_value("a.b.c");
        assert_eq!(contents(&m), json!({"a": {"b": 2}}));
        assert_eq!(m.keys(), ["a.b"]);
        let mut m = cm(json!({"a": 1}), &["a"], vec![]);
        m.remove_value("a");
        assert_eq!(contents(&m), json!({}));
        assert!(m.keys().is_empty());
        let mut m = cm(json!({"a": {"b": 1}}), &["a.b"], vec![]);
        m.remove_value("a.b");
        assert_eq!(contents(&m), json!({}));
        assert!(m.keys().is_empty());
    }

    #[test]
    fn override_cases() {
        // "get overriding configuration ..." cases that Rust can express (an override whose
        // contents is not an object cannot be built here)
        let base = json!({"a": {"b": 1}, "f": 1});
        let m = cm(
            base.clone(),
            &[],
            vec![ov(&["c"], json!({"a": {"d": 1}}), &["a"])],
        );
        assert_eq!(
            contents(&m.override_for("c")),
            json!({"a": {"b": 1, "d": 1}, "f": 1})
        );
        assert_eq!(contents(&m.override_for("xyz")), base);
        let m = cm(
            base.clone(),
            &[],
            vec![ov(&["c"], json!({"a": {"d": 1}, "g": 1}), &["a", "g"])],
        );
        assert_eq!(
            contents(&m.override_for("c")),
            json!({"a": {"b": 1, "d": 1}, "f": 1, "g": 1})
        );
        let m = cm(
            base,
            &[],
            vec![ov(
                &["c"],
                json!({"a": {"d": 1}, "f": {"g": 1}}),
                &["a", "f"],
            )],
        );
        assert_eq!(
            contents(&m.override_for("c")),
            json!({"a": {"b": 1, "d": 1}, "f": {"g": 1}})
        );
        let base = json!({"a": {"b": 1}, "f": {"g": 1}});
        let m = cm(
            base.clone(),
            &[],
            vec![ov(&["c"], json!({"a": {"d": 1}, "f": 1}), &["a", "f"])],
        );
        assert_eq!(
            contents(&m.override_for("c")),
            json!({"a": {"b": 1, "d": 1}, "f": 1})
        );
        let m = cm(base.clone(), &[], vec![ov(&["c"], json!({}), &[])]);
        assert_eq!(contents(&m.override_for("c")), base);
    }

    #[test]
    fn simple_and_recursive_merge() {
        // "simple merge", "recursive merge"
        let base = cm(json!({"a": 1, "b": 2}), &["a", "b"], vec![]);
        let add = cm(json!({"a": 3, "c": 4}), &["a", "c"], vec![]);
        let result = base.merge(&[&add]);
        assert_eq!(contents(&result), json!({"a": 3, "b": 2, "c": 4}));
        assert_eq!(result.keys(), ["a", "b", "c"]);
        let base = cm(json!({"a": {"b": 1}}), &["a.b"], vec![]);
        let add = cm(json!({"a": {"b": 2}}), &["a.b"], vec![]);
        let result = base.merge(&[&add]);
        assert_eq!(contents(&result), json!({"a": {"b": 2}}));
        assert_eq!(result.get_value(Some("a")), Some(json!({"b": 2})));
        assert_eq!(result.keys(), ["a.b"]);
    }

    #[test]
    fn override_with_multiple_identifiers() {
        // "Test override gives all content merged with overrides",
        // "Test override when an override has multiple identifiers",
        // "Test override when an identifier is defined in multiple overrides"
        let m = cm(
            json!({"a": 1, "c": 1}),
            &[],
            vec![ov(&["b"], json!({"a": 2}), &["a"])],
        );
        assert_eq!(contents(&m.override_for("b")), json!({"a": 2, "c": 1}));

        let m = cm(
            json!({"a": 1, "c": 1}),
            &["a", "c"],
            vec![ov(&["x", "y"], json!({"a": 2}), &["a"])],
        );
        for id in ["x", "y"] {
            let actual = m.override_for(id);
            assert_eq!(contents(&actual), json!({"a": 2, "c": 1}));
            assert_eq!(actual.keys(), ["a", "c"]);
            assert_eq!(m.keys_for_override_identifier(id), ["a"]);
        }

        let m = cm(
            json!({"a": 1, "c": 1}),
            &["a", "c"],
            vec![
                ov(&["x"], json!({"a": 3, "b": 1}), &["a", "b"]),
                ov(&["x", "y"], json!({"a": 2}), &["a"]),
            ],
        );
        let actual = m.override_for("x");
        assert_eq!(contents(&actual), json!({"a": 3, "c": 1, "b": 1}));
        assert_eq!(actual.keys(), ["a", "c"]);
        assert_eq!(m.keys_for_override_identifier("x"), ["a", "b"]);
    }

    #[test]
    fn merge_with_multiple_identifiers() {
        // "Test merge when configuration models have multiple identifiers"
        let m = cm(
            json!({"a": 1, "c": 1}),
            &["a", "c"],
            vec![
                ov(&["y"], json!({"c": 1}), &["c"]),
                ov(&["x", "y"], json!({"a": 2}), &["a"]),
            ],
        );
        let target = cm(
            json!({"a": 2, "b": 1}),
            &["a", "b"],
            vec![
                ov(&["x"], json!({"a": 3, "b": 2}), &["a", "b"]),
                ov(&["x", "y"], json!({"b": 3}), &["b"]),
            ],
        );
        let actual = m.merge(&[&target]);
        assert_eq!(contents(&actual), json!({"a": 2, "c": 1, "b": 1}));
        assert_eq!(actual.keys(), ["a", "c", "b"]);
        assert_eq!(
            actual.overrides(),
            [
                ov(&["y"], json!({"c": 1}), &["c"]),
                ov(&["x", "y"], json!({"a": 2, "b": 3}), &["a", "b"]),
                ov(&["x"], json!({"a": 3, "b": 2}), &["a", "b"]),
            ]
        );
    }

    #[test]
    fn conflicting_set_value_still_records_key() {
        let mut m = cm(json!({"a": 1}), &["a"], vec![]);
        assert_eq!(
            m.set_value("a.b", json!(2)),
            Err("Ignoring a.b as a is 1".to_owned())
        );
        assert_eq!(contents(&m), json!({"a": 1}));
        assert_eq!(m.keys(), ["a", "a.b"]);
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
