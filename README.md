# earthworm

A terminal UI for managing tmux sessions organised around git projects and
worktrees.

## Features

- Organise tmux sessions under git projects
- Automatically discover existing git worktrees for a project
- Create a new git worktree and tmux session in a single step
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

| Key      | Action                     |
| -------- | -------------------------- |
| `j`      | Move down                  |
| `k`      | Move up                    |
| `a`      | Attach to selected session |
| `n`      | New session                |
| `p`      | New project                |
| `d`      | Delete selected session    |
| `q`      | Quit                       |
| `Ctrl-z` | Suspend                    |

## Concepts

**Projects** map to a git repository on disk. A project is defined by a name and
a path to the repo.

**Sessions** are tmux sessions linked to a project. When you create a session
with a worktree name, earthworm creates the git worktree and the tmux session in
one step. Sessions linked to a project are shown indented beneath it in the
list.

**Worktrees** — when you add a project, earthworm scans the repo for existing
git worktrees and lists them automatically. Selecting one and pressing `n` opens
it as a new tmux session without creating a new worktree.

## Configuration

Config and data directories are printed when you run `earthworm --version`.

Keybindings can be customised by creating a `config.json5` in the config
directory:

```json5
{
  keybindings: {
    Home: {
      "<q>": "Quit",
      "<j>": "CmdSelectNext",
      "<k>": "CmdSelectPrev",
      "<p>": "CmdManageProjects",
      "<n>": "CmdAddSession",
      "<d>": "CmdDeleteItem",
      "<a>": "CmdAttach",
    },
  },
}
```

## License

MIT
