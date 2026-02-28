# BRP Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable the Bevy Remote Protocol so Claude Code can inspect and control the running AirJedi app at runtime.

**Architecture:** A `brp` cargo feature flag (default-on) gates a new `src/brp.rs` plugin module that wraps `RemotePlugin`, `RemoteHttpPlugin`, and `BrpExtrasPlugin`. The macOS bundle build excludes BRP.

**Tech Stack:** Bevy 0.18 (`bevy_remote` feature), `bevy_brp_extras` 0.18, `bevy_brp_mcp` (already installed)

**Design doc:** `docs/plans/2026-02-28-brp-integration-design.md`

---

### Task 1: Add BRP dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml:6-8` (features section)
- Modify: `Cargo.toml:31` (after last dependency)

**Step 1: Update features section**

Replace the existing features block:

```toml
[features]
default = ["hanabi"]
hanabi = ["dep:bevy_hanabi"]
```

With:

```toml
[features]
default = ["hanabi", "brp"]
hanabi = ["dep:bevy_hanabi"]
brp = ["bevy/bevy_remote", "dep:bevy_brp_extras"]
```

**Step 2: Add bevy_brp_extras dependency**

Add after the `bevy_hanabi` line in `[dependencies]`:

```toml
bevy_brp_extras = { version = "0.18", optional = true }
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully. New dependency downloads `bevy_brp_extras` and enables `bevy_remote` feature on bevy.

**Step 4: Commit**

```
git add Cargo.toml Cargo.lock
git commit -m "Add brp feature flag and bevy_brp_extras dependency"
```

---

### Task 2: Create src/brp.rs plugin module

**Files:**
- Create: `src/brp.rs`

**Step 1: Create the module**

Create `src/brp.rs` with:

```rust
use bevy::prelude::*;
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
use bevy_brp_extras::BrpExtrasPlugin;

pub struct BrpPlugin;

impl Plugin for BrpPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RemotePlugin::default(),
            RemoteHttpPlugin::default(),
            BrpExtrasPlugin::default(),
        ));
    }
}
```

**Step 2: Verify it compiles in isolation**

Run: `cargo check`
Expected: May fail because `mod brp` isn't declared in main.rs yet. That's fine — proceed to Task 3.

---

### Task 3: Wire BrpPlugin into main.rs

**Files:**
- Modify: `src/main.rs:38` (after `pub(crate) mod theme;`)
- Modify: `src/main.rs:161-235` (refactor App builder)

**Step 1: Add conditional module declaration**

After line 38 (`pub(crate) mod theme;`), add:

```rust
#[cfg(feature = "brp")]
mod brp;
```

**Step 2: Refactor App builder to `let mut app`**

The current code on line 161 is:

```rust
    App::new()
        .add_plugins(( ... ))
        ...
        .run();
```

Refactor to:

```rust
    let mut app = App::new();
    app.add_plugins(( ... ))
        ...
        ;

    #[cfg(feature = "brp")]
    app.add_plugins(brp::BrpPlugin);

    app.run();
```

Key changes:
- Line 161: `App::new()` becomes `let mut app = App::new();`
- Line 234: Remove `.run();` from the end of the chain, replace with `;`
- After line 234: Add the `#[cfg]` block and `app.run();`

**Step 3: Verify it compiles with BRP enabled**

Run: `cargo check`
Expected: Compiles successfully.

**Step 4: Verify it compiles with BRP disabled**

Run: `cargo check --no-default-features -F hanabi`
Expected: Compiles successfully without BRP.

**Step 5: Commit**

```
git add src/brp.rs src/main.rs
git commit -m "Add BrpPlugin with conditional registration in main"
```

---

### Task 4: Exclude BRP from macOS bundle build

**Files:**
- Modify: `macos/scripts/build-app.sh:30`

**Step 1: Update cargo build command**

On line 30 of `macos/scripts/build-app.sh`, change:

```bash
(cd "$ROOT_DIR" && cargo build --release)
```

To:

```bash
(cd "$ROOT_DIR" && cargo build --release --no-default-features -F hanabi)
```

**Step 2: Commit**

```
git add macos/scripts/build-app.sh
git commit -m "Exclude BRP from macOS app bundle release build"
```

---

### Task 5: Document BRP in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (dependencies section)

**Step 1: Add BRP to the dependencies table**

In the "Key dependencies" section, add after the `bevy_hanabi` entry:

```markdown
- `bevy_brp_extras = "0.18"` (optional `brp` feature, default on): BRP extras for remote inspection, screenshots, input simulation
```

**Step 2: Add a BRP section**

Add a new section after "Map Tile Caching" (before "3D Tile Rendering — Known Pitfalls"):

```markdown
## Bevy Remote Protocol (BRP)

The `brp` feature flag (default-on) enables runtime inspection and control of the app via the Bevy Remote Protocol. This allows AI coding assistants to query entities, inspect components, take screenshots, and simulate input through the `bevy_brp_mcp` MCP server.

- **Feature flag:** `brp` (disable with `--no-default-features -F hanabi`)
- **HTTP endpoint:** `localhost:15702` (default, not configurable)
- **MCP server:** Configured in `.mcp.json`, uses `bevy_brp_mcp` binary
- **macOS bundle:** BRP is excluded from release app bundles
```

**Step 3: Commit**

```
git add CLAUDE.md
git commit -m "Document BRP feature flag in CLAUDE.md"
```

---

### Task 6: Smoke test — run app and verify BRP responds

**Step 1: Run the app**

Run: `cargo run`
Expected: App launches normally. Look for log output indicating BRP HTTP server is listening (may show a line about port 15702).

**Step 2: Test BRP endpoint**

In a separate terminal, run:

```bash
curl -s http://localhost:15702 -d '{"jsonrpc":"2.0","id":1,"method":"world/list_components","params":{}}' -H 'Content-Type: application/json' | head -c 200
```

Expected: JSON response with a list of registered component types.

**Step 3: Test MCP tools (optional)**

If the app is running, try using the BRP MCP tools from Claude Code:
- `brp_status` should report `running_with_brp`
- `world_list_components` should return component types
- `brp_extras_screenshot` should return a screenshot

**Step 4: Stop the app and verify no-brp build**

Run: `cargo run --no-default-features -F hanabi`
Expected: App launches normally. The BRP HTTP server should NOT be listening — `curl` to port 15702 should fail.
