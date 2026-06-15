# earthworm

A terminal UI for managing tmux sessions organised around git projects and
worktrees.

## Features

- Organise tmux sessions under git projects
- Automatically discover existing git worktrees for a project
- Create a new git worktree and tmux session in a single step
- Edit and delete projects, sessions, and worktrees
- Search across the session list
- Help popup showing all keybindings
- State persists across restarts
- Configurable keybindings

## Prerequisites

- [tmux](https://github.com/tmux/tmux)
- [git](https://git-scm.com/)
- Rust toolchain (for building from source)

## Installation

```sh
cargo install --path .
```

Or build a release binary:

```sh
cargo build --release
./target/release/earthworm
```

## Usage

```sh
earthworm
```

### Keybindings

| Key       | Action                     |
| --------- | -------------------------- |
| `j`       | Move down                  |
| `k`       | Move up                    |
| `gg`      | Jump to top                |
| `G`       | Jump to bottom             |
| `/`       | Start search               |
| `n`       | Next search match          |
| `N`       | Previous search match      |
| `Space p` | New project                |
| `Space s` | New session                |
| `Space a` | Attach to selected session |
| `Space e` | Edit selected item         |
| `Space d` | Delete selected item       |
| `?`       | Show help                  |
| `q`       | Quit                       |
| `Ctrl-c`  | Quit                       |
| `Ctrl-z`  | Suspend                    |

## Concepts

**Projects** map to a git repository on disk. A project is defined by a name and
a path to the repo.

**Sessions** are tmux sessions linked to a project. When you create a session
with a worktree name, earthworm creates the git worktree and the tmux session in
one step. Sessions linked to a project are shown indented beneath it in the
list.

**Worktrees** — when you add a project, earthworm scans the repo for existing
git worktrees and lists them automatically.

## Configuration

Config and data directories are printed when you run `earthworm --version`.

Keybindings can be customised by creating a `config.json5` in the config
directory:

```json5
{
  keybindings: {
    Home: {
      "<q>": "Quit",
      "<Ctrl-c>": "Quit",
      "<Ctrl-z>": "Suspend",
      "<j>": "CmdSelectNext",
      "<k>": "CmdSelectPrev",
      "<g><g>": "CmdJumpTop",
      "<Shift-g>": "CmdJumpBottom",
      "</>": "CmdStartSearch",
      "<n>": "CmdSearchNext",
      "<Shift-n>": "CmdSearchPrev",
      "<Space><p>": "CmdAddProject",
      "<Space><s>": "CmdAddSession",
      "<Space><e>": "CmdEdit",
      "<Space><d>": "CmdDeleteItem",
      "<Space><a>": "CmdAttach",
      "<?>": "Help",
    },
  },
}
```

## License

MIT
