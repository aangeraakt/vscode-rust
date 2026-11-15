//! List widget models, ported from `src/vs/base/browser/ui/list/`.

pub mod range_map;

pub use range_map::{IndexRange, RangeMap, RangedGroup, consolidate, group_intersect, shift};
