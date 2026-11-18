#!/usr/bin/env bash
set -euo pipefail

clear

TOOLCHAIN=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)
if [[ -z "$TOOLCHAIN" ]]; then
  echo "❌ No se pudo leer el canal de rust-toolchain.toml" >&2
  exit 1
fi

MODE="dev"
for arg in "$@"; do
  case "$arg" in
    --dev)  MODE="dev"  ;;
    --prod) MODE="prod" ;;
    *)
      echo "Uso: $0 [--dev | --prod]" >&2
      exit 1
      ;;
  esac
done

# Detectar el directorio del paquete que produce el binario "ropero"
PKG_DIR=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "
import json, sys, os
d = json.load(sys.stdin)
for pkg in d['packages']:
    if any(t['name'] == 'ropero' and 'bin' in t['kind'] for t in pkg['targets']):
        print(os.path.dirname(pkg['manifest_path']))
        break
")

if [ -z "${PKG_DIR:-}" ]; then
  echo "❌ No se encontró ningún paquete que produzca el binario 'ropero'" >&2
  exit 1
fi

echo "📦 Paquete detectado en: $PKG_DIR"

cargo clean
cargo fmt --all -- --check


if [ "$MODE" = "dev" ]; then
  echo "🔧 Compilando e instalando en modo desarrollo (debug)…"
  cargo "+$TOOLCHAIN" install --path "$PKG_DIR" --debug --root "$HOME/.local" --locked --force
else
  echo "🚀 Compilando e instalando en modo producción (release)…"
  cargo "+$TOOLCHAIN" install --path "$PKG_DIR" --root "$HOME/.local" --locked --force
fi

echo "✅ Listo. Binario en $HOME/.local/bin/ropero"
