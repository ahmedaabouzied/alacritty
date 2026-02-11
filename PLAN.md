# Claude Code Agent Prompt: Extend Alacritty with Built-in Terminal Multiplexing

## Role & Execution Model

You are a long-running autonomous agent extending [Alacritty](https://github.com/alacritty/alacritty) — a GPU-accelerated terminal emulator written in Rust — with built-in terminal multiplexer capabilities (panes, windows/screens, and session persistence). You will clone the repo, study the codebase, plan, implement, test, and iterate independently.

**Commit discipline is paramount.** Every single commit must be one atomic, logical unit of change. A commit should do exactly one thing: add one struct, implement one trait, wire one event, add one test. If you can split it smaller, do so. Run `cargo test` and `cargo clippy` before every commit. Never bundle unrelated changes.

If you hit a blocker, document it in `BLOCKERS.md`, attempt an alternative approach, and continue.

---

## Project Context

### Alacritty Architecture (What Already Exists)

Alacritty is a Cargo workspace with these crates:

| Crate | Role |
|---|---|
| `alacritty` | Main binary: windowing (winit), OpenGL rendering, event loop, input handling, display, IPC |
| `alacritty_terminal` | Core terminal emulation: grid, PTY, VT parser, scrollback, selection |
| `alacritty_config` | Configuration loading (TOML), hot-reload |
| `alacritty_config_derive` | Proc macros for config deserialization |

Key architectural facts you must respect:
- **Rendering**: OpenGL ES 2.0 via `glutin`/`glow`, with damage tracking. The renderer draws cells from a grid.
- **Event loop**: Central `EventLoop` in `alacritty/src/event.rs` processes winit events, PTY output, IPC messages, and config changes.
- **Terminal state**: `alacritty_terminal::Term<T>` holds the grid, cursor, scrollback. Currently ONE `Term` per window.
- **PTY**: `alacritty_terminal::tty` handles fork/exec. Currently ONE PTY per window.
- **Input dispatch**: Keyboard/mouse events go through `alacritty/src/input.rs` → action bindings → either fed to PTY or handled internally (vi mode, search, etc.).
- **Display**: `alacritty/src/display/mod.rs` orchestrates rendering. It reads from `Term` and draws via OpenGL.
- **IPC**: `alacritty msg` sends commands to running instances via a socket.

### What Alacritty Explicitly Lacks (And We're Adding)

Alacritty's FAQ states: *"You won't find things like tabs or splits (which are best left to a window manager or terminal multiplexer)."*

We're going against this philosophy to create a self-contained experience. We need to add:

1. **Panes** — Split the window horizontally/vertically, each pane running its own PTY + Term
2. **Windows (Screens/Tabs)** — Multiple named windows per session, each with its own pane layout
3. **Session Persistence** — Detach/reattach: background server keeps PTYs alive, client reconnects
4. **Status Bar** — Bottom bar showing session, window list, pane info
5. **Keybinding layer** — Configurable leader/prefix key (default `Ctrl-Space`, with `Ctrl-B` as a built-in alternative) that intercepts input for multiplexer commands. Supports tmux-compatible command syntax.

---

## Step 0: Repository Setup

```bash
git clone https://github.com/alacritty/alacritty.git
cd alacritty
git checkout -b feat/multiplexer
cargo build  # Verify clean build first
```

Commit: `chore: create feat/multiplexer branch from upstream master`

---

## Execution Plan: Micro-Commits

### Phase 1: Study & Document (No Code Changes Yet)

Before writing ANY code, thoroughly read and understand the following files. Take notes in `ARCHITECTURE_NOTES.md`:

1. `alacritty/src/event.rs` — The event loop. Understand `EventLoop`, `ActionContext`, how PTY output triggers redraws.
2. `alacritty/src/display/mod.rs` — How `Display` renders. How it gets terminal content from `Term`.
3. `alacritty/src/input.rs` — How keyboard events become PTY writes or actions. The `Binding` system.
4. `alacritty_terminal/src/term/mod.rs` — `Term` struct, grid access, cursor.
5. `alacritty_terminal/src/tty/` — PTY creation (platform-specific).
6. `alacritty/src/renderer/` — OpenGL drawing of cells, rects, and text.
7. `alacritty_config/src/` — How config is structured and loaded.

**Commit:** `docs: add architecture notes for multiplexer integration points`

### Phase 2: New Crate — `alacritty_multiplexer`

Create a new workspace crate that owns all multiplexer logic. This keeps changes to existing crates minimal and the new code testable in isolation.

```
alacritty_multiplexer/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── pane.rs          # Pane: id + reference to a Term + PTY handle
    ├── layout.rs        # Binary split tree for pane arrangement
    ├── split.rs         # Split/close operations on layout tree
    ├── resize.rs        # Resize pane with constraints
    ├── window.rs        # Window (tab): named, owns a layout + panes
    ├── session.rs       # Session: named, owns windows
    ├── persistence.rs   # Serialize/restore session layout to disk
    ├── command.rs       # MuxCommand enum (Split, Close, Navigate, etc.)
    ├── statusbar.rs     # Status bar content generation (not rendering)
    ├── rect.rs          # Rect math for pane regions
    └── error.rs         # MuxError type
```

Do this in many small commits:

#### 2a. Scaffold the crate
- Create `alacritty_multiplexer/Cargo.toml` with dependencies on `alacritty_terminal`, `serde`, `thiserror`, `uuid`
- Add it to workspace `Cargo.toml`
- Create `src/lib.rs` with module declarations

**Commit:** `feat(mux): scaffold alacritty_multiplexer crate and add to workspace`

#### 2b. Error type
- Define `MuxError` with variants: `LayoutError`, `PaneNotFound`, `WindowNotFound`, `SessionError`, `PersistenceError`, `IoError`

**Commit:** `feat(mux): define MuxError with thiserror`

#### 2c. Rect type
- `Rect { x: u16, y: u16, width: u16, height: u16 }` with helper methods: `contains`, `split_horizontal`, `split_vertical`
- Unit tests for all methods

**Commit:** `feat(mux): add Rect type with split helpers and tests`

#### 2d. Layout tree
- `LayoutNode` enum: `Leaf { pane_id: PaneId }` or `Split { direction: Direction, ratio: f32, first: Box<LayoutNode>, second: Box<LayoutNode> }`
- `Direction` enum: `Horizontal`, `Vertical`
- `PaneId` as a newtype wrapper around `u32`
- Methods: `find_pane(&self, id) -> bool`, `pane_ids(&self) -> Vec<PaneId>`, `pane_count(&self) -> usize`
- Tests for each method

**Commit:** `feat(mux): add LayoutNode tree and PaneId type with tests`

#### 2e. Rect calculation from layout
- `calculate_rects(node: &LayoutNode, area: Rect) -> HashMap<PaneId, Rect>` — walk the tree, compute each pane's screen region
- Property: all rects must tile the area exactly (no gaps, no overlaps)
- Tests including edge cases (single pane, deep nesting)

**Commit:** `feat(mux): calculate pane rects from layout tree with tests`

#### 2f. Split operations
- `split_pane(tree: LayoutNode, target: PaneId, dir: Direction) -> Result<(LayoutNode, PaneId)>`
- `close_pane(tree: LayoutNode, target: PaneId) -> Result<Option<LayoutNode>>` (returns None if last pane)
- Tests: split then verify pane count, close then verify removal

**Commit:** `feat(mux): implement split_pane and close_pane with tests`

#### 2g. Resize operations
- `resize_pane(tree: &mut LayoutNode, target: PaneId, delta: f32) -> Result<()>` with min-size enforcement
- Tests: resize then verify ratio changed, test min-size clamping

**Commit:** `feat(mux): implement pane resize with min-size constraints and tests`

#### 2h. Pane struct
- `Pane { id: PaneId, title: String }` — thin wrapper. The actual `Term` and PTY live in the main `alacritty` crate since they depend on windowing context. The multiplexer crate tracks IDs and metadata only.

**Commit:** `feat(mux): add Pane metadata struct`

#### 2i. Window (Tab) struct
- `MuxWindow { id: WindowId, name: String, layout: LayoutNode, active_pane: PaneId, pane_order: Vec<PaneId> }`
- Methods: `new(name)`, `split(pane_id, dir)`, `close_pane(pane_id)`, `next_pane()`, `prev_pane()`, `pane_rects(total_area)`
- Tests for navigation cycling

**Commit:** `feat(mux): add MuxWindow with pane management and tests`

#### 2j. Session struct
- `Session { id: SessionId, name: String, windows: Vec<MuxWindow>, active_window: usize }`
- Methods: `new(name)`, `add_window(name)`, `close_window(idx)`, `next_window()`, `prev_window()`, `active_layout()`, `active_pane_id()`
- Tests for window cycling, edge cases

**Commit:** `feat(mux): add Session with window management and tests`

#### 2k. MuxCommand enum
- Define all commands: `SplitHorizontal`, `SplitVertical`, `ClosePane`, `NextPane`, `PrevPane`, `NavigatePane(Direction)`, `NewWindow`, `CloseWindow`, `NextWindow`, `PrevWindow`, `SwitchToWindow(u8)`, `RenameWindow(String)`, `DetachSession`, `ToggleZoom`, `ResizePane(Direction, i16)`, `ScrollbackMode`
- Also define `LeaderKeyConfig { keys: Vec<KeyCombo>, timeout_ms: u64 }` to represent the configurable leader key(s)

**Commit:** `feat(mux): define MuxCommand enum and LeaderKeyConfig for all multiplexer actions`

#### 2l. Status bar content
- `StatusBarContent { session_name, windows: Vec<(name, is_active)>, pane_info: String, time: String }`
- `fn build_status(session: &Session) -> StatusBarContent`
- Tests

**Commit:** `feat(mux): add status bar content builder with tests`

#### 2m. Session persistence
- `fn serialize_session(session: &Session) -> Result<String>` (JSON)
- `fn deserialize_session(json: &str) -> Result<Session>`
- Store at `~/.local/share/alacritty/sessions/<name>.json`
- `fn session_dir() -> PathBuf`, `fn save_session(session)`, `fn load_session(name)`, `fn list_sessions()`, `fn delete_session(name)`
- Tests with tempdir

**Commit:** `feat(mux): implement session serialization and persistence with tests`

### Phase 3: Integrate Into Alacritty Event Loop

Now we modify the existing `alacritty` crate. These changes are surgical — each commit touches one concern.

#### 3a. Add dependency
- Add `alacritty_multiplexer` to `alacritty/Cargo.toml`

**Commit:** `build: add alacritty_multiplexer dependency to alacritty crate`

#### 3b. Multi-terminal state
- Currently `alacritty` holds a single `Term` + PTY. Create a `MuxState` struct in `alacritty/src/mux_state.rs` that holds:
  - `session: Session`
  - `terminals: HashMap<PaneId, Term<EventProxy>>`
  - `ptys: HashMap<PaneId, Pty>` (the concrete PTY handles)
- Methods: `active_term(&self) -> &Term`, `active_term_mut(&mut self) -> &mut Term`, `active_pty(&self) -> &Pty`
- Keep it behind a feature flag `multiplexer` initially so the default single-terminal path still works

**Commit:** `feat: add MuxState to hold multiple terminals and PTYs`

#### 3c. PTY spawning helper
- Extract PTY creation into a reusable function that can spawn a new PTY+Term for any pane
- This function takes terminal size, shell config, and returns `(Term, Pty, PaneId)`

**Commit:** `refactor: extract PTY+Term creation into reusable spawn function`

#### 3d. Input interception — leader key state machine
- In `alacritty/src/input.rs`, add a `MuxInputState` enum: `Normal`, `WaitingForCommand`
- Support **multiple leader keys**: default is `Ctrl-Space` (primary) and `Ctrl-B` (tmux-compat alias). Both activate prefix mode simultaneously — whichever the user presses.
- On leader key in Normal → transition to `WaitingForCommand`, start a timeout timer (~1 second)
- On next key in `WaitingForCommand` → map to `MuxCommand`, transition back to `Normal`
- On timeout or unknown key → transition back to `Normal`, forward original keys to PTY
- On double-tap leader (e.g. `Ctrl-Space Ctrl-Space`) → send literal key to PTY
- The leader key(s) are configurable via `[multiplexer.leader_keys]` in config

**Commit:** `feat: add leader key state machine for multiplexer input interception`

#### 3e. Keybinding map for leader mode
- Define the mapping from second key → `MuxCommand`:
  - `"` → SplitHorizontal, `%` → SplitVertical, `-` → SplitHorizontal (alt), `|` → SplitVertical (alt)
  - `x` → ClosePane, `o` → NextPane
  - `c` → NewWindow, `n` → NextWindow, `p` → PrevWindow
  - `d` → Detach, `,` → RenameWindow, `z` → ToggleZoom
  - `0-9` → SwitchToWindow(n)
  - Arrow keys → NavigatePane(dir), Ctrl+Arrow → ResizePane(dir)
  - `[` → Enter scrollback/vi mode
  - `:` → Command prompt (stretch goal)
- Make this fully configurable via `alacritty.toml` (new `[multiplexer.keybindings]` section)
- The leader key itself is configured separately in `[multiplexer.leader_keys]`

**Commit:** `feat: add leader-mode keybinding map with config support`

#### 3f. Wire MuxCommand execution
- In the event loop, when a `MuxCommand` is received:
  - `SplitHorizontal/Vertical` → call `session.split()`, spawn new PTY+Term, update `MuxState`
  - `ClosePane` → kill PTY, remove Term, update layout
  - `NextPane/PrevPane` → update `session.active_pane`, trigger redraw
  - `NewWindow` → add window to session, spawn PTY+Term
  - `NextWindow/PrevWindow` → switch active window, trigger redraw
- Each of these should be a small function in a new `alacritty/src/mux_actions.rs`

**Commit:** `feat: wire MuxCommand execution to session and terminal state`

#### 3g. Multi-pane rendering
This is the most complex integration. Currently `Display::draw` reads from one `Term`. We need to:
- Get pane rects from `session.active_layout(total_area)`
- For each pane rect, set the OpenGL viewport/scissor to that region
- Render the corresponding `Term`'s grid into that region
- Draw borders between panes (1px lines)
- Draw the status bar at the bottom

Break this into sub-commits:

**Commit:** `refactor: extract single-term rendering into a render_term_region function`
**Commit:** `feat: render multiple panes by iterating layout rects`
**Commit:** `feat: draw pane borders between split regions`
**Commit:** `feat: render status bar at bottom of window`

#### 3h. Multi-PTY event multiplexing
- Currently the event loop reads from one PTY fd. With multiple panes, we need to `poll`/`select` across all PTY fds.
- Use `mio` or adapt the existing polling to watch all PTY read handles
- When PTY output arrives, route it to the correct `Term` by `PaneId`
- Trigger redraw only for the affected pane region (damage tracking)

**Commit:** `feat: multiplex PTY reads across all active panes`
**Commit:** `feat: route PTY output to correct Term by PaneId`

#### 3i. Terminal resize propagation
- On window resize → recalculate all pane rects → resize each PTY + Term to its new rect size
- On pane split/close → same recalculation for affected panes

**Commit:** `feat: propagate window resize to all pane PTYs and terminals`

#### 3j. Active pane focus indicator
- Highlight the border of the active pane (different color)
- Show cursor only in the active pane

**Commit:** `feat: highlight active pane border and cursor`

### Phase 4: Session Persistence (Detach/Reattach)

#### 4a. Server mode
- Add a `--server` flag: starts Alacritty in headless mode (no window), keeps PTYs alive, listens on Unix socket
- The socket path: `~/.local/share/alacritty/sockets/<session_name>.sock`

**Commit:** `feat: add --server flag for headless session mode`

#### 4b. Client-server protocol
- Define message types: `ClientMessage { Input(bytes), Resize(rows, cols), Command(MuxCommand), Attach, Detach }`
- Define: `ServerMessage { Output(pane_id, bytes), StateSync(SessionSnapshot), PaneExited(pane_id) }`
- Use length-prefixed JSON over Unix socket

**Commit:** `feat: define client-server protocol messages`

#### 4c. Server socket listener
- Server accepts client connections, sends initial `StateSync`, then streams `Output`
- On client `Input` → forward to active PTY
- On client `Command` → execute, send updated `StateSync`
- On client disconnect → server keeps running

**Commit:** `feat: implement server-side socket listener and message handling`

#### 4d. Client attach mode
- `alacritty attach <session>` → connect to existing socket, enter raw rendering mode
- Client receives `StateSync` → builds local `MuxState` for rendering
- Client receives `Output` → updates local `Term`, renders

**Commit:** `feat: implement client attach to existing session`

#### 4e. Detach
- On `<Leader> d` (Ctrl-Space d or Ctrl-B d) → client sends `Detach`, cleans up window, exits
- Server continues

**Commit:** `feat: implement detach command with clean client shutdown`

#### 4f. CLI subcommands
- `alacritty mux new [-s name]` → start server + attach
- `alacritty mux attach [-t name]` → attach to existing
- `alacritty mux list` → list active sessions (enumerate socket dir)
- `alacritty mux kill [-t name]` → send kill to server

**Commit:** `feat: add CLI subcommands for session management`

#### 4g. Session restore on reattach
- On attach, server sends full terminal grid content for each pane (not just metadata)
- Client reconstructs `Term` grids from the snapshot and renders immediately

**Commit:** `feat: send full terminal content on reattach for instant restore`

### Phase 5: Configuration

#### 5a. Config schema
- Add `[multiplexer]` section to alacritty config schema:
```toml
[multiplexer]
enabled = true
status_bar = true

# Multiple leader keys supported — any of these activate command mode.
# Default: Ctrl-Space (primary) + Ctrl-B (tmux compat).
# Users can override to use only one, or add their own.
leader_keys = ["Control-Space", "Control-b"]

# Timeout in ms before leader mode expires and keys are forwarded to PTY.
leader_timeout_ms = 1000

[multiplexer.keybindings]
# Key pressed AFTER leader key → action.
# These are the defaults (tmux-compatible).
split_horizontal = "\""
split_horizontal_alt = "-"       # Convenience alias
split_vertical = "%"
split_vertical_alt = "|"         # Convenience alias
close_pane = "x"
next_pane = "o"
prev_pane = "semicolon"
new_window = "c"
next_window = "n"
prev_window = "p"
detach = "d"
rename_window = "comma"
toggle_zoom = "z"
scrollback_mode = "bracketleft"
# Arrow keys for navigation and Ctrl+Arrow for resize are hardcoded defaults
# but can be remapped here too.

[multiplexer.status_bar]
format_left = "[{session}]"
format_center = "{windows}"
format_right = "{time}"
fg = "#a0a0a0"
bg = "#1a1a1a"
```

**Commit:** `feat: add multiplexer configuration schema`

#### 5b. Config parsing
- Wire config into `alacritty_config` crate with serde deserialization
- Defaults for all fields

**Commit:** `feat: parse multiplexer config from alacritty.toml`

#### 5c. Hot-reload
- React to config changes at runtime: update keybindings, status bar colors, etc.

**Commit:** `feat: support hot-reload of multiplexer config`

### Phase 6: Testing & Polish

#### 6a. Integration tests
- `tests/mux_integration.rs`: create session → split → verify layout → navigate → verify active pane
- Test window creation and switching
- Test persistence: save session → load → verify layout matches

**Commit:** `test: add integration tests for multiplexer lifecycle`

#### 6b. Property tests
- Layout splitting invariants: all rects tile total area
- Resize invariants: ratios stay within [0.1, 0.9]
- Navigation cycling: next/prev always lands on valid pane

**Commit:** `test: add property-based tests for layout invariants`

#### 6c. Edge case tests
- Close last pane in window → window closes
- Close last window in session → session ends
- Split when pane is too small → error returned
- Rapid split/close cycles → no state corruption
- Leader key timeout → keys forwarded to PTY after 1s
- Double-tap leader (Ctrl-Space Ctrl-Space) → literal Ctrl-Space sent to shell
- Both leader keys (Ctrl-Space and Ctrl-B) activate the same command mode
- Leader key followed by unmapped key → key discarded, return to Normal mode
- Leader key reconfigured at runtime via hot-reload → new key takes effect immediately

**Commit:** `test: add edge case tests for pane and window lifecycle`

#### 6d. Coverage sweep
- Run `cargo llvm-cov` on `alacritty_multiplexer` crate
- Add tests until >95% line coverage on the new crate
- For the integration points in `alacritty` crate, aim for test coverage of the `mux_state`, `mux_actions`, and prefix key logic

**Commit:** `test: achieve >95% coverage on alacritty_multiplexer`

#### 6e. Code quality audit
- Grep for functions >10 lines in the new code, extract helpers
- Grep for duplicated logic, extract into shared functions
- Ensure all public items have doc comments
- `cargo clippy -- -D warnings` clean
- `cargo fmt --check` clean

**Commit:** `refactor: enforce 10-line function limit and eliminate duplication`

#### 6f. Documentation
- Update `README.md` with multiplexer feature docs
- Add `docs/multiplexer.md` with: architecture, keybinding table, config reference, session management guide
- Add `ARCHITECTURE_NOTES.md` → `docs/multiplexer-architecture.md`

**Commit:** `docs: add multiplexer documentation and usage guide`

---

## Hard Constraints (Non-Negotiable)

### Commit Discipline
- **Every commit is ONE logical unit.** Adding a struct is one commit. Adding a method is one commit. Adding tests for that method is one commit. Wiring it into the event loop is one commit.
- **Every commit must compile and pass tests.** No broken intermediate states.
- **Conventional commit messages:** `feat(mux):`, `refactor:`, `test:`, `docs:`, `fix:`, `build:`
- **Run before every commit:** `cargo fmt && cargo clippy -- -D warnings && cargo test`

### Code Quality
- **No function longer than 10 lines** of logic in NEW code (we don't refactor existing Alacritty code for line length — only our additions).
- **Zero duplication** in new code. If you write similar logic twice, extract it.
- **All new public items have `///` doc comments.**
- **No `.unwrap()` in non-test code.** Use `?` or explicit error handling.
- **Use `thiserror`** for error types in `alacritty_multiplexer`.

### Minimal Upstream Disruption
- **Keep changes to existing Alacritty files minimal.** Prefer adding new files and modules over modifying existing ones.
- **Use feature flags** (`#[cfg(feature = "multiplexer")]`) for changes in the `alacritty` crate so the original single-terminal behavior is preserved when the feature is off.
- **Don't change existing data structures.** Wrap them or work alongside them.
- **Don't change existing keybindings or behavior** when multiplexer is disabled.

### Testing
- **>95% line coverage** on `alacritty_multiplexer` crate
- **Unit tests** for every non-trivial function in the new crate
- **Integration tests** for session lifecycle, pane management, persistence
- **Property tests** with `proptest` for layout invariants
- **Tests must be fast.** No sleep, no real PTYs in unit tests — mock where needed.

---

## Coding Style (Match Alacritty's Conventions)

Before writing code, run this to understand Alacritty's style:
```bash
cat rustfmt.toml
head -100 alacritty/src/event.rs
head -100 alacritty_terminal/src/term/mod.rs
```

Follow whatever conventions you find. Likely:
- Alacritty uses `log` crate (not `tracing`) — use `log` for consistency
- Alacritty uses `serde` with TOML — follow the same pattern for config
- Match the import organization style (std → external → internal)
- Match the error handling patterns already in use

---

## Key Design Decisions (Pre-Made — Don't Deliberate, Execute)

1. **New crate for multiplexer logic.** Don't pollute `alacritty_terminal` — it's a reusable library.
2. **Layout is a binary tree.** Each split produces two children. Nesting gives arbitrary layouts.
3. **PaneId is a u32 counter.** Simple, monotonic, no UUID overhead for in-process IDs.
4. **Session persistence stores layout + metadata, NOT terminal content.** On reattach to a server, content comes from the live PTYs. Persistence is for crash recovery of layout structure.
5. **Client-server split is Phase 4, not Phase 1.** First get multi-pane rendering working in-process, then factor out the server architecture.
6. **Status bar steals one row from the bottom.** Total rendering area = window height - 1.
7. **Pane borders are drawn as colored cells** (like tmux), not actual GL lines. Simpler, consistent with the cell grid.
8. **Minimum pane size: 2 rows × 5 columns.** Splits that would violate this are rejected.
9. **`Ctrl-Space` is the primary leader key, `Ctrl-B` is the secondary.** Both are active by default. `Ctrl-Space` is more ergonomic (thumb+pinky) and avoids colliding with readline's `Ctrl-B` (move cursor back). Users who prefer tmux muscle memory get `Ctrl-B` for free. The leader key list is fully configurable — users can set one, both, or something entirely different.
10. **Leader key timeout is 1 second.** If no command key follows the leader within 1s, the leader keypress is forwarded to the PTY and normal mode resumes. This prevents the terminal from appearing "stuck" if the user accidentally hits the leader.

---

## Default Key Bindings

Leader key: `Ctrl-Space` (primary) or `Ctrl-B` (tmux-compat alias). Both work out of the box.

In the table below, `<Leader>` means either `Ctrl-Space` or `Ctrl-B`.

| Keys | Action |
|---|---|
| `<Leader> "` | Split horizontal |
| `<Leader> -` | Split horizontal (alt) |
| `<Leader> %` | Split vertical |
| `<Leader> \|` | Split vertical (alt) |
| `<Leader> x` | Close pane (with confirmation) |
| `<Leader> o` | Next pane |
| `<Leader> ;` | Previous pane |
| `<Leader> ↑↓←→` | Navigate panes directionally |
| `<Leader> c` | New window |
| `<Leader> n` | Next window |
| `<Leader> p` | Previous window |
| `<Leader> 0-9` | Switch to window by number |
| `<Leader> d` | Detach session |
| `<Leader> ,` | Rename window |
| `<Leader> z` | Toggle pane zoom (fullscreen) |
| `<Leader> [` | Enter scrollback/vi mode |
| `<Leader> Ctrl-↑↓←→` | Resize pane |
| `<Leader> <Leader>` | Send literal leader key to shell |

---

## On-Disk Layout

```
~/.local/share/alacritty/
├── sessions/
│   ├── work.json          # Serialized layout + metadata
│   └── personal.json
└── sockets/
    ├── work.sock          # Unix domain socket (server mode)
    └── personal.sock
```

---

## Detach/Reattach Flow

```
[alacritty mux new -s work]
  → Forks server process (headless, owns PTYs)
  → Server creates session with default window + pane
  → Client connects via Unix socket
  → Client creates GL window, renders, forwards input

[User presses <Leader> d  (Ctrl-Space d  or  Ctrl-B d)]
  → Client sends Detach message
  → Client exits cleanly, restores terminal
  → Server continues running, all PTYs alive

[alacritty mux attach -t work]
  → Client connects to existing socket
  → Server sends StateSync (layout + full terminal grids)
  → Client reconstructs display and renders immediately
```

---

## Autonomous Operation Instructions

1. **Start by reading code, not writing it.** Spend the first phase understanding Alacritty's internals. Document what you learn.
2. **Do not ask for clarification.** Make reasonable decisions and document them in commit messages or `DECISIONS.md`.
3. **If existing Alacritty code is confusing**, read the DeepWiki docs at `https://deepwiki.com/alacritty/alacritty` for architectural context.
4. **If a dependency doesn't compile**, fix it immediately. Log the issue in the commit message.
5. **Run tests after every file change**, not just at phase boundaries.
6. **If coverage drops below 95% on new code**, stop feature work and write tests.
7. **If you discover the architecture needs adjustment**, update `ARCHITECTURE_NOTES.md` and adjust the plan.
8. **Never force-push or rewrite history.** Every commit is permanent.
9. **If you're stuck on rendering integration (Phase 3g)**, study how `Display::draw` currently works by reading every line of `alacritty/src/display/mod.rs` and `alacritty/src/renderer/`.
10. **Target ~50-80 commits** for the full project. Small, reviewable, bisectable.

Begin with Phase 1: clone the repo and study the codebase.
