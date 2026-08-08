#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OS="$(uname -s)"

if [[ "$OS" == "Linux" ]]; then
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
  export WEBKIT_DISABLE_COMPOSITING_MODE="${WEBKIT_DISABLE_COMPOSITING_MODE:-1}"
  export LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}"

  if [[ -z "${DISPLAY:-}" ]]; then
    for d in :0 :1 :2; do
      if DISPLAY="$d" xdpyinfo >/dev/null 2>&1; then
        export DISPLAY="$d"
        export GDK_BACKEND="${GDK_BACKEND:-x11}"
        break
      fi
    done
  fi
fi

cargo run -p qiring-desktop "$@"
