//! Font size and line height in pixels, without text measurement.
//!
//! Ported from `src/vs/editor/common/config/fontInfo.ts`: `GOLDEN_LINE_HEIGHT_RATIO`,
//! `MINIMUM_LINE_HEIGHT`, `BareFontInfo._create` (line height rules, rounding, minimum and zoom),
//! and the platform parts of `EDITOR_FONT_DEFAULTS`. Measuring glyphs (`FontInfo`) belongs to
//! the renderer and is not ported here.
//!
//! The platform and zoom level are parameters, so the rules are testable on every host.
//! Tests cover the line height and zoom rules of `BareFontInfo._create`; no upstream test
//! covers them directly.

use crate::environment::{Environment, Platform};

/// The line height used when the setting is 0, as a multiple of the font size
/// (`GOLDEN_LINE_HEIGHT_RATIO`: 1.5 on macOS, 1.35 elsewhere).
pub fn golden_line_height_ratio(platform: Platform) -> f64 {
    match platform {
        Platform::MacOs => 1.5,
        Platform::Windows | Platform::Linux => 1.35,
    }
}

/// Smallest line height in pixels (`MINIMUM_LINE_HEIGHT`). Smaller settings are multiples of the
/// font size.
pub const MINIMUM_LINE_HEIGHT: f64 = 8.0;

/// The default font size in pixels (`EDITOR_FONT_DEFAULTS.fontSize`: 12 on macOS, 14 elsewhere).
pub fn default_font_size(platform: Platform) -> f64 {
    match platform {
        Platform::MacOs => 12.0,
        Platform::Windows | Platform::Linux => 14.0,
    }
}

/// The font size and line height after the rules of `BareFontInfo._create`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// The font size in pixels, with the zoom applied.
    pub font_size: f64,
    /// The line height in pixels, rounded, at least [`MINIMUM_LINE_HEIGHT`] before zoom, with the
    /// zoom applied.
    pub line_height: f64,
}

/// Computes the pixel font size and line height for validated settings.
///
/// A `line_height` of 0 is computed from the font size with [`golden_line_height_ratio`]. Values
/// below [`MINIMUM_LINE_HEIGHT`] are multiples of the font size. The result is rounded and raised to
/// the minimum. The zoom level then scales both values by `1 + zoom_level * 0.1`.
pub fn bare_font_metrics(font_size: f64, line_height: f64, env: &Environment) -> FontMetrics {
    let mut line_height = if line_height == 0.0 {
        golden_line_height_ratio(env.platform) * font_size
    } else if line_height < MINIMUM_LINE_HEIGHT {
        line_height * font_size
    } else {
        line_height
    };

    // `Math.round` rounds halves up, which matches `round` for the non-negative values here.
    line_height = line_height.round().max(MINIMUM_LINE_HEIGHT);

    let multiplier = 1.0 + env.zoom_level * 0.1;
    FontMetrics {
        font_size: font_size * multiplier,
        line_height: line_height * multiplier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(platform: Platform, zoom_level: f64) -> Environment {
        Environment {
            platform,
            zoom_level,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn default_font_size_per_platform() {
        assert_close(default_font_size(Platform::MacOs), 12.0);
        assert_close(default_font_size(Platform::Windows), 14.0);
        assert_close(default_font_size(Platform::Linux), 14.0);
    }

    #[test]
    fn zero_line_height_uses_golden_ratio_per_platform() {
        // macOS: 12 * 1.5 = 18. Windows: 14 * 1.35 = 18.9, rounded to 19.
        let mac = bare_font_metrics(12.0, 0.0, &env(Platform::MacOs, 0.0));
        assert_close(mac.line_height, 18.0);
        let windows = bare_font_metrics(14.0, 0.0, &env(Platform::Windows, 0.0));
        assert_close(windows.line_height, 19.0);
    }

    #[test]
    fn small_line_height_is_a_multiple_of_the_font_size() {
        let metrics = bare_font_metrics(14.0, 1.5, &env(Platform::Linux, 0.0));
        assert_close(metrics.line_height, 21.0);
    }

    #[test]
    fn pixel_line_height_is_kept_and_rounded() {
        let metrics = bare_font_metrics(14.0, 20.4, &env(Platform::Linux, 0.0));
        assert_close(metrics.line_height, 20.0);
        let metrics = bare_font_metrics(14.0, 20.6, &env(Platform::Linux, 0.0));
        assert_close(metrics.line_height, 21.0);
    }

    #[test]
    fn line_height_has_a_minimum_of_eight_pixels() {
        // 0.5 * 6 = 3, raised to the minimum of 8.
        let metrics = bare_font_metrics(6.0, 0.5, &env(Platform::Linux, 0.0));
        assert_close(metrics.line_height, 8.0);
    }

    #[test]
    fn zoom_scales_font_size_and_line_height() {
        let metrics = bare_font_metrics(14.0, 20.0, &env(Platform::Linux, 1.0));
        assert_close(metrics.font_size, 15.4);
        assert_close(metrics.line_height, 22.0);
        let metrics = bare_font_metrics(14.0, 20.0, &env(Platform::Linux, -5.0));
        assert_close(metrics.font_size, 7.0);
        assert_close(metrics.line_height, 10.0);
    }
}
