# Custom Tab Shape Design

**Date:** 2026-02-28
**Status:** Approved

## Goal

Replace the default egui_tiles rectangular tab buttons with a custom shape:
- Top-left and top-right corners rounded (6 px radius)
- Bottom-right corner chamfered (10 px diagonal cut)
- Bottom-left corner square, flush with the panel content area
- Active tab blends seamlessly into the pane below; inactive tabs recede into the tab bar

## Scope

Active tab only. Inactive tabs use a simpler rounded-top rectangle.
All changes confined to `src/dock.rs`.

## Shape & Geometry

The active tab is a convex polygon built by `build_tab_path(rect, corner_radius, chamfer)`:

```
 ╭───────────────╮
 │   Aircraft     ╲   ← 10 px chamfer diagonal
─┘                 ╲──────────────────────────
  ↑
  6 px rounded corners (top-left, top-right)
```

Vertex order (clockwise):
1. Top-left arc — 8-segment polyline approximating a 6 px quarter-circle
2. Top edge — straight
3. Top-right arc — 8-segment polyline approximating a 6 px quarter-circle
4. Right edge — straight down to `(max.x, max.y - chamfer)`
5. Chamfer — diagonal to `(max.x - chamfer, max.y)`
6. Bottom edge — straight left to `(min.x, max.y)`
7. Left edge — straight up, closing the polygon

The polygon is convex; `Shape::convex_polygon` is used for efficient rendering.

Inactive tabs use `Shape::Rect` with `Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }`.

### Constants

```rust
const TAB_CORNER_RADIUS: f32 = 6.0;
const TAB_CHAMFER:        f32 = 10.0;
const TAB_H_PAD:          f32 = 10.0;  // left/right text padding
const TAB_V_PAD:          f32 = 4.0;   // top/bottom text padding
```

Tab width = `text_width + 2 * TAB_H_PAD + close_button_width (if closable) + TAB_CHAMFER`
(The chamfer size is added to the right pad so text is not clipped by the diagonal edge.)

## Colors

| Element | Color | Notes |
|---------|-------|-------|
| Active tab fill | `bg_primary` | Same as pane content — no visible seam at bottom |
| Inactive tab fill | `bg_secondary` at 120α | Blends into tab bar background |
| Inactive hovered fill | `bg_primary` at 80α | Subtle lift on hover |
| Active tab text | `text_primary` | Unchanged from current |
| Inactive tab text | `text_dim` | Unchanged from current |
| Tab bar background | `bg_secondary` | Unchanged — provided by `tab_bar_color` |

## Separator Line

Override `tab_bar_hline_stroke` → `Stroke::NONE`.

Visual separation between tab bar and content is achieved by color contrast alone:
the active tab's `bg_primary` fill merges flush with the panel, and the `bg_secondary`
tab bar background provides implicit separation for the inactive areas.

## Interaction Handling

`tab_ui` allocates a single response rect with `Sense::click_and_drag()`. egui_tiles
reads this response to detect tab selection (click) and drag-to-reorder (drag).

Paint order within `tab_ui`:
1. Background polygon (painter call — no layout cost)
2. Title text (painter call)
3. Close button (child widget, only when `is_tab_closable` returns true)

The close button consumes its own click via `on_tab_close`; it does not also
trigger tab selection because egui processes inner widgets before the outer response.

## Implementation Location

All changes in `src/dock.rs`, `impl Behavior<DockPane> for DockBehavior<'_>`:

| Addition | Type |
|----------|------|
| `tab_ui` | New method override |
| `tab_bar_hline_stroke` | New method override (returns `Stroke::NONE`) |
| `build_tab_path` | Private helper function |
| `TAB_CORNER_RADIUS`, `TAB_CHAMFER`, `TAB_H_PAD`, `TAB_V_PAD` | Constants |

No new files. No changes to `CachedThemeColors`, `tab_bg_color`, `tab_text_color`,
or `tab_bar_color` (those color methods become unused once `tab_ui` owns its own painting).
