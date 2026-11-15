//! Maps string language ids to the 8-bit ids stored in token metadata, and back.
//!
//! Ported from `LanguageIdCodec` in `src/vs/editor/common/services/languagesRegistry.ts`.
//! Id 0 is the null language and id 1 is plain text; registered languages get 2, 3, ...
//!
//! Divergence: the TypeScript codec silently overflows the 8-bit field after 255 languages. Here
//! [`LanguageIdCodec::register`] panics instead, since a wrapped id would decode to the wrong
//! language.

use crate::encoded_token_attributes::{EncodedLanguageId, metadata::LANGUAGEID_MASK};
use std::collections::HashMap;
use vscode_api::LanguageId;

/// The id of the null language (`NULL_LANGUAGE_ID`).
pub const NULL_LANGUAGE: &str = "vs.editor.nullLanguage";
/// The id of plain text (`PLAINTEXT_LANGUAGE_ID`).
pub const PLAINTEXT_LANGUAGE: &str = "plaintext";

/// Bidirectional map between language id strings and encoded ids.
#[derive(Debug)]
pub struct LanguageIdCodec {
    by_encoded: Vec<LanguageId>,
    by_name: HashMap<LanguageId, EncodedLanguageId>,
}

impl Default for LanguageIdCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIdCodec {
    /// Creates a codec holding only the null language and plain text.
    pub fn new() -> Self {
        let mut codec = Self {
            by_encoded: Vec::new(),
            by_name: HashMap::new(),
        };
        codec.insert(NULL_LANGUAGE.into(), 0);
        codec.insert(PLAINTEXT_LANGUAGE.into(), 1);
        codec
    }

    fn insert(&mut self, language: LanguageId, encoded: EncodedLanguageId) {
        let index = encoded as usize;
        if self.by_encoded.len() <= index {
            self.by_encoded
                .resize(index + 1, LanguageId::from(NULL_LANGUAGE));
        }
        self.by_encoded[index] = language.clone();
        self.by_name.insert(language, encoded);
    }

    /// Assigns the next id to `language`. Registering a known language does nothing.
    ///
    /// # Panics
    /// If more than 255 languages would be registered.
    pub fn register(&mut self, language: impl Into<LanguageId>) {
        let language = language.into();
        if self.by_name.contains_key(&language) {
            return;
        }
        let encoded = self.by_encoded.len() as EncodedLanguageId;
        assert!(
            encoded <= LANGUAGEID_MASK,
            "language id space exhausted: metadata holds 8 bits"
        );
        self.insert(language, encoded);
    }

    /// The encoded id of `language`, or the null id (0) when it is not registered
    /// (`encodeLanguageId`).
    pub fn encode(&self, language: &str) -> EncodedLanguageId {
        self.by_name.get(language).copied().unwrap_or(0)
    }

    /// The language registered under `encoded`, or the null language when there is none
    /// (`decodeLanguageId`).
    pub fn decode(&self, encoded: EncodedLanguageId) -> LanguageId {
        self.by_encoded
            .get(encoded as usize)
            .cloned()
            .unwrap_or_else(|| LanguageId::from(NULL_LANGUAGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_ids_are_null_and_plaintext() {
        let codec = LanguageIdCodec::new();
        assert_eq!(codec.encode(NULL_LANGUAGE), 0);
        assert_eq!(codec.encode(PLAINTEXT_LANGUAGE), 1);
        assert_eq!(codec.decode(1).as_str(), PLAINTEXT_LANGUAGE);
    }

    #[test]
    fn registered_languages_get_sequential_ids() {
        let mut codec = LanguageIdCodec::new();
        codec.register("rust");
        codec.register("typescript");
        codec.register("rust");
        assert_eq!(codec.encode("rust"), 2);
        assert_eq!(codec.encode("typescript"), 3);
        assert_eq!(codec.decode(3).as_str(), "typescript");
    }

    #[test]
    fn unknown_ids_fall_back_to_null() {
        let codec = LanguageIdCodec::new();
        assert_eq!(codec.encode("missing"), 0);
        assert_eq!(codec.decode(200).as_str(), NULL_LANGUAGE);
    }
}
