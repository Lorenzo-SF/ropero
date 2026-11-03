#!/usr/bin/env bash
# =============================================================================
# tests/lib/test-common.sh — funciones comunes para todos los test-*.sh
# =============================================================================
# bash 3.2 (macOS): sin namerefs, sin arrays asociativos, sin mapfile.
# Solo definiciones; 0 ejecución.
# =============================================================================

# Variables que cada test debe exportar antes de llamar a estas funciones:
#   TEST_MODEL         (ej. agenta1, gptoss, groot...)
#   TEST_DOCS_DIR      (default: $ROPERO_ROOT/docs)
#   TEST_LOG_FILE      (default: $HOME/.logs/ropero/$TEST_MODEL.log)

TEST_MODEL="${TEST_MODEL:-}"
TEST_DOCS_DIR="${TEST_DOCS_DIR:-}"
TEST_LOG_FILE="${TEST_LOG_FILE:-}"

# -----------------------------------------------------------------------------
# _test_setup_model — inicializa variables derivadas del modelo.
# -----------------------------------------------------------------------------
_test_setup_model() {
    [[ -n "$TEST_MODEL" ]] || { printf 'TEST_MODEL no definido\n' >&2; return 2; }
    : "${TEST_DOCS_DIR:=${ROPERO_ROOT:-.}/docs}"
    : "${TEST_LOG_FILE:=$HOME/.logs/ropero/$TEST_MODEL.log}"
    printf '%s\n' "$TEST_MODEL"
}

# -----------------------------------------------------------------------------
# _test_run_prueba <N> [largo|gorda] — ejecuta UNA prueba y recopila los
# 3 archivos importantes (log, json, sesion). Si se llama sin argumentos
# adicionales usa prueba<N>.md; con "largo" usa prueba_larga.md; con "gorda"
# usa prueba_gorda.md.
# -----------------------------------------------------------------------------
_test_run_prueba() {
    local num="$1" sufijo="$2"
    local md_file prueba_dir
    md_file="${TEST_DOCS_DIR}/prueba${num}.md"
    [[ "$sufijo" == "largo" ]] && md_file="${TEST_DOCS_DIR}/prueba_larga.md"
    [[ "$sufijo" == "gorda" ]] && md_file="${TEST_DOCS_DIR}/prueba_gorda.md"

    if [[ ! -f "$md_file" ]]; then
        printf 'FALTA %s\n' "$md_file" >&2
        return 1
    fi

    local prueba_dir="$HOME/test/$TEST_MODEL/prueba${num}${sufijo:+-$sufijo}"
    mkdir -p "$prueba_dir" || return 1

    printf '\n=== prueba %s (modelo: %s) ===\n' "$num" "$TEST_MODEL"
    printf 'directorio: %s\n' "$prueba_dir"
    printf 'markdown: %s\n' "$md_file"
    printf 'log: %s\n' "$TEST_LOG_FILE"
}

# -----------------------------------------------------------------------------
# _test_collect_log — recopila el log del modelo en el archivo recopilado.
# -----------------------------------------------------------------------------
_test_collect_log() {
    local destino="$1"
    if [[ -f "$TEST_LOG_FILE" ]]; then
        cp -f "$TEST_LOG_FILE" "$destino" || printf 'AVISO: no se pudo copiar log a %s\n' "$destino" >&2
    else
        printf 'AVISO: no existe log %s\n' "$TEST_LOG_FILE" >&2
    fi
}

# -----------------------------------------------------------------------------
# _test_generate_summary <archivo-salida> — genera archivo único con
# recopilación completa de los archivos recopilados para un test.
# -----------------------------------------------------------------------------
_test_generate_summary() {
    local out="$1"
    local modelo="${TEST_MODEL:-desconocido}"
    printf '# Recopilación de test: %s\n' "$modelo" > "$out"
    printf 'Fecha: %s\n' "$(date -Iseconds 2>/dev/null || date '+%Y-%m-%d %H:%M:%S')" >> "$out"
    printf 'Modelo: %s\n' "$modelo" >> "$out"
    printf 'Documentos: %s\n' "${TEST_DOCS_DIR:-$ROPERO_ROOT/docs}" >> "$out"
    printf 'Log base: %s\n' "${TEST_LOG_FILE:-$HOME/.logs/ropero/$TEST_MODEL.log}" >> "$out"
    printf '---\n' >> "$out"
    printf 'Cada prueba genera 3 archivos recopilados:\n' >> "$out"
    printf '  1. log del modelo (%s)\n' "${TEST_LOG_FILE:-$HOME/.logs/ropero/$TEST_MODEL.log}" >> "$out"
    printf '  2. JSON del resultado (prueba<N>.json)\n' >> "$out"
    printf '  3. Sesión exportada (sesion-prueba<N>.md)\n' >> "$out"
    printf '---\n' >> "$out"
    printf 'Este archivo es la recopilación consolidada para revisión.\n' >> "$out"
}
