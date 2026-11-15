# ADR 0002: UI toolkit

Status: accepted (Phase 0)

## Context

The editor needs fast text rendering: thousands of glyphs per frame, smooth scrolling through a
1M-line file (M1), per-character decorations, a minimap, and pixel-exact cursor placement.
The workbench needs VS Code's own layout model (a serializable split grid, sashes, composite
parts, tabs, quick input overlays). VS Code itself uses no widget toolkit: it draws its own
widgets on raw DOM, and its editor uses its own virtualized line rendering
(`src/vs/editor/browser/view/`, `src/vs/base/browser/ui/grid/`).

Candidates:

| Option | Text performance | Fit with VS Code's layout model | Risk |
|---|---|---|---|
| iced | good (cosmic-text) | Elm architecture fights VS Code's mutable, event-driven services; custom widgets needed anyway | medium |
| egui | immediate mode, re-lays out text every frame; weak for 1M lines + minimap | poor; no retained layout tree | high for editor |
| Slint | declarative DSL; editor would be a custom renderer inside it | poor for dynamic grid / views | medium |
| custom (winit + wgpu + cosmic-text via glyphon) | full control over glyph atlas and virtualization | direct port of VS Code's grid and view parts | highest effort, lowest ceiling risk |

## Decision

Custom renderer: **winit** for windows and input, **wgpu** for drawing, **cosmic-text**
(shaping, fallback, layout) with **glyphon** as the glyph-atlas renderer, and **accesskit** for
accessibility. On top of that, Lathe has a small retained widget layer that mirrors VS Code's
`base/browser/ui` building blocks (grid, split view, list, scrollable element, input box).

Ownership:
- `platform` (role 2): window, event loop, DPI, clipboard, dialogs (`rfd`), menus (`muda`),
  keychain (`keyring`).
- `editor-view` (role 5): glyph layout, line virtualization, minimap, decorations.
- `workbench` (role 7): widget layer, grid layout, parts, theming.

The draw layer API (rects, glyph runs, clip, layers) is defined by `platform` and approved by the
Architect before roles 5 and 7 depend on it.

## Consequences

- More up-front work for basic widgets, but no toolkit to fight while porting VS Code's layout.
- Text rendering, a hot spot for the 1M-line budget, is fully under our control.
- Accessibility is our job (accesskit); it is an acceptance item for M2.
