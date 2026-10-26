#!/bin/sh
# ratscope installer
#   curl -fsSL https://github.com/schnnjuan/ratscope/releases/latest/download/install.sh | sh
# flags: RATSCOPE_LANG=en|pt · RATSCOPE_PREFIX=dir
set -eu

REPO="schnnjuan/ratscope"
PREFIX="${RATSCOPE_PREFIX:-$HOME/.local/bin}"
URL="https://github.com/$REPO/releases/latest/download"

# ---- colors ----
E=$(printf '\033')
RESET="$E[0m"
MINT="$E[38;2;198;230;224m"
TEAL="$E[38;2;83;143;135m"
GRAY="$E[38;2;110;120;118m"
RED="$E[38;2;220;90;90m"
HIDE="$E[?25l"
SHOW="$E[?25h"

die() { printf '%s\n\n  %s%s%s\n\n' "$SHOW" "$RED" "$1" "$RESET" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || die "curl missing — install it and try again"
command -v tar  >/dev/null 2>&1 || die "tar missing — install it and try again"
command -v awk  >/dev/null 2>&1 || die "awk missing — install it and try again"

# ---- tty: stdin is the pipe under curl|sh, so read choices from /dev/tty ----
HAS_TTY=0
OLD_STTY=""
# [ -t 2 ]: stderr still points at the user's terminal under curl|sh
if [ "${RATSCOPE_NO_TTY:-}" != 1 ] && [ -t 2 ] && [ -r /dev/tty ] && (exec 3</dev/tty) 2>/dev/null; then
    exec 3</dev/tty
    HAS_TTY=1
    OLD_STTY=$(stty -g </dev/tty 2>/dev/null || true)
fi
cleanup() {
    if [ "$HAS_TTY" = 1 ] && [ -n "$OLD_STTY" ]; then
        stty "$OLD_STTY" </dev/tty 2>/dev/null || true
    fi
    printf '%s' "$SHOW"
}
trap cleanup EXIT

# ---- header ----
printf '%s' "$HIDE"
printf '\n'
# logo: real PNG via kitty/ghostty/wezterm/iterm2 image protocols; ASCII fallback
LOGO_B64=$(cat <<'EOB' | tr -d '\n'
iVBORw0KGgoAAAANSUhEUgAAApQAAACoCAYAAABe6H7DAAAABmJLR0QA/wD/AP+gvaeTAAAE50lE
QVR4nO3dsWpUQRTHYVf2jbTRygfQKiCKTQothIAINkKsFG0UsbIxYCOKkEorKytt9FkMJAa0WTux
Cgz/O94zud9XRz337MT9cZs5cwYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAkazmHgDm9Ob7103Pv//GuQt+xwA49c7OPQAAAGMTlAAA
RAQlAAARQQkAQERQAgAQEZQAAEQEJQAAEUEJAEBEUAIAEBGUAABEBCUAAJH13APAnA6ODuceAQCG
5w0lAAARQQkAQERQAgAQEZQAAEQEJQAAEUEJAEBEUAIAEBGUAABEBCUAABFBCQBARFACABBZtf6B
Rx/3Nz0G+V92L281P3NP9nmy0ffTm/1Pyz6nZZ8ns59p+X6fVus+vaEEACAiKAEAiAhKAAAighIA
gIigBAAgIigBAIgISgAAIoISAICIoAQAICIoAQCICEoAACKre+9el7pr8um17VJ3cS5N7/NQ7fP1
vNOq9ry92ee0Rt9ntfmrzdOq2vzV5mnVe35vKAEAiAhKAAAighIAgIigBAAgIigBAIgISgAAIoIS
AICIoAQAICIoAQCICEoAACKCEgCAyPrH4VHXf2Dv1s6i7qIdXe/zUI3nJWGf0xp9n9XmrzZPq2rz
V5unVe/5vaEEACAiKAEAiAhKAAAighIAgIigBAAgIigBAIgISgAAIoISAICIoAQAICIoAQCICEoA
ACLrg6PDuWdYtK3nTzZzz/Cv1vOwf/f+0He1L+38793aafq8Ws9ntfPc+3wu7fz0Nvo+q81fbZ5W
1ebvPU/v/z97z+8NJQAAEUEJAEBEUAIAEBGUAABEBCUAABFBCQBARFACABARlAAARAQlAAARQQkA
QERQAgAQWf/+eTz3DKfKlUcPmu7ibN3/h92HXe8mbp2/9ed7z9/K+T9Ztbvaq51P52dao++z2vy9
52n9/WpV7fux9z6rfT+28oYSAICIoAQAICIoAQCICEoAACKCEgCAiKAEACAiKAEAiAhKAAAighIA
gIigBAAgIigBAIisfx3Xunt0dKPvc/T5Wy3teUdX7fOqNs/oRt9ntfl7z/Pp8bOh755uVe3zrcYb
SgAAIoISAICIoAQAICIoAQCICEoAACKCEgCAiKAEACAiKAEAiAhKAAAighIAgIigBAAgsv79092U
Uxp9n6PP36r38166c3vT8vOfX7xc1N24raqdz2rzjG70fVabv9o8oxt9n63fR628oQQAICIoAQCI
CEoAACKCEgCAiKAEACAiKAEAiAhKAAAighIAgIigBAAgIigBAIgISgAAIquLN7e73u345dVrdxOf
oPf+exv98622/977rPa8vVU7n/Y/rdH3Ofp+qv1+VbO08+kNJQAAEUEJAEBEUAIAEBGUAABEBCUA
ABFBCQBARFACABARlAAARAQlAAARQQkAQERQAgAQcQ8nAJxC569f7XqX9Le37zUEf3lDCQBARFAC
ABARlAAARAQlAAARQQkAQERQAgAQEZQAAEQEJQAAEUEJAEBEUAIAEBGUAABE1nMPAABM79fx8dwj
sCDeUAIAEBGUAABEBCUAABFBCQBARFACABARlAAARAQlAAARQQkAQERQAgAQEZQAAEQEJQAAkT/d
PzixGoW1swAAAABJRU5ErkJggg==
EOB
)
IMG=ascii
case "${TERM_PROGRAM:-}" in WezTerm*|iTerm.app) IMG=iterm ;; esac
if [ "$IMG" = ascii ]; then
    case "${TERM:-}" in *kitty*|*ghostty*) IMG=kitty ;; esac
fi
if [ "$IMG" = ascii ]; then
    [ -n "${KITTY_WINDOW_ID:-}" ] && IMG=kitty
    [ -n "${GHOSTTY_RESOURCES_DIR:-}" ] && IMG=kitty
fi
[ -n "${TMUX:-}" ] && IMG=ascii
if [ "$IMG" = kitty ]; then
    printf '%s' "$LOGO_B64" | fold -w 4000 | awk -v E="$E" '
        { c[NR]=$0 }
        END {
            for (i=1;i<=NR;i++) {
                if (i==1) printf "%s_Gf=100,a=T,q=1,c=55,m=%d;%s%s\\", E, (i==NR?0:1), c[i], E
                else      printf "%s_Gm=%d;%s%s\\", E, (i==NR?0:1), c[i], E
            }
            printf "\n"
        }'
elif [ "$IMG" = iterm ]; then
    printf '%s]1337;File=inline=1;width=55;size=%d:%s\a\n' "$E" $(( ${#LOGO_B64} * 3 / 4 )) "$LOGO_B64"
else
    # ASCII fallback: dedicated 7-row pixel font, svg gradient, full blocks only
    python3 - "$E" <<'PYEOF' 2>/dev/null || true
import sys
E = sys.argv[1] if len(sys.argv) > 1 else "\033"
G = [
 ("r",1,[".###.","##...","##...","##...","##..."]),
 ("a",1,[".###.","...##",".####","#..##",".###."]),
 ("t",0,["..#..","..#..","#####","..#..","..#..","..#.."]),
 ("s",1,[".####","#....","###..","...#.","####."]),
 ("c",1,[".####","#....","#....","#....",".####"]),
 ("o",1,[".###.","#...#","#...#","#...#",".###."]),
 ("p",1,["####.","#..##","#..##","####.","#....","#...."]),
 ("e",1,[".###.","#...#","####.","#....",".###."]),
]
W,H=47,7
grid=[[0]*W for _ in range(H)]
x=0
for _,off,rows in G:
    for ry,row in enumerate(rows):
        for cx,ch in enumerate(row):
            if ch=="#": grid[off+ry][x+cx]=1
    x+=6
# same 12-color gradient as docs/logo.svg, sampled over 7 rows. Full blocks
# only — ▀/▄ render as full blocks on some terminals (blob).
GRAD=[(165,205,199),(149,193,186),(132,180,173),(116,168,160),(99,155,148),(83,143,135),
      (77,132,125),(70,122,115),(64,111,105),(58,100,96),(52,89,86),(45,79,76)]
for y in range(H):
    c=GRAD[min(y*11//(H-1),11)]
    line="".join((f"{E}[38;2;{c[0]};{c[1]};{c[2]}m█" if p else " ") for p in grid[y])
    print(f"{E}[39;49m"+line+f"{E}[39;49m")
PYEOF
fi

printf '\n'
printf '  %spixel-art dashboard for docker, systemd, ports and hardware%s\n\n' "$GRAY" "$RESET"

# ---- language ----
LANG_CHOICE="${RATSCOPE_LANG:-}"
if [ -z "$LANG_CHOICE" ] && [ "$HAS_TTY" = 1 ]; then
    printf '  %slanguage / idioma%s\n\n' "$TEAL" "$RESET"
    # -icanon -echo: bytes arrive unbuffered, no echo, Ctrl-C still works
    stty -icanon -echo </dev/tty 2>/dev/null || true
    sel=1
    ESC=$(printf '\033')
    CR=$(printf '\r')
    draw_lang() {
        printf '%s' "${E}[2A${E}[J"
        if [ "$sel" = 1 ]; then
            printf '  %s❯ english%s\n  %s  português%s\n' "$MINT" "$RESET" "$GRAY" "$RESET"
        else
            printf '  %s  english%s\n  %s❯ português%s\n' "$GRAY" "$RESET" "$MINT" "$RESET"
        fi
    }
    printf '  \n\n'
    draw_lang
    while :; do
        k=$(dd bs=1 count=1 <&3 2>/dev/null || true)
        if [ "$k" = "$ESC" ]; then
            rest=$(dd bs=1 count=2 <&3 2>/dev/null || true)
            case "$rest" in
                '[A'|'OA') sel=1 ;;
                '[B'|'OB') sel=2 ;;
                *) break ;; # esc alone: accept current
            esac
            draw_lang
        elif [ "$k" = "$CR" ] || [ -z "$k" ]; then
            break
        elif [ "$k" = "k" ] || [ "$k" = "1" ]; then
            sel=1; draw_lang
        elif [ "$k" = "j" ] || [ "$k" = "2" ]; then
            sel=2; draw_lang
        fi
    done
    stty "$OLD_STTY" </dev/tty 2>/dev/null || true
    [ "$sel" = 1 ] && LANG_CHOICE=en || LANG_CHOICE=pt
    printf '\n'
elif [ -z "$LANG_CHOICE" ]; then
    LANG_CHOICE=en
    printf '  %slanguage / idioma%s  english  %s(RATSCOPE_LANG=pt for português)%s\n\n' "$TEAL" "$RESET" "$GRAY" "$RESET"
else
    printf '  %slanguage / idioma%s  %s\n\n' "$TEAL" "$RESET" "$LANG_CHOICE"
fi

# ---- copy ----
if [ "$LANG_CHOICE" = pt ]; then
    M_DL="baixando ratscope"
    M_INST="instalando em"
    M_DONE="✔ instalado em"
    M_RUN="rode:"
    M_PATH="$PREFIX não está no PATH — adiciona ao teu shell profile"
    M_ARCH="arquitetura não suportada: %s — abre uma issue em github.com/$REPO"
    M_CFG="config já existe sem 'lang' — roda 'lang pt' no terminal do tema (:) do ratscope pra salvar"
else
    M_DL="downloading ratscope"
    M_INST="installing to"
    M_DONE="✔ installed"
    M_RUN="run:"
    M_PATH="$PREFIX is not in your PATH — add it to your shell profile"
    M_ARCH="unsupported arch: %s — open an issue at github.com/$REPO"
    M_CFG="config exists without 'lang' — run 'lang $LANG_CHOICE' in ratscope's theme terminal (:) to persist it"
fi

# ---- arch ----
case "$(uname -m)" in
    x86_64)        TARGET=x86_64-unknown-linux-musl ;;
    aarch64|arm64) TARGET=aarch64-unknown-linux-musl ;;
    *) die "$(printf "$M_ARCH" "$(uname -m)")" ;;
esac

mb() { awk -v n="$1" 'BEGIN{printf "%.1f", n/1048576}'; }

# ---- download with a real progress bar ----
SRC="$URL/ratscope-$TARGET.tar.gz"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"; cleanup' EXIT
printf '  %s%s %s%s\n' "$GRAY" "$M_DL" "$TARGET" "$RESET"
TOTAL=$(curl -sIL "$SRC" | tr -d '\r' | awk 'tolower($1)=="content-length:"{v=$2} END{print v+0}')
curl -sL -o "$TMP/dl" "$SRC" &
PID=$!
BAR_W=20
while kill -0 "$PID" 2>/dev/null; do
    CUR=$(wc -c 2>/dev/null < "$TMP/dl" || echo 0)
    if [ "${TOTAL:-0}" -gt 0 ] 2>/dev/null; then
        FILL=$((CUR * BAR_W / TOTAL))
        PCT=$((CUR * 100 / TOTAL))
        BAR=$(awk -v n="$FILL" 'BEGIN{for(i=0;i<n;i++)printf "█"}')
        printf '\r  %s%s%s%*s  %3d%%  %s/%s MB  ' "$MINT" "$BAR" "$RESET" "$((BAR_W - FILL))" "" "$PCT" "$(mb "$CUR")" "$(mb "$TOTAL")"
    else
        case $((i % 4)) in
            0) S=▘ ;; 1) S=▝ ;; 2) S=▗ ;; 3) S=▖ ;;
        esac
        printf '\r  %s%s%s  %s MB  ' "$TEAL" "$S" "$RESET" "$(mb "$CUR")"
    fi
    sleep 0.1
done
wait "$PID" || die "download failed — check your connection and try again"
CUR=$(wc -c < "$TMP/dl")
if [ "${TOTAL:-0}" -gt 0 ] 2>/dev/null; then
    BAR=$(awk -v n="$BAR_W" 'BEGIN{for(i=0;i<n;i++)printf "█"}')
    printf '\r  %s%s%s  100%%  %s/%s MB\n\n' "$MINT" "$BAR" "$RESET" "$(mb "$CUR")" "$(mb "$TOTAL")"
else
    printf '\r  \033[K\n\n'
fi

# ---- install with spinner ----
printf '  %s%s %s%s\n' "$GRAY" "$M_INST" "$PREFIX" "$RESET"
(
    mkdir -p "$PREFIX"
    tar xz -C "$TMP" -f "$TMP/dl"
    install -m 755 "$TMP/ratscope" "$PREFIX/ratscope"
) &
PID=$!
i=0
while kill -0 "$PID" 2>/dev/null; do
    case $((i % 4)) in
        0) S=▘ ;; 1) S=▝ ;; 2) S=▗ ;; 3) S=▖ ;;
    esac
    printf '\r  %s%s%s  ' "$TEAL" "$S" "$RESET"
    i=$((i + 1))
    sleep 0.08
done
if ! wait "$PID"; then
    die "install failed"
fi
printf '\r  \033[K\n\n'

# ---- persist language ----
CFG="$HOME/.config/ratscope"
mkdir -p "$CFG"
if [ -f "$CFG/config.json" ]; then
    grep -q '"lang"' "$CFG/config.json" || printf '  %s%s%s\n' "$GRAY" "$M_CFG" "$RESET"
else
    printf '{\n  "lang": "%s"\n}\n' "$LANG_CHOICE" > "$CFG/config.json"
fi

# ---- done ----
printf '  %s%s %s%s\n' "$MINT" "$M_DONE" "$PREFIX/ratscope" "$RESET"
printf '  %s%s%s  ratscope\n' "$GRAY" "$M_RUN" "$RESET"
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) printf '\n  %s%s%s\n' "$GRAY" "$M_PATH" "$RESET" ;;
esac
printf '\n'
