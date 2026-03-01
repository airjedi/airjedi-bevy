# Custom Tab Shape Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the default egui_tiles rectangular tabs with a custom shape — rounded top corners, chamfered bottom-right — that makes the active tab appear to lift seamlessly from the pane content.

**Architecture:** Override `tab_ui` in `DockBehavior` to hand-build each tab using egui's painter. A private helper `build_tab_path` generates the convex polygon vertices (arc approximations for rounded corners + a diagonal chamfer). Inactive tabs use `painter.rect_filled` with `Rounding { nw, ne }`. The separator line between the tab bar and pane is suppressed entirely.

**Tech Stack:** Rust, egui 0.29, egui_tiles 0.14, `epaint::Shape::convex_polygon`, `egui::Painter`

---

### Task 1: Add constants and `build_tab_path` helper

**Files:**
- Modify: `src/dock.rs` — inside `impl Behavior<DockPane> for DockBehavior<'_>`

**Step 1: Add the four tab geometry constants**

Place these directly before the `pane_ui` method (inside the impl block):

```rust
const TAB_CORNER_RADIUS: f32 = 6.0;
const TAB_CHAMFER: f32 = 10.0;
const TAB_H_PAD: f32 = 10.0;
const TAB_V_PAD: f32 = 4.0;
```

**Step 2: Add the `build_tab_path` private helper**

Place this as a standalone `fn` outside the impl block (or as an associated function), just above the impl block:

```rust
/// Build the convex polygon path for a custom tab shape.
///
/// Vertices (clockwise in screen coords where Y increases downward):
///   top-left arc → top edge → top-right arc → right edge →
///   chamfer diagonal → bottom edge → (implicit left edge closes polygon)
fn build_tab_path(rect: egui::Rect, corner_radius: f32, chamfer: f32) -> Vec<egui::Pos2> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = corner_radius;
    let c = chamfer;
    let min = rect.min;
    let max = rect.max;
    const STEPS: usize = 8;
    let mut pts = Vec::with_capacity(STEPS * 2 + 7);

    // Top-left arc: center=(min.x+r, min.y+r), angles π → 3π/2
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let a = PI + t * FRAC_PI_2;
        pts.push(egui::pos2(min.x + r + r * a.cos(), min.y + r + r * a.sin()));
    }

    // Top-right arc: center=(max.x-r, min.y+r), angles 3π/2 → 2π
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let a = 3.0 * FRAC_PI_2 + t * FRAC_PI_2;
        pts.push(egui::pos2(max.x - r + r * a.cos(), min.y + r + r * a.sin()));
    }

    // Right edge to chamfer start, then chamfer diagonal
    pts.push(egui::pos2(max.x, max.y - c));
    pts.push(egui::pos2(max.x - c, max.y));

    // Bottom-left corner (square) — convex_polygon closes back to first point automatically
    pts.push(egui::pos2(min.x, max.y));

    pts
}
```

**Step 3: Verify the project compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no output (no errors). Warnings are fine.

**Step 4: Commit**

```bash
git add src/dock.rs
git commit -m "Add build_tab_path helper and tab geometry constants"
```

---

### Task 2: Suppress the tab bar separator line

**Files:**
- Modify: `src/dock.rs` — `impl Behavior<DockPane> for DockBehavior<'_>`

**Step 1: Add the override**

Add this method to the existing impl block (alongside `gap_width`, `tab_bar_color`, etc.):

```rust
fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
    egui::Stroke::NONE
}
```

**Step 2: Build and verify**

```bash
cargo build 2>&1 | grep -E "^error"
```

**Step 3: Run and take a screenshot**

Use BRP:
```
brp_extras_screenshot(path: "tmp/tab-hline-test.png")
```

Expected: the horizontal line separating tab bar from pane content should be gone. The pane content area should now read as continuous with the tab bar region.

**Step 4: Commit**

```bash
git add src/dock.rs
git commit -m "Suppress tab bar separator line for custom tab blending"
```

---

### Task 3: Implement `tab_ui` with inactive tab rendering

**Files:**
- Modify: `src/dock.rs` — `impl Behavior<DockPane> for DockBehavior<'_>`

**Step 1: Add the `tab_ui` override — inactive branch only**

Add this method to the impl block. The active branch is a placeholder `todo!()` temporarily:

```rust
fn tab_ui(
    &mut self,
    tiles: &mut egui_tiles::Tiles<DockPane>,
    ui: &mut egui::Ui,
    id: egui::Id,
    tile_id: egui_tiles::TileId,
    state: &egui_tiles::TabState,
) -> egui::Response {
    let title = self.tab_title_for_tile(tiles, tile_id);
    let is_closable = self.is_tab_closable(tiles, tile_id);
    let text_color = if state.active {
        self.colors.text_primary
    } else {
        self.colors.text_dim
    };
    let close_w = if is_closable { 18.0 } else { 0.0 };

    // Measure title text
    let galley = title.into_galley(ui, Some(false), f32::INFINITY, egui::TextStyle::Button);
    let text_w = galley.size().x;
    let tab_h = ui.available_height();
    let extra_right = if state.active { Self::TAB_CHAMFER } else { 0.0 };
    let tab_w = text_w + 2.0 * Self::TAB_H_PAD + close_w + extra_right;

    let (tab_rect, mut response) = ui.allocate_exact_size(
        egui::vec2(tab_w, tab_h),
        egui::Sense::click_and_drag(),
    );

    if ui.is_rect_visible(tab_rect) {
        let painter = ui.painter();

        if state.active {
            // Placeholder — will be replaced in Task 4
            painter.rect_filled(tab_rect, egui::Rounding::ZERO, self.colors.bg_primary);
        } else {
            // Inactive: rounded top corners only
            let hovered = response.hovered();
            let fill = if hovered {
                egui::Color32::from_rgba_unmultiplied(
                    self.colors.bg_primary.r(),
                    self.colors.bg_primary.g(),
                    self.colors.bg_primary.b(),
                    80,
                )
            } else {
                egui::Color32::from_rgba_unmultiplied(
                    self.colors.bg_secondary.r(),
                    self.colors.bg_secondary.g(),
                    self.colors.bg_secondary.b(),
                    120,
                )
            };
            painter.rect_filled(
                tab_rect,
                egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 },
                fill,
            );
        }

        // Paint title text (vertically centered, left-padded)
        let text_pos = egui::pos2(
            tab_rect.left() + Self::TAB_H_PAD,
            tab_rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley.galley, text_color);
    }

    self.on_tab_button(tiles, tile_id, response)
}
```

**Step 2: Build**

```bash
cargo build 2>&1 | grep -E "^error"
```

Fix any type errors. Common issues:
- `galley.galley` vs `galley` — `into_galley` returns `WidgetTextGalley`, its inner `Arc<Galley>` is at `.galley`; painting uses `painter.galley(pos, arc_galley, color)`
- If the API differs, use: `painter.text(text_pos, egui::Align2::LEFT_CENTER, display_name, egui::FontId::new(13.0, egui::FontFamily::Proportional), text_color)` as a fallback

**Step 3: Run and verify inactive tabs**

```
brp_extras_screenshot(path: "tmp/tab-inactive-test.png")
```

Expected: inactive tabs have rounded top corners with slight transparency. Active tab is a flat rectangle (placeholder). No close button yet.

**Step 4: Commit**

```bash
git add src/dock.rs
git commit -m "Add tab_ui override with inactive tab rounded-rect styling"
```

---

### Task 4: Add active tab custom polygon

**Files:**
- Modify: `src/dock.rs` — the `if state.active` branch inside `tab_ui`

**Step 1: Replace the active tab placeholder with the polygon**

Replace the `state.active` branch (the placeholder rect_filled call) with:

```rust
if state.active {
    let pts = build_tab_path(tab_rect, Self::TAB_CORNER_RADIUS, Self::TAB_CHAMFER);
    painter.add(egui::Shape::convex_polygon(
        pts,
        self.colors.bg_primary,
        egui::Stroke::NONE,
    ));
}
```

**Step 2: Build**

```bash
cargo build 2>&1 | grep -E "^error"
```

**Step 3: Run and verify the active tab shape**

```
brp_extras_screenshot(path: "tmp/tab-active-shape-test.png")
```

Expected:
- Active tab has rounded top-left and top-right corners
- Active tab has a diagonal chamfer cut at the bottom-right
- Active tab fill color matches the pane content background (`bg_primary`) — no visible seam at the bottom
- Inactive tabs are unchanged (rounded-top rectangles)

If the chamfer goes the wrong direction (inward vs outward), adjust the `build_tab_path` vertices. The chamfer diagonal goes from `(max.x, max.y - chamfer)` to `(max.x - chamfer, max.y)` — it cuts the bottom-right corner from the right edge into the bottom edge.

**Step 4: Commit**

```bash
git add src/dock.rs
git commit -m "Render active tab as custom polygon with rounded corners and chamfer"
```

---

### Task 5: Add close button

**Files:**
- Modify: `src/dock.rs` — after the text-painting block inside `tab_ui`

**Step 1: Add the close button painting and interaction**

Insert this block after the `painter.galley(...)` call and before the closing `}` of the `if ui.is_rect_visible(tab_rect)` block:

```rust
// Close button (only when closable)
if is_closable {
    // Position close button left of the chamfer zone so it stays on the flat part
    let close_x = tab_rect.right()
        - (if state.active { Self::TAB_CHAMFER + 2.0 } else { 2.0 })
        - 12.0;
    let close_center = egui::pos2(close_x, tab_rect.center().y);
    let close_rect = egui::Rect::from_center_size(close_center, egui::vec2(14.0, 14.0));

    let close_resp = ui.interact(close_rect, id.with("close"), egui::Sense::click());
    let close_color = if close_resp.hovered() {
        egui::Color32::from_rgb(220, 80, 60)
    } else {
        self.colors.text_dim
    };
    painter.text(
        close_center,
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(13.0),
        close_color,
    );
    if close_resp.clicked() {
        self.on_tab_close(tiles, tile_id);
    }
}
```

**Step 2: Build**

```bash
cargo build 2>&1 | grep -E "^error"
```

**Step 3: Run and verify close button**

```
brp_extras_screenshot(path: "tmp/tab-close-btn-test.png")
```

Expected:
- A `×` appears near the right edge of each tab
- On active tab, the `×` appears to the left of the chamfer, not clipped by the diagonal
- Clicking `×` hides the tab (existing `on_tab_close` behavior)

**Step 4: Commit**

```bash
git add src/dock.rs
git commit -m "Add close button to custom tab_ui with chamfer-aware positioning"
```

---

### Task 6: Visual verification across themes and states

**Step 1: Test Cockpit Dark theme**

Switch to Cockpit Dark in the Settings panel. Take screenshot:
```
brp_extras_screenshot(path: "tmp/tab-cockpit-dark.png")
```

Verify: active tab blends into pane, chamfer is clean, inactive tabs recede into bar.

**Step 2: Test Catppuccin Latte (light theme)**

Switch to Catppuccin Latte. Take screenshot:
```
brp_extras_screenshot(path: "tmp/tab-latte.png")
```

Verify: tab shapes are still visible on the light background. If inactive tab fill at 120α is invisible against the light bar, increase alpha to ~180.

**Step 3: Adjust alpha if needed**

If inactive tabs are invisible on light themes, change the inactive non-hovered fill alpha from `120` to a value that works on both dark and light. Try `160` as a middle ground.

**Step 4: Final commit**

```bash
git add src/dock.rs
git commit -m "Tune inactive tab alpha for cross-theme visibility"
```

---

### Reference: Arc Math Verification

If the arc polygon looks jagged or wrong, verify these key points:

| Corner | Center | Start angle | End angle | Start point | End point |
|--------|--------|------------|-----------|-------------|-----------|
| Top-left | `(min.x+r, min.y+r)` | `π` | `3π/2` | `(min.x, min.y+r)` | `(min.x+r, min.y)` |
| Top-right | `(max.x-r, min.y+r)` | `3π/2` | `2π` | `(max.x-r, min.y)` | `(max.x, min.y+r)` |

All angles are in radians using `f32::cos` / `f32::sin` (standard math convention, Y-down screen coords work correctly with these values).

The chamfer point sequence: `(max.x, max.y - chamfer)` → `(max.x - chamfer, max.y)`. This cuts the bottom-right corner diagonally, creating a 45° chamfer for equal `TAB_CHAMFER` values on both axes.
