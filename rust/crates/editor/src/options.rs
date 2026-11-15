//! Option values: validation of raw settings, and computed values.
//!
//! Ported from `src/vs/editor/common/config/editorOptions.ts`:
//! - the validators of the options M1 uses (`EditorBooleanOption`, `EditorIntOption`,
//!   `EditorFloatOption`, `EditorStringOption`, `EditorStringEnumOption`, `EditorEnumOption`,
//!   `EditorFontSize`, `EditorLineHeight`, `EditorRenderLineNumbersOption`);
//! - `EditorOptionsUtil.validateOptions` and `computeOptions`, as [`ValidatedEditorOptions`]
//!   and [`ComputedEditorOptions`].
//!
//! Each typed field corresponds to one [`EditorOption`](crate::option::EditorOption). The TypeScript
//! `get(id)` lookup is replaced by typed fields. The default for `fontSize` depends on the
//! platform, which is a parameter. The migrations of `migrateOptions.ts` for these options
//! (boolean values of `wordWrap`, `lineNumbers`, `renderWhitespace`, and the `visible` value of
//! `cursorBlinking`) are applied in the validators, so the result is the same.
//!
//! Tests are named after the upstream cases they port (see each module).

use serde_json::{Map, Value};

use crate::environment::{Environment, Platform};
use crate::font;
use crate::option::EditorOption;
use crate::value::{boolean, clamped_int, float_or, string_or, string_set};

/// The default word separators (`USUAL_WORD_SEPARATORS` in `src/vs/editor/common/core/wordHelper.ts`).
pub const USUAL_WORD_SEPARATORS: &str = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";

/// The largest value of an integer option (`Constants.MAX_SAFE_SMALL_INTEGER`).
const MAX_SAFE_SMALL_INTEGER: i64 = 1 << 30;

/// `editor.autoClosingBrackets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosingBrackets {
    /// Always close brackets.
    Always,
    /// Close brackets as the language configuration says.
    LanguageDefined,
    /// Close brackets only when the cursor is before whitespace.
    BeforeWhitespace,
    /// Never close brackets.
    Never,
}

const AUTO_CLOSING_BRACKETS: [(&str, AutoClosingBrackets); 4] = [
    ("always", AutoClosingBrackets::Always),
    ("languageDefined", AutoClosingBrackets::LanguageDefined),
    ("beforeWhitespace", AutoClosingBrackets::BeforeWhitespace),
    ("never", AutoClosingBrackets::Never),
];

/// `editor.cursorBlinking`. The hidden style is not a setting value and is left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorBlinkingStyle {
    /// Blink.
    Blink,
    /// Blink with a smooth fade.
    Smooth,
    /// Blink with a prolonged filled state and a smooth fade.
    Phase,
    /// Expand and collapse on the y axis.
    Expand,
    /// Do not blink.
    Solid,
}

const CURSOR_BLINKING: [(&str, CursorBlinkingStyle); 6] = [
    ("blink", CursorBlinkingStyle::Blink),
    ("smooth", CursorBlinkingStyle::Smooth),
    ("phase", CursorBlinkingStyle::Phase),
    ("expand", CursorBlinkingStyle::Expand),
    ("solid", CursorBlinkingStyle::Solid),
    // Migration of the old `visible` value (`migrateOptions.ts`).
    ("visible", CursorBlinkingStyle::Solid),
];

/// `editor.cursorStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// A vertical line between two characters.
    Line,
    /// A block over a character.
    Block,
    /// A horizontal line under a character.
    Underline,
    /// A thin vertical line between two characters.
    LineThin,
    /// An outlined block over a character.
    BlockOutline,
    /// A thin horizontal line under a character.
    UnderlineThin,
}

const CURSOR_STYLE: [(&str, CursorStyle); 6] = [
    ("line", CursorStyle::Line),
    ("block", CursorStyle::Block),
    ("underline", CursorStyle::Underline),
    ("line-thin", CursorStyle::LineThin),
    ("block-outline", CursorStyle::BlockOutline),
    ("underline-thin", CursorStyle::UnderlineThin),
];

/// `editor.lineNumbers`. The custom renderer of the TypeScript option is a function, so it has no
/// settings value and is not represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumbers {
    /// No line numbers.
    Off,
    /// Absolute line numbers.
    On,
    /// Distance from the cursor line.
    Relative,
    /// Numbers every ten lines.
    Interval,
}

/// `editor.multiCursorModifier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiCursorModifier {
    /// Control on Windows and Linux, Command on macOS.
    CtrlCmd,
    /// Alt on Windows and Linux, Option on macOS.
    Alt,
}

/// A keyboard modifier, as used by mouse gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    /// The Alt key, or Option on macOS.
    Alt,
    /// The Control key.
    Ctrl,
    /// The Meta key: Command on macOS.
    Meta,
}

impl MultiCursorModifier {
    /// The key that adds cursors with the mouse on `platform`
    /// (`_multiCursorModifierFromString`).
    pub fn key(self, platform: Platform) -> ModifierKey {
        match self {
            MultiCursorModifier::CtrlCmd if platform == Platform::MacOs => ModifierKey::Meta,
            MultiCursorModifier::CtrlCmd => ModifierKey::Ctrl,
            MultiCursorModifier::Alt => ModifierKey::Alt,
        }
    }
}

/// `editor.renderWhitespace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderWhitespace {
    /// Render no whitespace.
    None,
    /// Render whitespace except single spaces between words.
    Boundary,
    /// Render whitespace only in the selection.
    Selection,
    /// Render only trailing whitespace.
    Trailing,
    /// Render all whitespace.
    All,
}

const RENDER_WHITESPACE: [(&str, RenderWhitespace); 5] = [
    ("none", RenderWhitespace::None),
    ("boundary", RenderWhitespace::Boundary),
    ("selection", RenderWhitespace::Selection),
    ("trailing", RenderWhitespace::Trailing),
    ("all", RenderWhitespace::All),
];

/// `editor.wordWrap`. The wrapping column of `bounded` and `wordWrapColumn` comes from
/// [`ValidatedEditorOptions::word_wrap_column`]. The layout that decides the column of `on` is
/// not ported yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrap {
    /// Lines never wrap.
    Off,
    /// Lines wrap at the viewport width.
    On,
    /// Lines wrap at `wordWrapColumn`.
    WordWrapColumn,
    /// Lines wrap at the smaller of the viewport width and `wordWrapColumn`.
    Bounded,
}

const WORD_WRAP: [(&str, WordWrap); 4] = [
    ("off", WordWrap::Off),
    ("on", WordWrap::On),
    ("wordWrapColumn", WordWrap::WordWrapColumn),
    ("bounded", WordWrap::Bounded),
];

/// Every validated setting, with defaults applied (`ValidatedEditorOptions`).
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedEditorOptions {
    /// `autoClosingBrackets`.
    pub auto_closing_brackets: AutoClosingBrackets,
    /// `cursorBlinking`.
    pub cursor_blinking: CursorBlinkingStyle,
    /// `cursorStyle`.
    pub cursor_style: CursorStyle,
    /// `fontSize` in pixels, before zoom.
    pub font_size: f64,
    /// `lineHeight`. 0 means computed from the font size; values below 8 are multiples of it.
    pub line_height: f64,
    /// `lineNumbers`.
    pub line_numbers: LineNumbers,
    /// `multiCursorModifier`.
    pub multi_cursor_modifier: MultiCursorModifier,
    /// `readOnly`.
    pub read_only: bool,
    /// `renderWhitespace`.
    pub render_whitespace: RenderWhitespace,
    /// `scrollBeyondLastLine`.
    pub scroll_beyond_last_line: bool,
    /// `wordSeparators`.
    pub word_separators: String,
    /// `wordWrap`.
    pub word_wrap: WordWrap,
    /// `wordWrapColumn`, between 1 and 2^30.
    pub word_wrap_column: u32,
}

impl ValidatedEditorOptions {
    /// Validates raw `editor.*` settings (`EditorOptionsUtil.validateOptions`). A missing or
    /// invalid value gives its default.
    pub fn validate(raw: &Map<String, Value>, platform: Platform) -> Self {
        let get = |id: EditorOption| raw.get(id.name());
        let str_of = |id: EditorOption| get(id).and_then(Value::as_str);

        let font_size = match float_or(
            get(EditorOption::FontSize),
            font::default_font_size(platform),
        ) {
            // `EditorFontSize.validate`: 0 means "use the default".
            0.0 => font::default_font_size(platform),
            size => size.clamp(6.0, 100.0),
        };

        Self {
            auto_closing_brackets: string_set(
                str_of(EditorOption::AutoClosingBrackets),
                AutoClosingBrackets::LanguageDefined,
                &AUTO_CLOSING_BRACKETS,
            ),
            cursor_blinking: string_set(
                str_of(EditorOption::CursorBlinking),
                CursorBlinkingStyle::Blink,
                &CURSOR_BLINKING,
            ),
            cursor_style: string_set(
                str_of(EditorOption::CursorStyle),
                CursorStyle::Line,
                &CURSOR_STYLE,
            ),
            font_size,
            line_height: float_or(get(EditorOption::LineHeight), 0.0).clamp(0.0, 150.0),
            line_numbers: validate_line_numbers(get(EditorOption::LineNumbers)),
            multi_cursor_modifier: string_set(
                str_of(EditorOption::MultiCursorModifier),
                MultiCursorModifier::Alt,
                &[
                    ("ctrlCmd", MultiCursorModifier::CtrlCmd),
                    ("alt", MultiCursorModifier::Alt),
                ],
            ),
            read_only: boolean(get(EditorOption::ReadOnly), false),
            render_whitespace: validate_render_whitespace(get(EditorOption::RenderWhitespace)),
            scroll_beyond_last_line: boolean(get(EditorOption::ScrollBeyondLastLine), true),
            word_separators: string_or(get(EditorOption::WordSeparators), USUAL_WORD_SEPARATORS),
            word_wrap: validate_word_wrap(get(EditorOption::WordWrap)),
            word_wrap_column: clamped_int(
                get(EditorOption::WordWrapColumn),
                80,
                1,
                MAX_SAFE_SMALL_INTEGER,
            ) as u32,
        }
    }
}

/// Validated settings with the values that depend on the environment computed
/// (`ComputedEditorOptions`).
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedEditorOptions {
    /// The validated settings.
    pub values: ValidatedEditorOptions,
    /// The font size in pixels, with zoom applied (`fontSize` after `compute`).
    pub font_size: f64,
    /// The line height in pixels, rounded and with zoom applied (`lineHeight` after `compute`).
    pub line_height: f64,
}

impl ComputedEditorOptions {
    /// Computes the options for `values` in `env` (`EditorOptionsUtil.computeOptions`).
    pub fn compute(values: ValidatedEditorOptions, env: &Environment) -> Self {
        let metrics = font::bare_font_metrics(values.font_size, values.line_height, env);
        Self {
            values,
            font_size: metrics.font_size,
            line_height: metrics.line_height,
        }
    }
}

/// `lineNumbers`: an absent value gives `On`. Booleans are the migrated old values. Any other
/// string, or any other type, gives `Off`, as the TypeScript validator does.
fn validate_line_numbers(value: Option<&Value>) -> LineNumbers {
    match value {
        None => LineNumbers::On,
        Some(Value::Bool(true)) => LineNumbers::On,
        Some(Value::String(s)) => match s.as_str() {
            "interval" => LineNumbers::Interval,
            "relative" => LineNumbers::Relative,
            "on" => LineNumbers::On,
            _ => LineNumbers::Off,
        },
        Some(_) => LineNumbers::Off,
    }
}

/// `renderWhitespace` with the migration of the old boolean values.
fn validate_render_whitespace(value: Option<&Value>) -> RenderWhitespace {
    let name = match value {
        Some(Value::Bool(true)) => Some("boundary"),
        Some(Value::Bool(false)) => Some("none"),
        other => other.and_then(Value::as_str),
    };
    string_set(name, RenderWhitespace::Selection, &RENDER_WHITESPACE)
}

/// `wordWrap` with the migration of the old boolean values.
fn validate_word_wrap(value: Option<&Value>) -> WordWrap {
    let name = match value {
        Some(Value::Bool(true)) => Some("on"),
        Some(Value::Bool(false)) => Some("off"),
        other => other.and_then(Value::as_str),
    };
    string_set(name, WordWrap::Off, &WORD_WRAP)
}

#[cfg(test)]
mod tests {
    //! Cases follow `src/vs/editor/test/browser/config/editorConfiguration.test.ts` (the
    //! validation part of the `wordWrap` and `wordWrapColumn` cases) and the value migrations
    //! of `migrateOptions.test.ts`. The wrapping-column cases that need the layout are not
    //! ported yet.

    use super::*;
    use serde_json::json;

    fn validate(settings: Value, platform: Platform) -> ValidatedEditorOptions {
        let raw = settings.as_object().cloned().unwrap_or_default();
        ValidatedEditorOptions::validate(&raw, platform)
    }

    fn linux(settings: Value) -> ValidatedEditorOptions {
        validate(settings, Platform::Linux)
    }

    #[test]
    fn defaults_when_nothing_is_set() {
        let v = linux(json!({}));
        assert_eq!(
            v.auto_closing_brackets,
            AutoClosingBrackets::LanguageDefined
        );
        assert_eq!(v.cursor_blinking, CursorBlinkingStyle::Blink);
        assert_eq!(v.cursor_style, CursorStyle::Line);
        assert_eq!(v.font_size, 14.0);
        assert_eq!(v.line_height, 0.0);
        assert_eq!(v.line_numbers, LineNumbers::On);
        assert_eq!(v.multi_cursor_modifier, MultiCursorModifier::Alt);
        assert!(!v.read_only);
        assert_eq!(v.render_whitespace, RenderWhitespace::Selection);
        assert!(v.scroll_beyond_last_line);
        assert_eq!(v.word_separators, USUAL_WORD_SEPARATORS);
        assert_eq!(v.word_wrap, WordWrap::Off);
        assert_eq!(v.word_wrap_column, 80);
        assert_eq!(validate(json!({}), Platform::MacOs).font_size, 12.0);
    }

    // editorConfiguration.test.ts: 'wordWrap compat false' / 'wordWrap compat true'
    #[test]
    fn word_wrap_compat_false() {
        assert_eq!(linux(json!({ "wordWrap": false })).word_wrap, WordWrap::Off);
    }

    #[test]
    fn word_wrap_compat_true() {
        assert_eq!(linux(json!({ "wordWrap": true })).word_wrap, WordWrap::On);
    }

    // editorConfiguration.test.ts: 'wordWrap on'
    #[test]
    fn word_wrap_on() {
        assert_eq!(linux(json!({ "wordWrap": "on" })).word_wrap, WordWrap::On);
    }

    // editorConfiguration.test.ts: 'wordWrap wordWrapColumn uses wordWrapColumn'
    #[test]
    fn word_wrap_column_is_used_as_given() {
        let v = linux(json!({ "wordWrap": "wordWrapColumn", "wordWrapColumn": 100 }));
        assert_eq!(v.word_wrap, WordWrap::WordWrapColumn);
        assert_eq!(v.word_wrap_column, 100);
    }

    // editorConfiguration.test.ts: 'wordWrap wordWrapColumn validates wordWrapColumn'
    #[test]
    fn word_wrap_column_validates_to_minimum_one() {
        let v = linux(json!({ "wordWrap": "wordWrapColumn", "wordWrapColumn": -1 }));
        assert_eq!(v.word_wrap, WordWrap::WordWrapColumn);
        assert_eq!(v.word_wrap_column, 1);
    }

    // editorConfiguration.test.ts: 'wordWrap bounded validates wordWrapColumn'
    #[test]
    fn bounded_word_wrap_validates_word_wrap_column() {
        let v = linux(json!({ "wordWrap": "bounded", "wordWrapColumn": -1 }));
        assert_eq!(v.word_wrap, WordWrap::Bounded);
        assert_eq!(v.word_wrap_column, 1);
    }

    // migrateOptions.test.ts: 'wordWrap', 'lineNumbers', 'cursorBlinking', 'renderWhitespace'
    #[test]
    fn line_numbers_migrates_booleans() {
        assert_eq!(
            linux(json!({ "lineNumbers": true })).line_numbers,
            LineNumbers::On
        );
        assert_eq!(
            linux(json!({ "lineNumbers": false })).line_numbers,
            LineNumbers::Off
        );
    }

    #[test]
    fn line_numbers_invalid_strings_are_off() {
        assert_eq!(
            linux(json!({ "lineNumbers": "bogus" })).line_numbers,
            LineNumbers::Off
        );
        assert_eq!(
            linux(json!({ "lineNumbers": 3 })).line_numbers,
            LineNumbers::Off
        );
        assert_eq!(
            linux(json!({ "lineNumbers": "relative" })).line_numbers,
            LineNumbers::Relative
        );
        assert_eq!(
            linux(json!({ "lineNumbers": "interval" })).line_numbers,
            LineNumbers::Interval
        );
    }

    #[test]
    fn cursor_blinking_migrates_visible_to_solid() {
        assert_eq!(
            linux(json!({ "cursorBlinking": "visible" })).cursor_blinking,
            CursorBlinkingStyle::Solid
        );
        assert_eq!(
            linux(json!({ "cursorBlinking": "bogus" })).cursor_blinking,
            CursorBlinkingStyle::Blink
        );
    }

    #[test]
    fn render_whitespace_migrates_booleans() {
        assert_eq!(
            linux(json!({ "renderWhitespace": true })).render_whitespace,
            RenderWhitespace::Boundary
        );
        assert_eq!(
            linux(json!({ "renderWhitespace": false })).render_whitespace,
            RenderWhitespace::None
        );
        assert_eq!(
            linux(json!({ "renderWhitespace": "all" })).render_whitespace,
            RenderWhitespace::All
        );
    }

    #[test]
    fn auto_closing_brackets_and_cursor_style_map_names() {
        let v = linux(json!({ "autoClosingBrackets": "never", "cursorStyle": "block-outline" }));
        assert_eq!(v.auto_closing_brackets, AutoClosingBrackets::Never);
        assert_eq!(v.cursor_style, CursorStyle::BlockOutline);
    }

    #[test]
    fn boolean_options_accept_the_string_false() {
        let v = linux(json!({ "readOnly": "false", "scrollBeyondLastLine": "false" }));
        assert!(!v.read_only);
        assert!(!v.scroll_beyond_last_line);
    }

    #[test]
    fn multi_cursor_modifier_maps_to_platform_keys() {
        let v = validate(json!({ "multiCursorModifier": "ctrlCmd" }), Platform::MacOs);
        assert_eq!(
            v.multi_cursor_modifier.key(Platform::MacOs),
            ModifierKey::Meta
        );
        assert_eq!(
            v.multi_cursor_modifier.key(Platform::Windows),
            ModifierKey::Ctrl
        );
        assert_eq!(
            MultiCursorModifier::Alt.key(Platform::MacOs),
            ModifierKey::Alt
        );
    }

    // The fontSize validator: 0 gives the default, values are clamped to 6..=100.
    #[test]
    fn font_size_zero_gives_the_default() {
        assert_eq!(linux(json!({ "fontSize": 0 })).font_size, 14.0);
        assert_eq!(linux(json!({ "fontSize": "0" })).font_size, 14.0);
        assert_eq!(
            validate(json!({ "fontSize": 0 }), Platform::MacOs).font_size,
            12.0
        );
    }

    #[test]
    fn font_size_is_clamped_and_parsed() {
        assert_eq!(linux(json!({ "fontSize": 3 })).font_size, 6.0);
        assert_eq!(linux(json!({ "fontSize": 200 })).font_size, 100.0);
        assert_eq!(linux(json!({ "fontSize": "20px" })).font_size, 20.0);
        assert_eq!(linux(json!({ "fontSize": "big" })).font_size, 14.0);
    }

    #[test]
    fn line_height_is_clamped_to_zero_to_150() {
        assert_eq!(linux(json!({ "lineHeight": -4 })).line_height, 0.0);
        assert_eq!(linux(json!({ "lineHeight": 400 })).line_height, 150.0);
        assert_eq!(linux(json!({ "lineHeight": 1.5 })).line_height, 1.5);
    }

    #[test]
    fn computed_options_apply_font_metrics() {
        let env = Environment {
            platform: Platform::Windows,
            zoom_level: 0.0,
        };
        let computed = ComputedEditorOptions::compute(linux(json!({ "fontSize": 14 })), &env);
        assert_eq!(computed.font_size, 14.0);
        assert_eq!(computed.line_height, 19.0);
        assert_eq!(computed.values.font_size, 14.0);
    }
}
