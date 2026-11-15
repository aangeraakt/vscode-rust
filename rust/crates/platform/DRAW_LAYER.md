# Draw layer: proposal (role 02)

Status: **proposal, not approved.** Roles 05 and 07 must not depend on these types until the
Architect approves the request in `COORDINATION.md`. ADR 0002 says the draw layer is defined by
`platform` and approved before roles 05 and 07 depend on it.

This is a Rust sketch for review. It is not compiled code. It becomes code in
`lathe-platform` after approval.

## Purpose

Roles 05 (editor view) and 07 (workbench widgets) build a `DrawList` for each frame. The
wgpu renderer consumes it. Neither role calls wgpu, and the renderer knows nothing about
editors or widgets.

```
editor-view (05) ─┐
                  ├─> DrawList ─> renderer (wgpu + glyphon, in platform) ─> window surface
workbench (07) ───┘
```

## Units and conventions

- Coordinates are **logical pixels** in window space, with the origin at the top-left of the
  content area. The renderer multiplies by the window scale factor. Callers never see physical
  pixels.
- Colors are sRGB, straight (non-premultiplied) alpha, `u8` per channel.
- A `DrawList` is built on the UI thread and handed to the renderer by value. It is not shared
  between threads, so it needs no locks.
- Glyph shaping happens in the caller's layer with cosmic-text (ADR 0002). The `DrawList`
  carries shaped glyph ids and positions, not strings. The renderer rasterizes them into the
  glyph atlas.

## Sketch

```rust
/// Logical-pixel point in window space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point { pub x: f32, pub y: f32 }

/// Logical-pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect { pub x: f32, pub y: f32, pub width: f32, pub height: f32 }

/// sRGB color, straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

/// Identifies a font face. The renderer owns the font database; callers get keys from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontKey(pub u32);

/// One shaped glyph. `x` is relative to the run origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub id: u16,      // glyph index in `font`
    pub font: FontKey,
    pub x: f32,       // advance-based offset from the run origin
    pub y: f32,       // baseline offset from the run origin
}

/// Shaped text drawn at `origin` (baseline-left). A run uses one color.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    pub origin: Point,
    pub color: Color,
    pub glyphs: Vec<Glyph>,
}

/// One drawing operation inside a layer. Clips are a stack: `PushClip` and `PopClip` must be
/// balanced within the layer. Unbalanced lists are rejected when the list is validated.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    FillRect { rect: Rect, color: Color },
    GlyphRun(GlyphRun),
    PushClip(Rect),
    PopClip,
}

/// Stable identifier for a layer, so the renderer can cache its contents between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub u32);

/// A group of commands drawn in order. Layers are drawn in ascending `z`, with ties broken by
/// `id`. Each layer starts with an empty clip stack.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub id: LayerId,
    pub z: i32,
    pub commands: Vec<DrawCommand>,
}

/// Everything drawn in one frame for one window.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList {
    pub layers: Vec<Layer>,
}

impl DrawList {
    /// Checks the invariants the renderer relies on: balanced clips per layer and unique layer
    /// ids. Returns an error that names the layer otherwise.
    pub fn validate(&self) -> Result<(), DrawListError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawListError {
    DuplicateLayer(LayerId),
    UnbalancedClip(LayerId),
}
```

## Renderer contract

- The renderer draws layers in `(z, id)` order. Within a layer, it runs commands in order.
- Clip stacks are intersected. A `PushClip` rectangle is clipped to the current clip.
- The renderer may cache a layer by `LayerId` when its command list is unchanged. The caller
  must then reissue the layer with a new command list, not mutate it in place.
- Text is drawn only from `GlyphRun`. A `FillRect` cannot draw text, and a glyph run cannot
  draw a rectangle.

## Not in this proposal

- Images and textures (needed for the minimap and icons later; add a `DrawCommand` variant when
  a role needs it).
- Shadows, gradients, rounded corners, and path strokes.
- Hit testing and accessibility. Those belong to the widget layer and to accesskit (ADR 0002).
- The glyph atlas and shaping cache. They belong to the renderer and to role 05.

## Open questions for the Architect

1. Is `DrawList` built per window or per frame? This sketch assumes one per window per frame.
2. Glyph ids: are they per font face (as above) or global? The sketch uses the face-local
   index with a `FontKey`.
3. Is the renderer part of `platform` (as this sketch assumes) or a separate crate that
   depends on `platform`?
4. `f32` for geometry and `u8` for colors: acceptable for the 1M-line budget, or does the
   editor need a different layout unit?
