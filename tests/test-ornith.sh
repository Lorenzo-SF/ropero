#!/usr/bin/env bash
# =============================================================================
# tests/test-MODELO.sh — test mantenible (refactor) con recopilación
# =============================================================================
# bash 3.2 (macOS): sin namerefs, sin arrays asociativos.
# Usa tests/lib/test-common.sh para funciones reutilizables.
# =============================================================================
TEST_MODEL="ornith"
TEST_DOCS_DIR="${ROPERO_ROOT:-.}/docs"
TEST_LOG_FILE="$HOME/.logs/ropero/ornith.log"
if [[ -f "\${ROPERO_ROOT:-.}/tests/lib/test-common.sh" ]]; then . "\${ROPERO_ROOT:-.}/tests/lib/test-common.sh"; fi

cd ~/test/$TEST_MODEL || exit 2

# -----------------------------------------------------------------
for sufijo in "" "largo" "gorda"; do
    case "$sufijo" in largo) num="larga";; gorda) num="gorda";; *) num="";; esac
    for i in $(seq 1 3); do
        [[ -n "$sufijo" ]] && break
        pkill -9 opencode 2>/dev/null; pkill -9 mcode 2>/dev/null; pkill -9 python 2>/dev/null; pkill -9 llama-server 2>/dev/null
        rm -f "$TEST_LOG_FILE"
        ropero $TEST_MODEL --background
        sleep 20
        prueba_dir="$HOME/test/$TEST_MODEL/prueba${i}"
        mkdir -p "$prueba_dir" || exit 2
        cd "$prueba_dir"
        opencode run "$(</home/merendandum/workspace/github/lasaca/ropero/docs/prueba${i}.md)" --agent build --model "marichocho/Puertourraco-256k" --format json > prueba${i}.json 2>/dev/null || printf "AVISO: opencode run prueba${i} falló
" >&2
        session${i}=$(cat prueba${i}.json 2>/dev/null | jq -r "select(.sessionID or .session_id) | (.sessionID // .session_id)" | sort | uniq | head -1)
        [[ -n "$session${i}" ]] && opencode session export "$session${i}" > sesion-prueba${i}.md 2>/dev/null || printf "AVISO: export session${i} falló
" >&2
        cp -f "$TEST_LOG_FILE" log-llama-sesion-prueba${i}.txt 2>/dev/null || printf "AVISO: no hay log para prueba${i}
" >&2
    done
done

# Recopilación final: archivo único con toda la recopilación
TEST_RESUMEN="$HOME/test/$TEST_MODEL/recopilacion-completa.md"
mkdir -p "$(dirname "$TEST_RESUMEN")" || exit 2
_test_generate_summary "$TEST_RESUMEN"
# Añadir recopilación de cada prueba al archivo final
for sufijo in "" "largo" "gorda"; do
    case "$sufijo" in largo) num="larga";; gorda) num="gorda";; *) num="";; esac
    for i in $(seq 1 3); do
        [[ -n "$sufijo" ]] && break
        printf "---
" >> "$TEST_RESUMEN"
        printf "PRUEBA %s (%s)
" "$i" "${sufijo:-normal}" >> "$TEST_RESUMEN"
        printf "Directorio: $HOME/test/$TEST_MODEL/prueba${i}
" >> "$TEST_RESUMEN"
        printf "Log: %s
" "$TEST_LOG_FILE" >> "$TEST_RESUMEN"
        printf "JSON: $HOME/test/$TEST_MODEL/prueba${i}/prueba${i}.json
" >> "$TEST_RESUMEN"
        printf "Sesión: $HOME/test/$TEST_MODEL/prueba${i}/sesion-prueba${i}.md
" >> "$TEST_RESUMEN"
    done
done
printf "
Recopilación completa generada: %s
" "$TEST_RESUMEN"

