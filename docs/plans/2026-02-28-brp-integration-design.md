# BRP Integration Design

**Date:** 2026-02-28
**Issue:** #15 — Add Bevy Remote Protocol (BRP) plugin for MCP server integration

## Goal

Enable the Bevy Remote Protocol so AI coding assistants can inspect and modify entities, components, and resources at runtime via the `bevy_brp_mcp` MCP server.

## Architecture

A single `brp` cargo feature flag (default-on) gates all BRP functionality. A new `src/brp.rs` module wraps the three required plugins into a single `BrpPlugin`.

### Components

1. **RemotePlugin** (from `bevy::remote`) — Core BRP protocol implementation
2. **RemoteHttpPlugin** (from `bevy::remote::http`) — HTTP transport on localhost:15702
3. **BrpExtrasPlugin** (from `bevy_brp_extras` crate) — Screenshots, input simulation, diagnostics, macOS gestures

### Data Flow

```
Claude Code → bevy_brp_mcp (MCP stdio server) → HTTP localhost:15702 → RemotePlugin → ECS
```

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Compile-time gating | Feature flag `brp` | Can be excluded from release bundles; default-on for dev |
| Module location | New `src/brp.rs` | Follows existing plugin-per-module pattern |
| Extras scope | Full suite | Screenshots, input sim, diagnostics, macOS gestures all useful for AI debugging |
| HTTP port | Default 15702 | Standard BRP port; MCP server expects it |
| macOS bundle | Exclude BRP | Build with `--no-default-features -F hanabi` to strip HTTP listener |

## Changes

### Cargo.toml

```toml
[features]
default = ["hanabi", "brp"]
hanabi = ["dep:bevy_hanabi"]
brp = ["bevy/bevy_remote", "dep:bevy_brp_extras"]

[dependencies]
bevy_brp_extras = { version = "0.18", optional = true }
```

### src/brp.rs (new)

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

### src/main.rs

- Add `#[cfg(feature = "brp")] mod brp;`
- Refactor `App::new()...run()` chain to `let mut app = App::new(); ... app.run();`
- Add `#[cfg(feature = "brp")] app.add_plugins(brp::BrpPlugin);` before `.run()`

### macos/Makefile

Update the release build command to exclude BRP:
```makefile
cargo build --release --no-default-features -F hanabi
```

### CLAUDE.md

Document the `brp` feature flag in the dependencies section.

## Existing Infrastructure

- `.mcp.json` already configures `bevy_brp_mcp` as a stdio MCP server — no changes needed
- `bevy_brp_mcp` binary already installed at `~/.cargo/bin/bevy_brp_mcp`

## Capabilities Once Integrated

**Core BRP:**
- Query entities, get/insert/remove/mutate components
- List all registered component types
- Spawn/despawn entities, reparent in hierarchy
- Get/insert/remove/mutate resources
- Trigger events, watch component changes

**Extras (via bevy_brp_extras):**
- Take screenshots of the running app
- Simulate keyboard input and text typing
- Simulate mouse clicks, drags, scrolling
- macOS trackpad gestures (pinch, rotation, double-tap)
- Get diagnostics (FPS, frame time)
- Set window title, shut down app
