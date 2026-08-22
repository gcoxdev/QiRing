#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OS="$(uname -s)"
WINDOW_BACKEND="${QIRING_WINDOW_BACKEND:-auto}"
CARGO_ARGS=()

for argument in "$@"; do
  case "$argument" in
    --x11)
      WINDOW_BACKEND="x11"
      ;;
    --wayland)
      WINDOW_BACKEND="wayland"
      ;;
    *)
      CARGO_ARGS+=("$argument")
      ;;
  esac
done

case "$WINDOW_BACKEND" in
  auto|x11|wayland) ;;
  *)
    echo "QIRING_WINDOW_BACKEND must be auto, x11, or wayland." >&2
    exit 2
    ;;
esac

if [[ "$OS" == "Linux" ]]; then
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"

  if [[ "${QIRING_SOFTWARE_RENDERING:-0}" == "1" ]]; then
    export WEBKIT_DISABLE_COMPOSITING_MODE="${WEBKIT_DISABLE_COMPOSITING_MODE:-1}"
    export LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}"
  fi

  if [[ "$WINDOW_BACKEND" == "x11" && -z "${DISPLAY:-}" ]]; then
    for d in :0 :1 :2; do
      if DISPLAY="$d" xdpyinfo >/dev/null 2>&1; then
        export DISPLAY="$d"
        break
      fi
    done
  fi

  if [[ "$WINDOW_BACKEND" == "x11" ]]; then
    if [[ -z "${DISPLAY:-}" ]]; then
      echo "QiRing could not find an available X11 display." >&2
      exit 1
    fi
    export GDK_BACKEND="x11"
    export WINIT_UNIX_BACKEND="x11"
  elif [[ "$WINDOW_BACKEND" == "wayland" ]]; then
    export GDK_BACKEND="wayland"
    export WINIT_UNIX_BACKEND="wayland"
  elif [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
    for d in :0 :1 :2; do
      if DISPLAY="$d" xdpyinfo >/dev/null 2>&1; then
        export DISPLAY="$d"
        export GDK_BACKEND="x11"
        export WINIT_UNIX_BACKEND="x11"
        break
      fi
    done
  fi
fi

cargo run -p qiring-desktop "${CARGO_ARGS[@]}"
