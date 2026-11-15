//! Editor option ids and the configuration change event.
//!
//! Ported from `src/vs/editor/common/config/editorOptions.ts`: the `EditorOption` enum (ids),
//! `ConfigurationChangedEvent`, and the change check of `EditorOptionsUtil.checkEquals`.
//!
//! The TypeScript enum has about 130 ids. This crate lists the ones ported so far, in the order
//! they were added. [`EditorOption::name`] is the key under `editor.` in settings.

use crate::options::{ComputedEditorOptions, ValidatedEditorOptions};

/// An editor option. Each variant is one `editor.*` setting, or a computed value derived from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorOption {
    /// `autoClosingBrackets`.
    AutoClosingBrackets,
    /// `cursorBlinking`.
    CursorBlinking,
    /// `cursorStyle`.
    CursorStyle,
    /// `fontSize`. Its computed value is in [`ComputedEditorOptions::font_size`].
    FontSize,
    /// `lineHeight`. Its computed value is in [`ComputedEditorOptions::line_height`].
    LineHeight,
    /// `lineNumbers`.
    LineNumbers,
    /// `multiCursorModifier`.
    MultiCursorModifier,
    /// `readOnly`.
    ReadOnly,
    /// `renderWhitespace`.
    RenderWhitespace,
    /// `scrollBeyondLastLine`.
    ScrollBeyondLastLine,
    /// `wordSeparators`.
    WordSeparators,
    /// `wordWrap`.
    WordWrap,
    /// `wordWrapColumn`.
    WordWrapColumn,
}

impl EditorOption {
    /// Every option, in declaration order. Its index is the option's position in
    /// [`ConfigurationChangedEvent`].
    pub const ALL: [EditorOption; 13] = [
        EditorOption::AutoClosingBrackets,
        EditorOption::CursorBlinking,
        EditorOption::CursorStyle,
        EditorOption::FontSize,
        EditorOption::LineHeight,
        EditorOption::LineNumbers,
        EditorOption::MultiCursorModifier,
        EditorOption::ReadOnly,
        EditorOption::RenderWhitespace,
        EditorOption::ScrollBeyondLastLine,
        EditorOption::WordSeparators,
        EditorOption::WordWrap,
        EditorOption::WordWrapColumn,
    ];

    /// The number of options.
    pub const COUNT: usize = Self::ALL.len();

    /// The setting key under `editor.` (the `name` of the TypeScript option).
    pub const fn name(self) -> &'static str {
        match self {
            EditorOption::AutoClosingBrackets => "autoClosingBrackets",
            EditorOption::CursorBlinking => "cursorBlinking",
            EditorOption::CursorStyle => "cursorStyle",
            EditorOption::FontSize => "fontSize",
            EditorOption::LineHeight => "lineHeight",
            EditorOption::LineNumbers => "lineNumbers",
            EditorOption::MultiCursorModifier => "multiCursorModifier",
            EditorOption::ReadOnly => "readOnly",
            EditorOption::RenderWhitespace => "renderWhitespace",
            EditorOption::ScrollBeyondLastLine => "scrollBeyondLastLine",
            EditorOption::WordSeparators => "wordSeparators",
            EditorOption::WordWrap => "wordWrap",
            EditorOption::WordWrapColumn => "wordWrapColumn",
        }
    }
}

/// Which options changed between two computed option sets (`ConfigurationChangedEvent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationChangedEvent {
    changed: [bool; EditorOption::COUNT],
}

impl ConfigurationChangedEvent {
    /// The event for the change from `old` to `new`, or `None` if no option changed
    /// (`EditorOptionsUtil.checkEquals`).
    pub fn between(old: &ComputedEditorOptions, new: &ComputedEditorOptions) -> Option<Self> {
        let (a, b): (&ValidatedEditorOptions, &ValidatedEditorOptions) = (&old.values, &new.values);
        let differs = [
            (
                EditorOption::AutoClosingBrackets,
                a.auto_closing_brackets != b.auto_closing_brackets,
            ),
            (
                EditorOption::CursorBlinking,
                a.cursor_blinking != b.cursor_blinking,
            ),
            (EditorOption::CursorStyle, a.cursor_style != b.cursor_style),
            (EditorOption::FontSize, old.font_size != new.font_size),
            (EditorOption::LineHeight, old.line_height != new.line_height),
            (EditorOption::LineNumbers, a.line_numbers != b.line_numbers),
            (
                EditorOption::MultiCursorModifier,
                a.multi_cursor_modifier != b.multi_cursor_modifier,
            ),
            (EditorOption::ReadOnly, a.read_only != b.read_only),
            (
                EditorOption::RenderWhitespace,
                a.render_whitespace != b.render_whitespace,
            ),
            (
                EditorOption::ScrollBeyondLastLine,
                a.scroll_beyond_last_line != b.scroll_beyond_last_line,
            ),
            (
                EditorOption::WordSeparators,
                a.word_separators != b.word_separators,
            ),
            (EditorOption::WordWrap, a.word_wrap != b.word_wrap),
            (
                EditorOption::WordWrapColumn,
                a.word_wrap_column != b.word_wrap_column,
            ),
        ];
        let mut changed = [false; EditorOption::COUNT];
        for (id, differs) in differs {
            changed[id as usize] = differs;
        }
        changed.contains(&true).then_some(Self { changed })
    }

    /// Whether `id` changed (`hasChanged`).
    pub fn has_changed(&self, id: EditorOption) -> bool {
        self.changed[id as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Environment, Platform};
    use serde_json::{Map, json};

    fn computed(settings: Map<String, serde_json::Value>) -> ComputedEditorOptions {
        let values = ValidatedEditorOptions::validate(&settings, Platform::Linux);
        let env = Environment {
            platform: Platform::Linux,
            zoom_level: 0.0,
        };
        ComputedEditorOptions::compute(values, &env)
    }

    #[test]
    fn ids_are_unique_and_named() {
        let mut names: Vec<&str> = EditorOption::ALL.iter().map(|id| id.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), EditorOption::COUNT);
        for (index, id) in EditorOption::ALL.iter().enumerate() {
            assert_eq!(*id as usize, index);
        }
    }

    #[test]
    fn does_not_report_an_event_when_nothing_changes() {
        let a = computed(Map::new());
        let b = computed(Map::new());
        assert_eq!(ConfigurationChangedEvent::between(&a, &b), None);
    }

    #[test]
    fn reports_only_the_options_that_changed() {
        let a = computed(Map::new());
        let mut settings = Map::new();
        settings.insert("wordWrap".into(), json!("on"));
        let b = computed(settings);
        let event = ConfigurationChangedEvent::between(&a, &b).expect("word wrap changed");
        assert!(event.has_changed(EditorOption::WordWrap));
        assert!(!event.has_changed(EditorOption::ReadOnly));
        assert!(!event.has_changed(EditorOption::FontSize));
    }

    #[test]
    fn a_font_size_change_marks_the_computed_font_options() {
        let a = computed(Map::new());
        let mut settings = Map::new();
        settings.insert("fontSize".into(), json!(20));
        let b = computed(settings);
        let event = ConfigurationChangedEvent::between(&a, &b).expect("font size changed");
        assert!(event.has_changed(EditorOption::FontSize));
        assert!(event.has_changed(EditorOption::LineHeight));
    }
}
