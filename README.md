# ratscope

A terminal dashboard for your machine — docker containers, systemd services, listening ports, processes and hardware — in a solid-color pixel-art TUI. Built for homelab monitoring.

## Features

- **Docker** — containers with CPU/mem/disk usage, CPU sparklines, status colors
- **systemd** — system and user units, running/all toggle, enable/disable
- **Ports** — every listening socket and the process behind it
- **PC** — global CPU with live sparkline, RAM/swap/disk/docker stacked bar, top processes by memory (clickable, killable), per-mount disk usage
- **Per-target detail screen** (`v`) — live info + full action menu: logs (live), restart, stop, start, pause/unpause, reload, kill, kill -9, enable/disable, inspect/status
- **Theme terminal** (`:`) — change colors, borders and gauges live, persist to `~/.config/ratscope/config.json`
- Truecolor palette, works with any terminal theme

## Install

```sh
cargo install ratscope
```

or grab a static binary from [releases](https://github.com/schnnjuan/ratscope/releases).

## Keys

| Key | Action |
|-----|--------|
| `Tab` | cycle docker / systemd / ports / pc |
| `↑↓` | select row |
| `Enter` | action menu |
| `v` | detail screen |
| `:` | theme terminal |
| `a` | systemd: show all units |
| `d` | docker disk usage |
| `x` | docker contexts |
| `r` | refresh now |
| `q` | quit |

Requires Linux, a truecolor terminal, and Rust 1.85+ (edition 2024) if building from source.

## License

MIT
