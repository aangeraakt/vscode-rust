//! Tokenization and language configuration for Lathe.
//!
//! Owned by role 06 (Language Services). Pure logic only: no I/O, no UI.
//!
//! Modules:
//! - [`encoded_token_attributes`]: packed per-token metadata (`encodedTokenAttributes.ts`).
//! - [`language_id_codec`]: string language ids to the 8-bit ids stored in metadata
//!   (`services/languagesRegistry.ts`, `LanguageIdCodec`).
//! - [`line_tokens`]: the tokens of one line (`editor/common/tokens/lineTokens.ts`).
//!
//! Offsets follow the text model (ADR 0003): token offsets count UTF-16 code units, as they do
//! in the TypeScript source.

pub mod encoded_token_attributes;
pub mod language_id_codec;
pub mod line_tokens;

pub use encoded_token_attributes::{ColorId, FontStyle, StandardTokenType, TokenMetadata};
pub use language_id_codec::LanguageIdCodec;
pub use line_tokens::{LineTokens, SlicedLineTokens, ViewLineTokens};
