<div align="center">

<img src="docs/logo.svg" alt="ratscope" width="440"/>

**pixel-art terminal dashboard for your homelab**

docker · systemd · listening ports · pc · hardware — one solid-color TUI

[![release](https://img.shields.io/github/v/release/schnnjuan/ratscope?style=flat-square&color=538F87&v=0.11.2)](https://github.com/schnnjuan/ratscope/releases)
[![crates.io](https://img.shields.io/crates/v/ratscope?style=flat-square&color=538F87&v=0.11.2)](https://crates.io/crates/ratscope)
[![license](https://img.shields.io/badge/license-MIT-538F87?style=flat-square)](LICENSE)

</div>

## features

- **docker** — containers with cpu/mem/disk, cpu sparklines, status colors, prune
- **systemd** — system and user units, running/all toggle, enable/disable
- **ports** — every listening socket and the process behind it
- **pc** — live cpu sparkline, ram/swap/disk/docker stacked bar, top processes (clickable, killable), per-mount disk usage
- **hardware info** (`H`) — kernel, uptime, load, cpu, ram, swap, disks, raid, docker total; fastfetch/neofetch if available
- **detail screen** (`v`) — per-target view with a full action menu: live logs, restart, stop, start, pause/unpause, reload, kill, `kill -9`, enable/disable, inspect
- **theme terminal** (`:`) — change colors, borders and gauges live; persist to `~/.config/ratscope/config.json`; includes **carmesim** preset
- **english & português** — english by default, switch with `lang pt`
- truecolor, zero images, zero assets — everything is terminal characters

## install

**cargo**

```sh
cargo install ratscope
```

**curl (any linux, static binary)**

```sh
curl -fsSL https://ratscope.schnnjuan.com/install.sh | sh
```

português por padrão? instale com:

```sh
curl -fsSL https://ratscope.schnnjuan.com/install.sh | RATSCOPE_LANG=pt sh
```

**from source**

```sh
git clone https://github.com/schnnjuan/ratscope
cd ratscope && cargo install --path .
```

requires linux and a truecolor terminal.

## keys

| key | action |
|-----|--------|
| `Tab` | cycle docker / systemd / ports / hardware / pc |
| `↑↓` | select row |
| `Enter` | action menu |
| `v` | detail screen |
| `:` | theme terminal |
| `a` | systemd: show all units |
| `d` | docker disk usage |
| `x` | docker contexts |
| `r` | refresh now |
| `H` | hardware info popup |
| `q` | quit |

## language

english is the default. to switch to português:

- type `lang pt` in the theme terminal (`:`), then `save`, or
- set `"lang": "pt"` in `~/.config/ratscope/config.json`, or
- choose it during the curl install.

## license

MIT
