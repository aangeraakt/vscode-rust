//! Chords, keybindings and the resolved-keybinding contract. Ported from
//! `src/vs/base/common/keybindings.ts`.
//!
//! A keybinding is one or two chords. A chord is a set of modifiers plus either a key code (what
//! the key produces) or a scan code (which physical key it is). Encodings are `u32`: the low 16
//! bits are the first chord, the high 16 bits the second; see [`KeyMod`] for the modifier bits.
//!
//! Upstream's structural `Modifiers` interface is represented by the four modifier fields on each
//! chord type.

use crate::key_codes::{KeyCode, KeyMod, ScanCode};
use crate::platform::OperatingSystem;

/// The low byte of an encoded chord holds the key code.
const KEY_CODE_MASK: u32 = 0x0000_00FF;

/// Decodes a one- or two-chord encoding (upstream `decodeKeybinding`, number form).
///
/// Returns `None` for `0` (no binding), for a key code that is not a [`KeyCode`], and for
/// anything else that would not form a keybinding.
pub fn decode_keybinding(keybinding: u32, os: OperatingSystem) -> Option<Keybinding> {
    if keybinding == 0 {
        return None;
    }
    let first_chord = keybinding & 0x0000_FFFF;
    let second_chord = (keybinding & 0xFFFF_0000) >> 16;
    let mut chords = vec![Chord::KeyCode(create_simple_keybinding(first_chord, os)?)];
    if second_chord != 0 {
        chords.push(Chord::KeyCode(create_simple_keybinding(second_chord, os)?));
    }
    Keybinding::new(chords)
}

/// Decodes a sequence of encoded chords (upstream `decodeKeybinding`, array form).
///
/// Returns `None` for an empty sequence or any chord that does not decode.
pub fn decode_keybinding_sequence(keybindings: &[u32], os: OperatingSystem) -> Option<Keybinding> {
    let chords = keybindings
        .iter()
        .map(|&keybinding| create_simple_keybinding(keybinding, os).map(Chord::KeyCode))
        .collect::<Option<Vec<_>>>()?;
    Keybinding::new(chords)
}

/// Decodes one encoded chord (upstream `createSimpleKeybinding`).
///
/// On macOS the Cmd and Ctrl modifiers are swapped relative to Windows and Linux: the Ctrl-Cmd
/// bit is Cmd there, and the WinCtrl bit is Ctrl. Returns `None` when the key bits are not a
/// [`KeyCode`].
pub fn create_simple_keybinding(keybinding: u32, os: OperatingSystem) -> Option<KeyCodeChord> {
    let ctrl_cmd = (keybinding & KeyMod::CTRL_CMD) != 0;
    let win_ctrl = (keybinding & KeyMod::WIN_CTRL) != 0;
    let is_mac = os == OperatingSystem::Macintosh;

    let ctrl_key = if is_mac { win_ctrl } else { ctrl_cmd };
    let shift_key = (keybinding & KeyMod::SHIFT) != 0;
    let alt_key = (keybinding & KeyMod::ALT) != 0;
    let meta_key = if is_mac { ctrl_cmd } else { win_ctrl };
    let key_code = KeyCode::from_u32(keybinding & KEY_CODE_MASK)?;

    Some(KeyCodeChord::new(
        ctrl_key, shift_key, alt_key, meta_key, key_code,
    ))
}

fn modifier_bit(set: bool) -> char {
    if set { '1' } else { '0' }
}

/// A chord that matches on the key code of the keyboard event (upstream `KeyCodeChord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCodeChord {
    /// Ctrl is held.
    pub ctrl_key: bool,
    /// Shift is held.
    pub shift_key: bool,
    /// Alt is held.
    pub alt_key: bool,
    /// Meta (Win, Cmd, or Super) is held.
    pub meta_key: bool,
    /// The key code.
    pub key_code: KeyCode,
}

impl KeyCodeChord {
    /// Builds a chord from its modifiers and key code.
    pub fn new(
        ctrl_key: bool,
        shift_key: bool,
        alt_key: bool,
        meta_key: bool,
        key_code: KeyCode,
    ) -> Self {
        Self {
            ctrl_key,
            shift_key,
            alt_key,
            meta_key,
            key_code,
        }
    }

    /// A string that identifies the chord, with a `K` prefix so it never equals a scan code chord.
    pub fn hash_code(&self) -> String {
        format!(
            "K{}{}{}{}{}",
            modifier_bit(self.ctrl_key),
            modifier_bit(self.shift_key),
            modifier_bit(self.alt_key),
            modifier_bit(self.meta_key),
            self.key_code as u32
        )
    }

    /// True when the key code is a modifier key, or unknown (upstream `isModifierKey`, the method).
    pub fn is_modifier_key(&self) -> bool {
        matches!(
            self.key_code,
            KeyCode::Unknown | KeyCode::Ctrl | KeyCode::Meta | KeyCode::Alt | KeyCode::Shift
        )
    }

    /// Wraps this chord in a single-chord keybinding (upstream `toKeybinding`).
    pub fn to_keybinding(self) -> Keybinding {
        Keybinding::from_chord(Chord::KeyCode(self))
    }

    /// True when the chord is a modifier key that is also held as its own modifier, such as Ctrl
    /// with the Ctrl key code.
    pub fn is_duplicate_modifier_case(&self) -> bool {
        (self.ctrl_key && self.key_code == KeyCode::Ctrl)
            || (self.shift_key && self.key_code == KeyCode::Shift)
            || (self.alt_key && self.key_code == KeyCode::Alt)
            || (self.meta_key && self.key_code == KeyCode::Meta)
    }
}

/// A chord that matches on the physical key (upstream `ScanCodeChord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCodeChord {
    /// Ctrl is held.
    pub ctrl_key: bool,
    /// Shift is held.
    pub shift_key: bool,
    /// Alt is held.
    pub alt_key: bool,
    /// Meta (Win, Cmd, or Super) is held.
    pub meta_key: bool,
    /// The physical key.
    pub scan_code: ScanCode,
}

impl ScanCodeChord {
    /// Builds a chord from its modifiers and scan code.
    pub fn new(
        ctrl_key: bool,
        shift_key: bool,
        alt_key: bool,
        meta_key: bool,
        scan_code: ScanCode,
    ) -> Self {
        Self {
            ctrl_key,
            shift_key,
            alt_key,
            meta_key,
            scan_code,
        }
    }

    /// A string that identifies the chord, with an `S` prefix so it never equals a key code chord.
    pub fn hash_code(&self) -> String {
        format!(
            "S{}{}{}{}{}",
            modifier_bit(self.ctrl_key),
            modifier_bit(self.shift_key),
            modifier_bit(self.alt_key),
            modifier_bit(self.meta_key),
            self.scan_code as u32
        )
    }

    /// True when the chord holds a modifier key's own physical key together with that modifier.
    pub fn is_duplicate_modifier_case(&self) -> bool {
        (self.ctrl_key
            && (self.scan_code == ScanCode::ControlLeft
                || self.scan_code == ScanCode::ControlRight))
            || (self.shift_key
                && (self.scan_code == ScanCode::ShiftLeft
                    || self.scan_code == ScanCode::ShiftRight))
            || (self.alt_key
                && (self.scan_code == ScanCode::AltLeft || self.scan_code == ScanCode::AltRight))
            || (self.meta_key
                && (self.scan_code == ScanCode::MetaLeft || self.scan_code == ScanCode::MetaRight))
    }
}

/// One chord of a keybinding: either kind (upstream `Chord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    /// Matches the key code.
    KeyCode(KeyCodeChord),
    /// Matches the physical key.
    ScanCode(ScanCodeChord),
}

impl Chord {
    /// The identity string of the chord, as [`KeyCodeChord::hash_code`] or
    /// [`ScanCodeChord::hash_code`].
    pub fn hash_code(&self) -> String {
        match self {
            Chord::KeyCode(chord) => chord.hash_code(),
            Chord::ScanCode(chord) => chord.hash_code(),
        }
    }
}

/// A keybinding: one or more chords (upstream `Keybinding`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybinding {
    chords: Vec<Chord>,
}

impl Keybinding {
    /// Builds a keybinding. Returns `None` for an empty chord list, which upstream rejects with
    /// `illegalArgument`.
    pub fn new(chords: Vec<Chord>) -> Option<Self> {
        if chords.is_empty() {
            None
        } else {
            Some(Self { chords })
        }
    }

    /// Builds a keybinding from a single chord.
    pub fn from_chord(chord: Chord) -> Self {
        Self {
            chords: vec![chord],
        }
    }

    /// The chords, in order.
    pub fn chords(&self) -> &[Chord] {
        &self.chords
    }

    /// The identity string of the keybinding: chord identities joined with `;`.
    pub fn hash_code(&self) -> String {
        self.chords
            .iter()
            .map(Chord::hash_code)
            .collect::<Vec<_>>()
            .join(";")
    }
}

/// A chord as it is shown and dispatched: modifiers, plus labels for the key (upstream
/// `ResolvedChord`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedChord {
    /// Ctrl is held.
    pub ctrl_key: bool,
    /// Shift is held.
    pub shift_key: bool,
    /// Alt is held.
    pub alt_key: bool,
    /// Meta is held.
    pub meta_key: bool,
    /// The key as shown in the UI, or `None` for a modifier-only chord.
    pub key_label: Option<String>,
    /// The key as read by assistive technology, or `None`.
    pub key_aria_label: Option<String>,
}

/// A modifier-only chord, used for dispatch (upstream `SingleModifierChord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleModifierChord {
    /// Ctrl alone.
    Ctrl,
    /// Shift alone.
    Shift,
    /// Alt alone.
    Alt,
    /// Meta alone.
    Meta,
}

impl SingleModifierChord {
    /// The dispatch string for this modifier: `ctrl`, `shift`, `alt`, or `meta`.
    pub fn as_str(self) -> &'static str {
        match self {
            SingleModifierChord::Ctrl => "ctrl",
            SingleModifierChord::Shift => "shift",
            SingleModifierChord::Alt => "alt",
            SingleModifierChord::Meta => "meta",
        }
    }
}

/// A keybinding resolved for a keyboard layout and platform, ready to label and dispatch
/// (upstream `ResolvedKeybinding`).
///
/// Implementations are provided by a layout-aware resolver; this trait is the contract the rest of
/// the workbench depends on.
pub trait ResolvedKeybinding {
    /// The label shown in the UI, or `None` when the binding cannot be shown.
    fn label(&self) -> Option<String>;
    /// The label for assistive technology.
    fn aria_label(&self) -> Option<String>;
    /// The accelerator string for Electron menus.
    fn electron_accelerator(&self) -> Option<String>;
    /// The string written to user settings for this binding.
    fn user_settings_label(&self) -> Option<String>;
    /// True when the user-settings label matches the UI label.
    fn is_wysiwyg(&self) -> bool;
    /// True when the binding has more than one chord.
    fn has_multiple_chords(&self) -> bool;
    /// The resolved chords, in order.
    fn chords(&self) -> &[ResolvedChord];
    /// One dispatch string per chord (`shift+D` style), `None` for modifier-only chords.
    fn dispatch_chords(&self) -> Vec<Option<String>>;
    /// One modifier-only dispatch value per chord, `None` for chords with more than one modifier or
    /// a regular key.
    fn single_modifier_dispatch_chords(&self) -> Vec<Option<SingleModifierChord>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_codes::key_chord;

    fn kc(ctrl: bool, shift: bool, alt: bool, meta: bool, key: KeyCode) -> KeyCodeChord {
        KeyCodeChord::new(ctrl, shift, alt, meta, key)
    }

    fn one(chord: KeyCodeChord) -> Option<Keybinding> {
        Some(chord.to_keybinding())
    }

    fn two(first: KeyCodeChord, second: KeyCodeChord) -> Option<Keybinding> {
        Keybinding::new(vec![Chord::KeyCode(first), Chord::KeyCode(second)])
    }

    /// Upstream test `MAC binary encoding` in `src/vs/base/test/common/keyCodes.test.ts`.
    #[test]
    fn mac_binary_encoding() {
        let enter = KeyCode::Enter as u32;
        let test = |expected: Option<Keybinding>, k: u32| {
            assert_eq!(
                decode_keybinding(k, OperatingSystem::Macintosh),
                expected,
                "encoding {k:#x}"
            );
        };

        test(None, 0);
        test(one(kc(false, false, false, false, KeyCode::Enter)), enter);
        test(
            one(kc(true, false, false, false, KeyCode::Enter)),
            KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, false, true, false, KeyCode::Enter)),
            KeyMod::ALT | enter,
        );
        test(
            one(kc(true, false, true, false, KeyCode::Enter)),
            KeyMod::ALT | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, true, false, false, KeyCode::Enter)),
            KeyMod::SHIFT | enter,
        );
        test(
            one(kc(true, true, false, false, KeyCode::Enter)),
            KeyMod::SHIFT | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, true, true, false, KeyCode::Enter)),
            KeyMod::SHIFT | KeyMod::ALT | enter,
        );
        test(
            one(kc(true, true, true, false, KeyCode::Enter)),
            KeyMod::SHIFT | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, false, false, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | enter,
        );
        test(
            one(kc(true, false, false, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, false, true, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::ALT | enter,
        );
        test(
            one(kc(true, false, true, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, true, false, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::SHIFT | enter,
        );
        test(
            one(kc(true, true, false, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::WIN_CTRL | enter,
        );
        test(
            one(kc(false, true, true, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::ALT | enter,
        );
        test(
            one(kc(true, true, true, true, KeyCode::Enter)),
            KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
        );

        test(
            two(
                kc(false, false, false, false, KeyCode::Enter),
                kc(false, false, false, false, KeyCode::Tab),
            ),
            key_chord(enter, KeyCode::Tab as u32),
        );
        test(
            two(
                kc(false, false, false, true, KeyCode::KeyY),
                kc(false, false, false, false, KeyCode::KeyZ),
            ),
            key_chord(
                KeyMod::CTRL_CMD | KeyCode::KeyY as u32,
                KeyCode::KeyZ as u32,
            ),
        );
    }

    /// Upstream test `WINDOWS & LINUX binary encoding` in `src/vs/base/test/common/keyCodes.test.ts`.
    #[test]
    fn windows_and_linux_binary_encoding() {
        let enter = KeyCode::Enter as u32;
        for os in [OperatingSystem::Linux, OperatingSystem::Windows] {
            let test = |expected: Option<Keybinding>, k: u32| {
                assert_eq!(decode_keybinding(k, os), expected, "{os:?} encoding {k:#x}");
            };

            test(None, 0);
            test(one(kc(false, false, false, false, KeyCode::Enter)), enter);
            test(
                one(kc(false, false, false, true, KeyCode::Enter)),
                KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(false, false, true, false, KeyCode::Enter)),
                KeyMod::ALT | enter,
            );
            test(
                one(kc(false, false, true, true, KeyCode::Enter)),
                KeyMod::ALT | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(false, true, false, false, KeyCode::Enter)),
                KeyMod::SHIFT | enter,
            );
            test(
                one(kc(false, true, false, true, KeyCode::Enter)),
                KeyMod::SHIFT | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(false, true, true, false, KeyCode::Enter)),
                KeyMod::SHIFT | KeyMod::ALT | enter,
            );
            test(
                one(kc(false, true, true, true, KeyCode::Enter)),
                KeyMod::SHIFT | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(true, false, false, false, KeyCode::Enter)),
                KeyMod::CTRL_CMD | enter,
            );
            test(
                one(kc(true, false, false, true, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(true, false, true, false, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::ALT | enter,
            );
            test(
                one(kc(true, false, true, true, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(true, true, false, false, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::SHIFT | enter,
            );
            test(
                one(kc(true, true, false, true, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::WIN_CTRL | enter,
            );
            test(
                one(kc(true, true, true, false, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::ALT | enter,
            );
            test(
                one(kc(true, true, true, true, KeyCode::Enter)),
                KeyMod::CTRL_CMD | KeyMod::SHIFT | KeyMod::ALT | KeyMod::WIN_CTRL | enter,
            );

            test(
                two(
                    kc(false, false, false, false, KeyCode::Enter),
                    kc(false, false, false, false, KeyCode::Tab),
                ),
                key_chord(enter, KeyCode::Tab as u32),
            );
            test(
                two(
                    kc(true, false, false, false, KeyCode::KeyY),
                    kc(false, false, false, false, KeyCode::KeyZ),
                ),
                key_chord(
                    KeyMod::CTRL_CMD | KeyCode::KeyY as u32,
                    KeyCode::KeyZ as u32,
                ),
            );
        }
    }

    /// Upstream test `issue #173325: wrong interpretations of special keys (e.g. [Equal] is
    /// mistaken for V)` in `src/vs/base/test/common/keybindings.test.ts`.
    #[test]
    fn issue_173325_key_code_and_scan_code_chords_differ() {
        let a = KeyCodeChord::new(true, false, false, false, KeyCode::KeyV);
        let b = ScanCodeChord::new(true, false, false, false, ScanCode::Equal);
        assert_ne!(a.hash_code(), b.hash_code());
    }

    #[test]
    fn decode_rejects_unknown_key_codes_and_empty_sequences() {
        assert_eq!(
            decode_keybinding_sequence(&[], OperatingSystem::Linux),
            None
        );
        // 0xFF is not a KeyCode.
        assert_eq!(decode_keybinding(0xFF, OperatingSystem::Linux), None);
        assert_eq!(Keybinding::new(Vec::new()), None);
    }

    #[test]
    fn decode_sequence_matches_two_number_encoding() {
        let encoded = key_chord(KeyCode::Enter as u32, KeyCode::Tab as u32);
        assert_eq!(
            decode_keybinding(encoded, OperatingSystem::Windows),
            decode_keybinding_sequence(
                &[KeyCode::Enter as u32, KeyCode::Tab as u32],
                OperatingSystem::Windows
            )
        );
    }

    #[test]
    fn hash_codes_identify_chords() {
        let k = Keybinding::new(vec![
            Chord::KeyCode(kc(true, false, false, false, KeyCode::KeyK)),
            Chord::ScanCode(ScanCodeChord::new(
                false,
                true,
                false,
                false,
                ScanCode::KeyS,
            )),
        ])
        .expect("two chords");
        let expected = format!(
            "K1000{};S0100{}",
            KeyCode::KeyK as u32,
            ScanCode::KeyS as u32
        );
        assert_eq!(k.hash_code(), expected);
    }

    #[test]
    fn duplicate_modifier_cases() {
        assert!(kc(true, false, false, false, KeyCode::Ctrl).is_duplicate_modifier_case());
        assert!(!kc(true, false, false, false, KeyCode::KeyA).is_duplicate_modifier_case());
        assert!(
            ScanCodeChord::new(false, false, false, true, ScanCode::MetaRight)
                .is_duplicate_modifier_case()
        );
        assert!(kc(false, false, false, false, KeyCode::Unknown).is_modifier_key());
    }
}
