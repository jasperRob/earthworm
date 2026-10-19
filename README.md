# earthworm

[![CI](https://github.com/jasperRob/earthworm/actions/workflows/ci.yml/badge.svg)](https://github.com/jasperRob/earthworm/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Working on several branches of the same repo at once? earthworm gives each git
worktree its own tmux session, and lets you create, find and jump between them
from a single list in your terminal.

- [Quick start](#quick-start)
- [How it works](#how-it-works)
- [Keybindings](#keybindings)
- [Configuration](#configuration)
- [Development](#development)

## Why earthworm?

- Start work on a branch with one keypress, which creates both the worktree and
  its tmux session
- See every repo, its sessions, and its worktrees without a session, in one tree
- Clean up a branch by removing its session and worktree from the same list

## Installation

earthworm runs on **macOS and Linux**, and needs:

- [tmux](https://github.com/tmux/tmux)
- [git](https://git-scm.com/)
- A [Rust toolchain](https://rustup.rs/), to build it

Install it with Cargo:

```sh
cargo install --git https://github.com/jasperRob/earthworm
```

Or from a clone:

```sh
git clone https://github.com/jasperRob/earthworm
cd earthworm
cargo install --path .
```

## Quick start

1. Run `earthworm`.
2. Press `Space p` to add a project, giving it a name and the path to a git
   repo. Its existing worktrees are listed under it.
3. With the project selected, press `Space c` and enter a branch name. earthworm
   creates a worktree for the branch and a tmux session in it.
4. Press `Enter` on the session to attach to it.
5. Press `Tab` or `Ctrl-o` to move forwards and back through the sessions you've
   visited.

Press `?` at any time to see every keybinding.

## How it works

The list is a tree of projects, each holding its sessions followed by any
worktrees that don't have a session yet:

```
 ▾ api (3)               project: a git repo on disk
   ├ ● main              session in the repo itself, always listed first
   ├ ● fix-login         session in a worktree
   └ ○ feat/search       worktree without a session
 ▸ web (2)               collapsed project
 ● scratch               tmux session not tied to a project
```

**Projects** are git repositories, defined by a name and a path.

**Sessions** are tmux sessions. A project's sessions are named
`<project>_<session>` in tmux, so they won't clash with your other sessions.
tmux sessions that earthworm didn't create are listed at the bottom, and
sessions that no longer exist in tmux are removed from the list.

**Worktrees** are read from `git worktree list` for each project, so ones you
create outside earthworm show up too. Select one and press `Space s` to open a
session in it.

### What earthworm does to your repo

When you create a session in a project with `Space c`:

- The worktree is created **beside** the repo as `<repo>-<name>`, with any `/`
  in the name replaced by `-`. For a repo at `~/src/api`, `feat/search` becomes
  `~/src/api-feat-search`.
- The branch is taken from `origin` if it exists there, then from a local
  branch, and is otherwise created from your current `HEAD`. An existing local
  branch that is behind `origin` is fast-forwarded.

Deleting a **session** only kills the tmux session, and its worktree is kept.
Deleting a **worktree** runs `git worktree remove`, which refuses if the
worktree has changes or untracked files.

## Keybindings

**Navigation**

| Key        | Action                                        |
| ---------- | --------------------------------------------- |
| `j` / `k`  | Move down / up                                |
| `gg` / `G` | Jump to top / bottom                          |
| `Enter`    | Attach to session, or collapse/expand project |
| `h` / `l`  | Collapse / expand project                     |
| `za`       | Toggle project collapsed                      |

**Projects and sessions**

| Key       | Action                                             |
| --------- | -------------------------------------------------- |
| `Space p` | New project                                        |
| `Space c` | New session and worktree in the selected project   |
| `Space s` | New session, or a session in the selected worktree |
| `Space a` | Attach to selected session                         |
| `Space e` | Edit selected item                                 |
| `Space d` | Delete selected item                               |

**Search**

| Key       | Action                             |
| --------- | ---------------------------------- |
| `/`       | Start search                       |
| `Enter`   | Keep the search and return to list |
| `n` / `N` | Next / previous match              |
| `Esc`     | Clear search                       |

**History and app**

| Key              | Action                      |
| ---------------- | --------------------------- |
| `Tab` / `Ctrl-i` | Next session in history     |
| `Ctrl-o`         | Previous session in history |
| `?`              | Show help                   |
| `q` / `Ctrl-c`   | Quit                        |
| `Ctrl-z`         | Suspend                     |

## Configuration

Create a `config.json5` in the config directory to change keybindings. You only
need to list the bindings you want to change, as they are merged over the
defaults:

```json5
{
  keybindings: {
    Home: {
      "<Ctrl-n>": "CmdAddSessionInProject",
    },
  },
}
```

See [`.config/config.json5`](.config/config.json5) for the defaults and the
names of every action. `config.json`, `config.yaml`, `config.toml` and
`config.ini` are also read.

| OS    | Config directory                                       | Data directory                                          |
| ----- | ------------------------------------------------------ | ------------------------------------------------------- |
| macOS | `~/Library/Application Support/com.kdheepak.earthworm` | `~/Library/Application Support/com.kdheepak.earthworm`  |
| Linux | `$XDG_CONFIG_HOME/earthworm` (`~/.config/earthworm`)   | `$XDG_DATA_HOME/earthworm` (`~/.local/share/earthworm`) |

Set `EARTHWORM_CONFIG` or `EARTHWORM_DATA` to use other directories.
`earthworm --version` prints the directories in use.

## Development

```sh
cargo test     # run the tests
./clippy       # lint, as CI does
cargo fmt      # format
```

## License

[MIT](LICENSE)
