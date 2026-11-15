//! Key codes, scan codes, the modifier bit layout, and the string tables that map them to UI
//! labels, user-settings names and native key names.
//!
//! Ported from `src/vs/base/common/keyCodes.ts`. The lookup tables are built once from the
//! upstream mapping table, in upstream order, so first-seen and last-write behavior matches.
//! Upstream's `DependsOnKbLayout` sentinel (-1) is represented as `None`.

mod codes;
mod mappings;

use std::collections::HashMap;
use std::sync::OnceLock;

pub use codes::{KeyCode, ScanCode};

/// Modifier bits used by keybinding encodings. Upstream: `KeyMod` (`keyCodes.ts`).
pub struct KeyMod;

impl KeyMod {
    /// Ctrl on Windows and Linux, Cmd on macOS.
    pub const CTRL_CMD: u32 = 1 << 11;
    /// Shift.
    pub const SHIFT: u32 = 1 << 10;
    /// Alt (Option on macOS).
    pub const ALT: u32 = 1 << 9;
    /// Win on Windows, Ctrl on macOS and Linux.
    pub const WIN_CTRL: u32 = 1 << 8;
}

/// Joins two encodings into a two-chord encoding: `first` in the low 16 bits, the key of
/// `second` in the high 16 bits. Upstream: `KeyChord` (`keyCodes.ts`).
pub fn key_chord(first_part: u32, second_part: u32) -> u32 {
    let chord_part = (second_part & 0x0000_FFFF) << 16;
    first_part | chord_part
}

/// Upstream: `isModifierKey` (`keyCodes.ts`).
pub fn is_modifier_key(key_code: KeyCode) -> bool {
    matches!(
        key_code,
        KeyCode::Ctrl | KeyCode::Shift | KeyCode::Alt | KeyCode::Meta
    )
}

/// Number of DOM `keyCode` slots (upstream `new Array(230)`).
const EVENT_KEY_CODE_SLOTS: usize = 230;

/// Upstream `KeyCodeStrMap`: a key code table with lookups in both directions.
struct KeyCodeLabels {
    by_code: Vec<Option<&'static str>>,
    by_label: HashMap<String, KeyCode>,
}

impl KeyCodeLabels {
    fn new() -> Self {
        Self {
            by_code: vec![None; KeyCode::ALL.len()],
            by_label: HashMap::new(),
        }
    }

    fn define(&mut self, key_code: KeyCode, label: &'static str) {
        self.by_code[key_code as usize] = Some(label);
        self.by_label.insert(label.to_lowercase(), key_code);
    }

    fn label(&self, key_code: KeyCode) -> Option<&'static str> {
        self.by_code[key_code as usize]
    }

    fn key_code(&self, label: &str) -> Option<KeyCode> {
        self.by_label.get(&label.to_lowercase()).copied()
    }
}

/// Every lookup table upstream builds at module load.
struct Tables {
    ui_labels: KeyCodeLabels,
    us_user_settings_labels: KeyCodeLabels,
    general_user_settings_labels: KeyCodeLabels,
    /// Upstream `EVENT_KEY_CODE_MAP`, indexed by DOM `keyCode`.
    event_key_codes: Vec<Option<KeyCode>>,
    /// Upstream `NATIVE_WINDOWS_KEY_CODE_TO_KEY_CODE`.
    native_windows_key_codes: HashMap<&'static str, KeyCode>,
    /// Upstream `SCAN_CODE_STR_TO_EVENT_KEY_CODE`.
    scan_code_to_event_key_code: HashMap<&'static str, u32>,
    /// Upstream `scanCodeIntToStr`, indexed by scan code.
    scan_code_labels: Vec<Option<&'static str>>,
    /// Upstream `scanCodeStrToInt`.
    scan_codes_by_label: HashMap<&'static str, ScanCode>,
    /// Upstream `scanCodeLowerCaseStrToInt`, keyed by lower-cased label.
    scan_codes_by_lower_case_label: HashMap<String, ScanCode>,
    /// Upstream `IMMUTABLE_CODE_TO_KEY_CODE`; `None` where the mapping depends on the layout.
    immutable_key_codes: Vec<Option<KeyCode>>,
    /// Upstream `IMMUTABLE_KEY_CODE_TO_CODE`; `None` where the mapping depends on the layout.
    immutable_scan_codes: Vec<Option<ScanCode>>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(build_tables)
}

/// Upstream: the IIFE at the end of `keyCodes.ts`, which walks the mapping rows in order.
fn build_tables() -> Tables {
    let mut t = Tables {
        ui_labels: KeyCodeLabels::new(),
        us_user_settings_labels: KeyCodeLabels::new(),
        general_user_settings_labels: KeyCodeLabels::new(),
        event_key_codes: vec![None; EVENT_KEY_CODE_SLOTS],
        native_windows_key_codes: HashMap::new(),
        scan_code_to_event_key_code: HashMap::new(),
        scan_code_labels: vec![None; ScanCode::ALL.len()],
        scan_codes_by_label: HashMap::new(),
        scan_codes_by_lower_case_label: HashMap::new(),
        immutable_key_codes: vec![None; ScanCode::ALL.len()],
        immutable_scan_codes: vec![None; KeyCode::ALL.len()],
    };
    let mut scan_code_seen = vec![false; ScanCode::ALL.len()];
    let mut key_code_seen = vec![false; KeyCode::ALL.len()];

    for &(
        immutable,
        scan_code,
        scan_code_str,
        key_code,
        key_code_str,
        event_key_code,
        vkey,
        us_label,
        general_label,
    ) in mappings::ROWS
    {
        if !scan_code_seen[scan_code as usize] {
            scan_code_seen[scan_code as usize] = true;
            t.scan_code_labels[scan_code as usize] = Some(scan_code_str);
            t.scan_codes_by_label.insert(scan_code_str, scan_code);
            t.scan_codes_by_lower_case_label
                .insert(scan_code_str.to_lowercase(), scan_code);
            if immutable {
                t.immutable_key_codes[scan_code as usize] = Some(key_code);
                if key_code != KeyCode::Unknown
                    && key_code != KeyCode::Enter
                    && !is_modifier_key(key_code)
                {
                    t.immutable_scan_codes[key_code as usize] = Some(scan_code);
                }
            }
        }
        if !key_code_seen[key_code as usize] {
            key_code_seen[key_code as usize] = true;
            assert!(
                !key_code_str.is_empty(),
                "no label for key code {key_code:?} (scan code {scan_code_str})"
            );
            t.ui_labels.define(key_code, key_code_str);
            let us = or_empty(us_label, key_code_str);
            t.us_user_settings_labels.define(key_code, us);
            let general = or_empty(or_empty(general_label, us_label), key_code_str);
            t.general_user_settings_labels.define(key_code, general);
        }
        if event_key_code != 0 {
            t.event_key_codes[event_key_code as usize] = Some(key_code);
        }
        if !scan_code_str.is_empty() {
            t.scan_code_to_event_key_code
                .insert(scan_code_str, event_key_code);
        }
        if !vkey.is_empty() {
            t.native_windows_key_codes.insert(vkey, key_code);
        }
    }
    // Upstream adds this after the loop: `Enter` is excluded from the table above because the
    // numpad Enter row shares its key code.
    t.immutable_scan_codes[KeyCode::Enter as usize] = Some(ScanCode::Enter);
    t
}

/// Returns `label`, or `fallback` when `label` is empty (upstream's `a || b`).
fn or_empty(label: &'static str, fallback: &'static str) -> &'static str {
    if label.is_empty() { fallback } else { label }
}

/// Upstream `EVENT_KEY_CODE_MAP`: the key code for a DOM `keyCode`, if the table has one.
pub fn key_code_for_event_key_code(event_key_code: u32) -> Option<KeyCode> {
    tables()
        .event_key_codes
        .get(event_key_code as usize)
        .copied()
        .flatten()
}

/// Upstream `NATIVE_WINDOWS_KEY_CODE_TO_KEY_CODE`: the key code for a Windows virtual-key name.
pub fn key_code_for_native_windows_key(vkey: &str) -> Option<KeyCode> {
    tables().native_windows_key_codes.get(vkey).copied()
}

/// Upstream `SCAN_CODE_STR_TO_EVENT_KEY_CODE`: the DOM `keyCode` for a scan code label.
pub fn event_key_code_for_scan_code_label(label: &str) -> Option<u32> {
    tables().scan_code_to_event_key_code.get(label).copied()
}

/// Upstream `IMMUTABLE_CODE_TO_KEY_CODE`: the key code a scan code always produces, or `None`
/// when it depends on the keyboard layout.
pub fn immutable_key_code_for_scan_code(scan_code: ScanCode) -> Option<KeyCode> {
    tables()
        .immutable_key_codes
        .get(scan_code as usize)
        .copied()
        .flatten()
}

/// Upstream `IMMUTABLE_KEY_CODE_TO_CODE`: the scan code a key code always comes from, or `None`
/// when it depends on the keyboard layout.
pub fn immutable_scan_code_for_key_code(key_code: KeyCode) -> Option<ScanCode> {
    tables()
        .immutable_scan_codes
        .get(key_code as usize)
        .copied()
        .flatten()
}

/// Upstream `ScanCodeUtils`.
pub struct ScanCodeUtils;

impl ScanCodeUtils {
    /// Looks up a scan code by a lower-cased label. Like upstream `lowerCaseToEnum`, the argument
    /// is used as given and is not lower-cased first.
    pub fn from_lower_case_label(label: &str) -> Option<ScanCode> {
        tables().scan_codes_by_lower_case_label.get(label).copied()
    }

    /// Looks up a scan code by its label (upstream `toEnum`).
    pub fn from_label(label: &str) -> Option<ScanCode> {
        tables().scan_codes_by_label.get(label).copied()
    }

    /// The label of a scan code (upstream `toString`).
    pub fn to_label(scan_code: ScanCode) -> Option<&'static str> {
        tables().scan_code_labels[scan_code as usize]
    }
}

/// Upstream `KeyCodeUtils`.
pub struct KeyCodeUtils;

impl KeyCodeUtils {
    /// The UI label of a key code (upstream `toString`).
    pub fn to_label(key_code: KeyCode) -> Option<&'static str> {
        tables().ui_labels.label(key_code)
    }

    /// The key code for a UI label (upstream `fromString`). Lookup ignores case.
    pub fn from_label(label: &str) -> Option<KeyCode> {
        tables().ui_labels.key_code(label)
    }

    /// The US user-settings label of a key code (upstream `toUserSettingsUS`).
    pub fn to_us_user_settings_label(key_code: KeyCode) -> Option<&'static str> {
        tables().us_user_settings_labels.label(key_code)
    }

    /// The general user-settings label of a key code (upstream `toUserSettingsGeneral`).
    pub fn to_general_user_settings_label(key_code: KeyCode) -> Option<&'static str> {
        tables().general_user_settings_labels.label(key_code)
    }

    /// The key code for a user-settings label (upstream `fromUserSettings`). The US labels are
    /// tried first, then the general labels. Lookup ignores case.
    pub fn from_user_settings_label(label: &str) -> Option<KeyCode> {
        let t = tables();
        t.us_user_settings_labels
            .key_code(label)
            .or_else(|| t.general_user_settings_labels.key_code(label))
    }

    /// The Electron accelerator name of a key code (upstream `toElectronAccelerator`). Numpad
    /// keys have none: Electron renders them like the regular keys, so they need custom labels.
    pub fn to_electron_accelerator(key_code: KeyCode) -> Option<&'static str> {
        let value = key_code as u32;
        if (KeyCode::Numpad0 as u32..=KeyCode::NumpadDivide as u32).contains(&value) {
            return None;
        }
        match key_code {
            KeyCode::UpArrow => Some("Up"),
            KeyCode::DownArrow => Some("Down"),
            KeyCode::LeftArrow => Some("Left"),
            KeyCode::RightArrow => Some("Right"),
            KeyCode::Delete => Some("Delete"),
            _ => Self::to_label(key_code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream test `mapping for Minus` in `src/vs/base/test/common/keyCodes.test.ts`.
    #[test]
    fn mapping_for_minus() {
        assert_eq!(key_code_for_event_key_code(189), Some(KeyCode::Minus));
        assert_eq!(
            key_code_for_native_windows_key("VK_OEM_MINUS"),
            Some(KeyCode::Minus)
        );
        assert_eq!(
            ScanCodeUtils::from_lower_case_label("minus"),
            Some(ScanCode::Minus)
        );
        assert_eq!(ScanCodeUtils::from_label("Minus"), Some(ScanCode::Minus));
        assert_eq!(ScanCodeUtils::to_label(ScanCode::Minus), Some("Minus"));
        assert_eq!(immutable_key_code_for_scan_code(ScanCode::Minus), None);
        assert_eq!(immutable_scan_code_for_key_code(KeyCode::Minus), None);
        assert_eq!(KeyCodeUtils::to_label(KeyCode::Minus), Some("-"));
        assert_eq!(KeyCodeUtils::from_label("-"), Some(KeyCode::Minus));
        assert_eq!(
            KeyCodeUtils::to_us_user_settings_label(KeyCode::Minus),
            Some("-")
        );
        assert_eq!(
            KeyCodeUtils::to_general_user_settings_label(KeyCode::Minus),
            Some("OEM_MINUS")
        );
        assert_eq!(
            KeyCodeUtils::from_user_settings_label("-"),
            Some(KeyCode::Minus)
        );
        assert_eq!(
            KeyCodeUtils::from_user_settings_label("OEM_MINUS"),
            Some(KeyCode::Minus)
        );
        assert_eq!(
            KeyCodeUtils::from_user_settings_label("oem_minus"),
            Some(KeyCode::Minus)
        );
    }

    /// Upstream test `mapping for Space` in `src/vs/base/test/common/keyCodes.test.ts`.
    #[test]
    fn mapping_for_space() {
        assert_eq!(key_code_for_event_key_code(32), Some(KeyCode::Space));
        assert_eq!(
            key_code_for_native_windows_key("VK_SPACE"),
            Some(KeyCode::Space)
        );
        assert_eq!(
            ScanCodeUtils::from_lower_case_label("space"),
            Some(ScanCode::Space)
        );
        assert_eq!(ScanCodeUtils::from_label("Space"), Some(ScanCode::Space));
        assert_eq!(ScanCodeUtils::to_label(ScanCode::Space), Some("Space"));
        assert_eq!(
            immutable_key_code_for_scan_code(ScanCode::Space),
            Some(KeyCode::Space)
        );
        assert_eq!(
            immutable_scan_code_for_key_code(KeyCode::Space),
            Some(ScanCode::Space)
        );
        assert_eq!(KeyCodeUtils::to_label(KeyCode::Space), Some("Space"));
        assert_eq!(KeyCodeUtils::from_label("Space"), Some(KeyCode::Space));
        assert_eq!(
            KeyCodeUtils::to_us_user_settings_label(KeyCode::Space),
            Some("Space")
        );
        assert_eq!(
            KeyCodeUtils::to_general_user_settings_label(KeyCode::Space),
            Some("Space")
        );
        assert_eq!(
            KeyCodeUtils::from_user_settings_label("Space"),
            Some(KeyCode::Space)
        );
        assert_eq!(
            KeyCodeUtils::from_user_settings_label("space"),
            Some(KeyCode::Space)
        );
    }

    /// Every enum member has a UI label and a scan code label. The table build panics on a row
    /// without a label, so this test is the guard that catches a bad regeneration.
    #[test]
    fn every_member_has_a_label() {
        for key_code in KeyCode::ALL {
            assert!(
                KeyCodeUtils::to_label(*key_code).is_some(),
                "{key_code:?} has no UI label"
            );
        }
        for scan_code in ScanCode::ALL {
            assert!(
                ScanCodeUtils::to_label(*scan_code).is_some(),
                "{scan_code:?} has no label"
            );
        }
    }

    #[test]
    fn enum_values_follow_declaration_order() {
        for (index, key_code) in KeyCode::ALL.iter().enumerate() {
            assert_eq!(*key_code as usize, index);
            assert_eq!(KeyCode::from_u32(index as u32), Some(*key_code));
        }
        for (index, scan_code) in ScanCode::ALL.iter().enumerate() {
            assert_eq!(*scan_code as usize, index);
        }
        assert_eq!(KeyCode::from_u32(KeyCode::ALL.len() as u32), None);
    }

    #[test]
    fn key_chord_places_second_key_in_high_bits() {
        assert_eq!(
            key_chord(
                KeyMod::CTRL_CMD | KeyCode::KeyY as u32,
                KeyCode::KeyZ as u32
            ),
            (KeyMod::CTRL_CMD | KeyCode::KeyY as u32) | ((KeyCode::KeyZ as u32) << 16)
        );
    }

    #[test]
    fn electron_accelerator_skips_numpad_and_maps_arrows() {
        assert_eq!(
            KeyCodeUtils::to_electron_accelerator(KeyCode::Numpad0),
            None
        );
        assert_eq!(
            KeyCodeUtils::to_electron_accelerator(KeyCode::UpArrow),
            Some("Up")
        );
        assert_eq!(
            KeyCodeUtils::to_electron_accelerator(KeyCode::Delete),
            Some("Delete")
        );
        assert_eq!(
            KeyCodeUtils::to_electron_accelerator(KeyCode::Enter),
            Some("Enter")
        );
    }
}
