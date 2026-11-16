# Architecture

Ember is a single crate (`ember`) exposing a library (`src/lib.rs`) and a thin binary (`src/main.rs`). The UI is immediate-mode (egui via eframe).

| Module | Responsibility |
| --- | --- |
| `app` | Application state, commands (`Cmd`), shortcuts, per-frame housekeeping, screenshots |
| `panels` | Menu, status bar, activity bar, side panels, bottom panel, tabs, welcome screen |
| `dialogs` | Palette, find bar, settings, about, confirmations, toasts |
| `editor` | Text editor widget: gutter, overlays, smart editing keys |
| `highlight` | syntect used only as a scope parser, mapped to Ember's own token classes; incremental per-line cache |
| `theme` | Built-in themes and TOML loader |
| `icons`, `glyph` | Code-drawn file icons and toolbar glyphs |
| `explorer`, `search`, `terminal` | File tree, threaded project search, command runner |
| `plugins` | Rhai host, hooks and API |
| `dap` | Debug Adapter Protocol client over stdio and the `Debugger` state machine |
| `dap_mock` | Scripted adapter (`ember --mock-dap`) for end-to-end tests |
| `presence` | Discord rich presence on a background thread |
| `config` | `config.toml` persistence |

## Testing

- Unit tests live next to modules (`*_tests.rs`), including a headless egui harness that injects key events into the editor.
- `tests/debugger_session.rs` drives a full debug session (breakpoint → stack → variables → step → exit) against the mock adapter.

## Custom theme keys

See `src/theme.rs` — a theme TOML mirrors the `Theme` struct's colour fields as `#rrggbb` strings.
