#!/bin/sh
# ratscope installer — usage:
#   curl -fsSL https://raw.githubusercontent.com/schnnjuan/ratscope/master/install.sh | sh
#   RATSCOPE_LANG=pt sh install.sh   (skip the language prompt)
set -eu

REPO="schnnjuan/ratscope"
PREFIX="${RATSCOPE_PREFIX:-$HOME/.local/bin}"

# ---- language (en default) ----
LANG_CHOICE="${RATSCOPE_LANG:-}"
if [ -z "$LANG_CHOICE" ]; then
    printf 'Language / Idioma?\n  1) English (default)\n  2) Português\n> '
    read -r ans
    case "$ans" in
        2|pt|PT|portugues|Português) LANG_CHOICE=pt ;;
        *) LANG_CHOICE=en ;;
    esac
fi

# ---- detect arch ----
case "$(uname -m)" in
    x86_64)  TARGET=x86_64-unknown-linux-musl ;;
    aarch64|arm64) TARGET=aarch64-unknown-linux-musl ;;
    *) echo "unsupported arch: $(uname -m)" >&2; exit 1 ;;
esac

# ---- download latest release ----
URL="https://github.com/$REPO/releases/latest/download/ratscope-$TARGET.tar.gz"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
echo "downloading $URL ..."
curl -fsSL "$URL" | tar xz -C "$TMP"

mkdir -p "$PREFIX"
install -m 755 "$TMP/ratscope" "$PREFIX/ratscope"

# ---- persist language choice ----
CFG="$HOME/.config/ratscope"
mkdir -p "$CFG"
if [ -f "$CFG/config.json" ]; then
    grep -q '"lang"' "$CFG/config.json" || \
        echo "  note: config exists without 'lang' — run 'lang $LANG_CHOICE' in ratscope's theme terminal (:) to persist it"
else
    printf '{\n  "lang": "%s"\n}\n' "$LANG_CHOICE" > "$CFG/config.json"
fi

echo
echo "✔ ratscope installed at $PREFIX/ratscope (language: $LANG_CHOICE)"
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) echo "  note: $PREFIX is not in your PATH — add it to your shell profile" ;;
esac
echo "run it:  ratscope"
