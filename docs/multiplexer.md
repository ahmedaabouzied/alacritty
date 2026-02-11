# Alacritty Multiplexer

Built-in terminal multiplexer for Alacritty providing pane splitting, window/tab
management, session persistence, and a configurable status bar — all without
external tools like tmux or screen.

## Architecture

The multiplexer is implemented as a separate crate (`alacritty_multiplexer`)
that is optionally linked into the main `alacritty` binary via the
`multiplexer` feature flag.

### Crate Structure

```
alacritty_multiplexer/
  src/
    lib.rs           Module declarations
    cli.rs           CLI subcommand types (new, attach, list, kill)
    command.rs       MuxCommand enum (all multiplexer actions)
    config.rs        Configuration schema + SerdeReplace for hot-reload
    error.rs         MuxError type with thiserror
    layout.rs        Binary split tree (LayoutNode) and PaneId
    pane.rs          Pane metadata (id + title)
    persistence.rs   Session save/load to ~/.local/share/alacritty/sessions/
    protocol.rs      Client-server protocol (ClientMessage / ServerMessage)
    rect.rs          Rectangle math for pane regions
    resize.rs        Resize operations with min/max ratio constraints
    server.rs        Server-side session management and command dispatch
    session.rs       Session (owns windows, tracks active window)
    socket.rs        Unix domain socket I/O helpers and message framing
    split.rs         Split/close operations on the layout tree
    statusbar.rs     Status bar content generation
    window.rs        MuxWindow (tab: owns layout + panes)
```

### Data Model

```
Session
  ├── id: SessionId
  ├── name: String
  ├── windows: Vec<MuxWindow>
  └── active_window: usize

MuxWindow
  ├── id: WindowId
  ├── name: String
  ├── layout: LayoutNode (binary tree)
  ├── active_pane: PaneId
  ├── panes: HashMap<PaneId, Pane>
  └── zoomed: bool

LayoutNode
  ├── Leaf { pane_id }
  └── Split { direction, ratio, first, second }
```

### Integration with Alacritty (feature = "multiplexer")

When the `multiplexer` feature is enabled, seven modules are added to the
`alacritty` binary crate:

| Module | Role |
|--------|------|
| `mux_state.rs` | Holds `Session` + per-pane `Term<EventProxy>` + PTY handles |
| `mux_spawn.rs` | Creates PTY + Term for new panes |
| `mux_input.rs` | Leader key state machine intercepting keyboard events |
| `mux_actions.rs` | Dispatches `MuxCommand`, config hot-reload, resize propagation |
| `mux_render.rs` | Computes pane pixel regions, borders, status bar, and colors |
| `mux_server.rs` | Server-side socket listener for detach/reattach |
| `mux_client.rs` | Client-side connection for attaching to sessions |

## Key Bindings

Leader key: **Ctrl-Space** (primary) or **Ctrl-B** (tmux-compatible alias).
Both work simultaneously by default.

Press the leader key, then one of the following keys:

| Key | Action |
|-----|--------|
| `"` | Split horizontal (top/bottom) |
| `-` | Split horizontal (alt) |
| `%` | Split vertical (left/right) |
| `\|` | Split vertical (alt) |
| `x` | Close pane |
| `o` | Next pane |
| `;` | Previous pane |
| `c` | New window |
| `n` | Next window |
| `p` | Previous window |
| `0`-`9` | Switch to window by number |
| `d` | Detach session |
| `,` | Rename window |
| `z` | Toggle pane zoom (full-screen) |
| `[` | Enter scrollback/vi mode |
| Leader | Send literal leader key to shell |

Leader mode times out after 1 second. If no command key is pressed, the leader
keypress is discarded and normal input resumes.

## Configuration

Add a `[multiplexer]` section to `alacritty.toml`:

```toml
[multiplexer]
enabled = true
status_bar = true

# Leader keys that activate command mode (any of these).
leader_keys = ["Control-Space", "Control-b"]

# Timeout in ms before leader mode expires.
leader_timeout_ms = 1000

[multiplexer.keybindings]
split_horizontal = "\""
split_horizontal_alt = "-"
split_vertical = "%"
split_vertical_alt = "|"
close_pane = "x"
next_pane = "o"
prev_pane = ";"
new_window = "c"
next_window = "n"
prev_window = "p"
detach = "d"
rename_window = ","
toggle_zoom = "z"
scrollback_mode = "["

[multiplexer.status_bar]
format_left = "[{session}]"
format_center = "{windows}"
format_right = "{time}"
fg = "#a0a0a0"
bg = "#1a1a1a"
```

All fields are optional and have sensible defaults.

## Session Management

### On-Disk Layout

```
~/.local/share/alacritty/
  sessions/
    work.json          Serialized session layout + metadata
    personal.json
  sockets/
    work.sock          Unix domain socket (server mode)
    personal.sock
```

### CLI Subcommands

```
alacritty mux new [-s name]      Start a new session
alacritty mux attach [-t name]   Attach to an existing session
alacritty mux list               List active sessions
alacritty mux kill [-t name]     Kill a session
```

### Detach/Reattach Flow

1. `alacritty mux new -s work` — forks a server process (headless, owns PTYs),
   client connects via Unix socket and renders.
2. Press `<Leader> d` — client sends Detach, exits cleanly; server keeps
   running with all PTYs alive.
3. `alacritty mux attach -t work` — new client connects, receives full state
   sync, and renders immediately.

### Client-Server Protocol

Messages are exchanged as length-prefixed JSON over Unix domain sockets:

**Client → Server:**
- `Input(bytes)` — raw terminal input
- `Resize { rows, cols }` — terminal resize
- `Command(MuxCommand)` — multiplexer command
- `Attach` — request to attach
- `Detach` — request to detach

**Server → Client:**
- `Output { pane_id, data }` — terminal output from a pane
- `StateSync(Session)` — full session state on attach
- `PaneExited(pane_id)` — pane process exited
- `ServerShutdown` — server is shutting down

## Design Decisions

1. **Separate crate** — multiplexer logic is in `alacritty_multiplexer`,
   independent of windowing and rendering, enabling isolated testing.
2. **Binary layout tree** — each split produces two children; nesting gives
   arbitrary layouts.
3. **PaneId is a u32 counter** — simple, monotonic, no UUID overhead.
4. **Feature flag** — `#[cfg(feature = "multiplexer")]` keeps the default
   single-terminal path unchanged.
5. **Status bar steals one row** — rendering area = window height - 1.
6. **Minimum pane size: 2 rows x 5 columns** — splits violating this are
   rejected.
7. **Leader key timeout: 1 second** — prevents the terminal from appearing
   stuck if the leader key is pressed accidentally.
