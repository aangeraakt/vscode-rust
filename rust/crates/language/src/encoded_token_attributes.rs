//! Packed token metadata: a 32-bit word per token holding language, token type, bracket balance,
//! font style, foreground and background colors.
//!
//! Ported from `src/vs/editor/common/encodedTokenAttributes.ts` (`MetadataConsts`,
//! `TokenMetadata`, `FontStyle`, `ColorId`, `StandardTokenType`). The bit layout is the one
//! documented in that file:
//!
//! ```text
//!     3322 2222 2222 1111 1111 1100 0000 0000
//!     1098 7654 3210 9876 5432 1098 7654 3210
//!     xxxx xxxx xxxx xxxx xxxx xxxx xxxx xxxx
//!     bbbb bbbb ffff ffff fFFF FBTT LLLL LLLL
//! ```
//!
//! `L` language id (8 bits), `T` standard token type (2), `B` balanced brackets (1), `F` font
//! style (4), `f` foreground (9), `b` background (8).
//!
//! Divergences from the TypeScript:
//! - Only the reading side is ported (`TokenMetadata` getters). The CSS class name and inline
//!   style helpers belong to the rendering layer and are not here.
//! - `FontStyle::NotSet` (semantic-token control bits) is not ported; it is only used by the
//!   semantic token merge, which is not part of this crate yet.

/// Language id as stored in metadata: 8 bits. Map to and from string ids with
/// [`crate::LanguageIdCodec`].
pub type EncodedLanguageId = u32;

/// Bit masks and offsets of the metadata word (`MetadataConsts`).
pub mod metadata {
    /// Mask of the language id field.
    pub const LANGUAGEID_MASK: u32 = 0b0000_0000_0000_0000_0000_0000_1111_1111;
    /// Mask of the standard token type field.
    pub const TOKEN_TYPE_MASK: u32 = 0b0000_0000_0000_0000_0000_0011_0000_0000;
    /// Mask of the balanced-brackets flag.
    pub const BALANCED_BRACKETS_MASK: u32 = 0b0000_0000_0000_0000_0000_0100_0000_0000;
    /// Mask of the font style field.
    pub const FONT_STYLE_MASK: u32 = 0b0000_0000_0000_0000_0111_1000_0000_0000;
    /// Mask of the foreground color field.
    pub const FOREGROUND_MASK: u32 = 0b0000_0000_1111_1111_1000_0000_0000_0000;
    /// Mask of the background color field.
    pub const BACKGROUND_MASK: u32 = 0b1111_1111_0000_0000_0000_0000_0000_0000;

    /// Shift of the language id field.
    pub const LANGUAGEID_OFFSET: u32 = 0;
    /// Shift of the standard token type field.
    pub const TOKEN_TYPE_OFFSET: u32 = 8;
    /// Shift of the balanced-brackets flag.
    pub const BALANCED_BRACKETS_OFFSET: u32 = 10;
    /// Shift of the font style field.
    pub const FONT_STYLE_OFFSET: u32 = 11;
    /// Shift of the foreground color field.
    pub const FOREGROUND_OFFSET: u32 = 15;
    /// Shift of the background color field.
    pub const BACKGROUND_OFFSET: u32 = 24;
}

/// A color slot index into the theme's color map (`ColorId`). Open-ended: values above the
/// reserved ones index the map directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColorId(pub u32);

impl ColorId {
    /// No color.
    pub const NONE: ColorId = ColorId(0);
    /// The default foreground.
    pub const DEFAULT_FOREGROUND: ColorId = ColorId(1);
    /// The default background.
    pub const DEFAULT_BACKGROUND: ColorId = ColorId(2);
}

/// Font style flags (`FontStyle`). Combine with `|`, test with [`FontStyle::contains`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FontStyle(pub u32);

impl FontStyle {
    /// No style.
    pub const NONE: FontStyle = FontStyle(0);
    /// Italic.
    pub const ITALIC: FontStyle = FontStyle(1);
    /// Bold.
    pub const BOLD: FontStyle = FontStyle(2);
    /// Underline.
    pub const UNDERLINE: FontStyle = FontStyle(4);
    /// Strikethrough.
    pub const STRIKETHROUGH: FontStyle = FontStyle(8);

    /// Whether every flag in `other` is set.
    pub fn contains(self, other: FontStyle) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for FontStyle {
    type Output = FontStyle;
    fn bitor(self, rhs: FontStyle) -> FontStyle {
        FontStyle(self.0 | rhs.0)
    }
}

/// Standard token types (`StandardTokenType`). Themes and bracket matching use these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StandardTokenType {
    /// Anything else.
    Other,
    /// Comment.
    Comment,
    /// String literal.
    String,
    /// Regular expression literal.
    RegEx,
}

impl StandardTokenType {
    /// Decodes the 2-bit field. Only the low two bits are read, so every value maps to a variant.
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0b11 {
            1 => Self::Comment,
            2 => Self::String,
            3 => Self::RegEx,
            _ => Self::Other,
        }
    }
}

/// The presentation fields of a token, as the renderer reads them (`ITokenPresentation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenPresentation {
    /// Foreground color slot.
    pub foreground: ColorId,
    /// Italic flag.
    pub italic: bool,
    /// Bold flag.
    pub bold: bool,
    /// Underline flag.
    pub underline: bool,
    /// Strikethrough flag.
    pub strikethrough: bool,
}

/// Field readers for a metadata word (`TokenMetadata`).
pub struct TokenMetadata;

impl TokenMetadata {
    /// The encoded language id of the token.
    pub fn language_id(metadata: u32) -> EncodedLanguageId {
        (metadata & metadata::LANGUAGEID_MASK) >> metadata::LANGUAGEID_OFFSET
    }

    /// The standard token type of the token.
    pub fn token_type(metadata: u32) -> StandardTokenType {
        StandardTokenType::from_bits(
            (metadata & metadata::TOKEN_TYPE_MASK) >> metadata::TOKEN_TYPE_OFFSET,
        )
    }

    /// Whether the token is a balanced bracket.
    pub fn contains_balanced_brackets(metadata: u32) -> bool {
        metadata & metadata::BALANCED_BRACKETS_MASK != 0
    }

    /// The font style flags of the token.
    pub fn font_style(metadata: u32) -> FontStyle {
        FontStyle((metadata & metadata::FONT_STYLE_MASK) >> metadata::FONT_STYLE_OFFSET)
    }

    /// The foreground color slot of the token.
    pub fn foreground(metadata: u32) -> ColorId {
        ColorId((metadata & metadata::FOREGROUND_MASK) >> metadata::FOREGROUND_OFFSET)
    }

    /// The background color slot of the token.
    pub fn background(metadata: u32) -> ColorId {
        ColorId((metadata & metadata::BACKGROUND_MASK) >> metadata::BACKGROUND_OFFSET)
    }

    /// The presentation fields of the token (`getPresentationFromMetadata`).
    pub fn presentation(metadata: u32) -> TokenPresentation {
        let style = Self::font_style(metadata);
        TokenPresentation {
            foreground: Self::foreground(metadata),
            italic: style.contains(FontStyle::ITALIC),
            bold: style.contains(FontStyle::BOLD),
            underline: style.contains(FontStyle::UNDERLINE),
            strikethrough: style.contains(FontStyle::STRIKETHROUGH),
        }
    }

    /// Packs fields into a metadata word. Each field is masked to its width, so out-of-range
    /// values cannot overwrite neighboring fields.
    pub fn pack(
        language_id: EncodedLanguageId,
        token_type: StandardTokenType,
        balanced_brackets: bool,
        font_style: FontStyle,
        foreground: ColorId,
        background: ColorId,
    ) -> u32 {
        let token_type: u32 = match token_type {
            StandardTokenType::Other => 0,
            StandardTokenType::Comment => 1,
            StandardTokenType::String => 2,
            StandardTokenType::RegEx => 3,
        };
        ((language_id << metadata::LANGUAGEID_OFFSET) & metadata::LANGUAGEID_MASK)
            | ((token_type << metadata::TOKEN_TYPE_OFFSET) & metadata::TOKEN_TYPE_MASK)
            | (u32::from(balanced_brackets) << metadata::BALANCED_BRACKETS_OFFSET)
            | ((font_style.0 << metadata::FONT_STYLE_OFFSET) & metadata::FONT_STYLE_MASK)
            | ((foreground.0 << metadata::FOREGROUND_OFFSET) & metadata::FOREGROUND_MASK)
            | ((background.0 << metadata::BACKGROUND_OFFSET) & metadata::BACKGROUND_MASK)
    }
}

/// Metadata of a token with no language, type or style, colored with the defaults. Matches
/// `LineTokens.defaultTokenMetadata`.
pub const DEFAULT_TOKEN_METADATA: u32 = (FontStyle::NONE.0 << metadata::FONT_STYLE_OFFSET)
    | (ColorId::DEFAULT_FOREGROUND.0 << metadata::FOREGROUND_OFFSET)
    | (ColorId::DEFAULT_BACKGROUND.0 << metadata::BACKGROUND_OFFSET);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_do_not_overlap_and_cover_the_word() {
        let all = [
            metadata::LANGUAGEID_MASK,
            metadata::TOKEN_TYPE_MASK,
            metadata::BALANCED_BRACKETS_MASK,
            metadata::FONT_STYLE_MASK,
            metadata::FOREGROUND_MASK,
            metadata::BACKGROUND_MASK,
        ];
        let mut seen = 0u32;
        for mask in all {
            assert_eq!(seen & mask, 0, "masks overlap");
            seen |= mask;
        }
        assert_eq!(seen, u32::MAX);
    }

    #[test]
    fn pack_then_read_round_trips_every_field() {
        let word = TokenMetadata::pack(
            200,
            StandardTokenType::RegEx,
            true,
            FontStyle::ITALIC | FontStyle::STRIKETHROUGH,
            ColorId(300),
            ColorId(150),
        );
        assert_eq!(TokenMetadata::language_id(word), 200);
        assert_eq!(TokenMetadata::token_type(word), StandardTokenType::RegEx);
        assert!(TokenMetadata::contains_balanced_brackets(word));
        assert_eq!(
            TokenMetadata::font_style(word),
            FontStyle(FontStyle::ITALIC.0 | FontStyle::STRIKETHROUGH.0)
        );
        assert_eq!(TokenMetadata::foreground(word), ColorId(300));
        assert_eq!(TokenMetadata::background(word), ColorId(150));
    }

    #[test]
    fn pack_masks_out_of_range_fields() {
        // A 9-bit foreground of 512 must not spill into the background field.
        let word = TokenMetadata::pack(
            0,
            StandardTokenType::Other,
            false,
            FontStyle::NONE,
            ColorId(512),
            ColorId::NONE,
        );
        assert_eq!(TokenMetadata::foreground(word), ColorId::NONE);
        assert_eq!(TokenMetadata::background(word), ColorId::NONE);
    }

    #[test]
    fn known_layout_values() {
        // Foreground 1 sits at bit 15, as the upstream LineTokens tests assume (32768).
        assert_eq!(1u32 << metadata::FOREGROUND_OFFSET, 32768);
        assert_eq!(TokenMetadata::foreground(32768), ColorId(1));
        assert_eq!(TokenMetadata::token_type(0x100), StandardTokenType::Comment);
        assert_eq!(TokenMetadata::token_type(0x200), StandardTokenType::String);
        assert_eq!(TokenMetadata::token_type(0x300), StandardTokenType::RegEx);
        assert!(TokenMetadata::contains_balanced_brackets(0x400));
    }

    #[test]
    fn presentation_reads_flags() {
        let word = TokenMetadata::pack(
            0,
            StandardTokenType::Other,
            false,
            FontStyle::BOLD | FontStyle::UNDERLINE,
            ColorId(4),
            ColorId::NONE,
        );
        assert_eq!(
            TokenMetadata::presentation(word),
            TokenPresentation {
                foreground: ColorId(4),
                italic: false,
                bold: true,
                underline: true,
                strikethrough: false,
            }
        );
    }

    #[test]
    fn default_metadata_has_default_colors() {
        assert_eq!(
            TokenMetadata::foreground(DEFAULT_TOKEN_METADATA),
            ColorId(1)
        );
        assert_eq!(
            TokenMetadata::background(DEFAULT_TOKEN_METADATA),
            ColorId(2)
        );
        assert_eq!(
            TokenMetadata::font_style(DEFAULT_TOKEN_METADATA),
            FontStyle::NONE
        );
    }
}
