//! Generated from `src/vs/base/common/keyCodes.ts`: the `KeyCode` and `ScanCode` enums.
//!
//! Member names follow upstream, with screaming-case names turned into CamelCase. Discriminants
//! follow upstream order. The upstream `DependsOnKbLayout` sentinel (-1) and `MAX_VALUE` are not
//! variants; layout-dependent lookups return `None` instead.

/// Declares a fieldless enum whose discriminants follow the listed order, plus `ALL` and
/// `from_u32`.
macro_rules! code_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident,)* }) => {
        #[allow(missing_docs)] // Variants mirror upstream members one-to-one; see the enum doc.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u16)]
        $(#[$meta])*
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            /// Every value, indexed by its discriminant.
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// Returns the value with this discriminant, if there is one.
            pub fn from_u32(value: u32) -> Option<$name> {
                Self::ALL.get(value as usize).copied()
            }
        }
    };
}

code_enum! {
    /// Virtual key codes. Upstream: `KeyCode` in `keyCodes.ts`.
    KeyCode {
        Unknown, Backspace, Tab, Enter, Shift, Ctrl, Alt, PauseBreak, CapsLock, Escape, Space,
        PageUp, PageDown, End, Home, LeftArrow, UpArrow, RightArrow, DownArrow, Insert, Delete,
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, KeyA, KeyB,
        KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ,
        KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Meta, ContextMenu, F1, F2, F3, F4,
        F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23,
        F24, NumLock, ScrollLock, Semicolon, Equal, Comma, Minus, Period, Slash, Backquote,
        BracketLeft, Backslash, BracketRight, Quote, Oem8, IntlBackslash, Numpad0, Numpad1,
        Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadMultiply,
        NumpadAdd, NumpadSeparator, NumpadSubtract, NumpadDecimal, NumpadDivide, KeyInComposition,
        AbntC1, AbntC2, AudioVolumeMute, AudioVolumeUp, AudioVolumeDown, BrowserSearch,
        BrowserHome, BrowserBack, BrowserForward, MediaTrackNext, MediaTrackPrevious, MediaStop,
        MediaPlayPause, LaunchMediaPlayer, LaunchMail, LaunchApp2, Clear,
    }
}

code_enum! {
    /// Physical key codes (`keyboardEvent.code`). Upstream: `ScanCode` in `keyCodes.ts`.
    ScanCode {
        None, Hyper, Super, Fn, FnLock, Suspend, Resume, Turbo, Sleep, WakeUp, KeyA, KeyB, KeyC,
        KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
        KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Digit1, Digit2, Digit3, Digit4, Digit5,
        Digit6, Digit7, Digit8, Digit9, Digit0, Enter, Escape, Backspace, Tab, Space, Minus, Equal,
        BracketLeft, BracketRight, Backslash, IntlHash, Semicolon, Quote, Backquote, Comma, Period,
        Slash, CapsLock, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, PrintScreen,
        ScrollLock, Pause, Insert, Home, PageUp, Delete, End, PageDown, ArrowRight, ArrowLeft,
        ArrowDown, ArrowUp, NumLock, NumpadDivide, NumpadMultiply, NumpadSubtract, NumpadAdd,
        NumpadEnter, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8,
        Numpad9, Numpad0, NumpadDecimal, IntlBackslash, ContextMenu, Power, NumpadEqual, F13, F14,
        F15, F16, F17, F18, F19, F20, F21, F22, F23, F24, Open, Help, Select, Again, Undo, Cut,
        Copy, Paste, Find, AudioVolumeMute, AudioVolumeUp, AudioVolumeDown, NumpadComma, IntlRo,
        KanaMode, IntlYen, Convert, NonConvert, Lang1, Lang2, Lang3, Lang4, Lang5, Abort, Props,
        NumpadParenLeft, NumpadParenRight, NumpadBackspace, NumpadMemoryStore, NumpadMemoryRecall,
        NumpadMemoryClear, NumpadMemoryAdd, NumpadMemorySubtract, NumpadClear, NumpadClearEntry,
        ControlLeft, ShiftLeft, AltLeft, MetaLeft, ControlRight, ShiftRight, AltRight, MetaRight,
        BrightnessUp, BrightnessDown, MediaPlay, MediaRecord, MediaFastForward, MediaRewind,
        MediaTrackNext, MediaTrackPrevious, MediaStop, Eject, MediaPlayPause, MediaSelect,
        LaunchMail, LaunchApp2, LaunchApp1, SelectTask, LaunchScreenSaver, BrowserSearch,
        BrowserHome, BrowserBack, BrowserForward, BrowserStop, BrowserRefresh, BrowserFavorites,
        ZoomToggle, MailReply, MailForward, MailSend,
    }
}
