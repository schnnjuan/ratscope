<div align="center">

<img src="docs/logo.svg" alt="ratscope" width="550"/>

**A pixel-art terminal dashboard for your homelab.**

Docker · systemd · listening ports · CPU · RAM · disk — all in one solid-color TUI.

[![release](https://img.shields.io/github/v/release/schnnjuan/ratscope?style=flat-square&color=538F87)](https://github.com/schnnjuan/ratscope/releases)
[![license](https://img.shields.io/crates/l/ratscope?style=flat-square&color=538F87)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/ratscope?style=flat-square&color=538F87)](https://crates.io/crates/ratscope)

</div>

---

## Why

Four ssh sessions and a `htop` open just to check if the homelab is alive? ratscope puts everything in one terminal window, renders like a retro console, and stays out of your way.

- **Docker** — containers with CPU/mem/disk, CPU sparklines, status colors, prune
- **systemd** — system and user units, running/all toggle, enable/disable
- **Ports** — every listening socket and the process behind it
- **PC** — live CPU sparkline, RAM/swap/disk/docker stacked bar, top processes (clickable, killable), per-mount disk usage
- **Detail screen** (`v`) — per-target view with a full action menu: live logs, restart, stop, start, pause/unpause, reload, kill, `kill -9`, enable/disable, inspect
- **Theme terminal** (`:`) — change colors, borders and gauges live; persist to `~/.config/ratscope/config.json`
- **English & Portuguese** — English by default, switch with `lang pt`
- Truecolor, zero images, zero assets — everything is terminal characters

## Install

**cargo**

```sh
cargo install ratscope
```

**curl (any Linux, static binary)**

```sh
curl -fsSL https://github.com/schnnjuan/ratscope/releases/latest/download/install.sh | sh
```

Português por padrão? Instale com:

```sh
curl -fsSL https://github.com/schnnjuan/ratscope/releases/latest/download/install.sh | RATSCOPE_LANG=pt sh
```

**from source**

```sh
git clone https://github.com/schnnjuan/ratscope
cd ratscope && cargo install --path .
```

Requires Linux and a truecolor terminal.

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

## Language

English is the default. To switch to Portuguese, either:

- type `lang pt` in the theme terminal (`:`), then `save`, or
- set `"lang": "pt"` in `~/.config/ratscope/config.json`, or
- choose it during the curl install.

## License

MIT
