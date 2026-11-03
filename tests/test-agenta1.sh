#!/usr/bin/env bash
# =============================================================================
# tests/test-agenta1.sh — test mantenible (refactor) con recopilación
# =============================================================================
# bash 3.2 (macOS): sin namerefs, sin arrays asociativos.
# Usa tests/lib/test-common.sh para funciones reutilizables.
# =============================================================================

TEST_MODEL="agenta1"
TEST_DOCS_DIR="${ROPERO_ROOT:-.}/docs"
TEST_LOG_FILE="$HOME/.logs/ropero/agenta1.log"

if [[ -f "${ROPERO_ROOT:-.}/tests/lib/test-common.sh" ]]; then
    . "${ROPERO_ROOT:-.}/tests/lib/test-common.sh"
fi

# Función auxiliar para recopilar los 3 archivos de cada prueba.
_test_recolectar_prueba() {
    local num="$1" sufijo="$2" destino="$3"
    local prueba_dir="$HOME/test/$TEST_MODEL/prueba${num}${sufijo:+-$sufijo}"
    local md_file="$TEST_DOCS_DIR/prueba${num}.md"
    [[ "$sufijo" == "largo" ]] && md_file="$TEST_DOCS_DIR/prueba_larga.md"
    [[ "$sufijo" == "gorda" ]] && md_file="$TEST_DOCS_DIR/prueba_gorda.md"

    # 1. Recopilar log
    if [[ -f "$TEST_LOG_FILE" ]]; then
        cp -f "$TEST_LOG_FILE" "$destino" || printf 'AVISO: no se pudo recopilar log en %s\n' "$destino" >&2
    else
        printf 'AVISO: log %s no existe\n' "$TEST_LOG_FILE" >&2
    fi

    # 2. Recopilar JSON del resultado (prueba<N>.json)
    local json_file="$prueba_dir/prueba${num}.json"
    if [[ -f "$json_file" ]]; then
        printf 'JSON recopilado: %s\n' "$json_file" >> "$destino"
    else
        printf 'AVISO: JSON %s no existe\n' "$json_file" >> "$destino"
    fi

    # 3. Recopilar sesión exportada (sesion-prueba<N>.md)
    local md_sesion="$prueba_dir/sesion-prueba${num}.md"
    if [[ -f "$md_file" ]]; then
        printf 'Markdown usado: %s\n' "$md_file" >> "$destino"
    else
        printf 'AVISO: markdown %s no existe\n' "$md_file" >> "$destino"
    fi

    printf '--- recopilación prueba %s (%s) completa ---\n' "$num" "${sufijo:-normal}" >> "$destino"
}

cd ~/test/$TEST_MODEL || exit 2

# -----------------------------------------------------------------------------
# Pruebas 1-3 + larga + gorda (5 pruebas)
# -----------------------------------------------------------------------------
for sufijo in "" "largo" "gorda"; do
    # Determinar número de prueba según sufijo
    case "$sufijo" in
        "")    num=1 ;;  # para simplificar: se hace 3 veces, largo, gorda
        largo) num=larga ;;
        gorda) num=gorda ;;
    esac
    for i in $(seq 1 3); do
        # Si sufijo es vacío, hacemos 3 pruebas (1,2,3)
        # Si es largo o gorda, hacemos 1 sola prueba con ese sufijo
        [[ -n "$sufijo" ]] && break
        [[ -z "$sufijo" && $i -gt 3 ]] && break

        # Limpieza y arranque
        pkill -9 opencode 2>/dev/null; pkill -9 mcode 2>/dev/null; pkill -9 python 2>/dev/null; pkill -9 llama-server 2>/dev/null
        rm -f "$TEST_LOG_FILE"
        ropero $TEST_MODEL --background
        sleep 20

        mkdir -p ~/test/$TEST_MODEL/prueba${i}
        cd ~/test/$TEST_MODEL/prueba${i}
        opencode run "$(</home/merendendam/workspace/github/lasaca/ropero/docs/prueba${i}.md)" --agent build --model "marichocho/Puertourraco-256k" --format json > prueba${i}.json 2>/dev/null || printf 'AVISO: opencode run prueba${i} falló\n' >&2
        session${i}=$(cat prueba${i}.json 2>/dev/null | jq -r 'select(.sessionID or .session_id) | (.sessionID // .session_id)' | sort | uniq | head -1)
        if [[ -n "$session${i}" ]]; then
            opencode session export "$session${i}" > sesion-prueba${i}.md 2>/dev/null || printf 'AVISO: export session${i} falló\n' >&2
        fi
        cp -f "$TEST_LOG_FILE" log-llama-sesion-prueba${i}.txt 2>/dev/null || printf 'AVISO: no hay log para prueba${i}\n' >&2
    done
done

# -----------------------------------------------------------------------------
# Recopilación final: archivo único con toda la recopilación (5 pruebas)
# -----------------------------------------------------------------------------
TEST_RESUMEN="$HOME/test/$TEST_MODEL/recopilacion-completa.md"
mkdir -p "$(dirname "$TEST_RESUMEN")" || exit 2

_test_generate_summary "$TEST_RESUMEN"

# Añadir los archivos recopilados de cada prueba al archivo final
for sufijo in "" "largo" "gorda"; do
    case "$sufijo" in
        largo) num="larga" ;;
        gorda) num="gorda" ;;
        *)    num="" ;;
    esac

    # Pruebas 1-3
    for i in $(seq 1 3); do
        if [[ -z "$sufijo" ]]; then
            prueba_dir="$HOME/test/$TEST_MODEL/prueba${i}"
            archivo_recopilado="$prueba_dir/recopilacion-prueba${i}.txt"
            printf '---\n' >> "$TEST_RESUMEN"
            printf 'PRUEBA %s (normal)\n' "$i" >> "$TEST_RESUMEN"
            printf 'Directorio: %s\n' "$prueba_dir" >> "$TEST_RESUMEN"
            # Copiar contenido recopilado (log, json, sesion) en líneas resumidas
            if [[ -f "$TEST_LOG_FILE" ]]; then
                printf 'Log (líneas finales):\n' >> "$TEST_RESUMEN"
                tail -n 3 "$TEST_LOG_FILE" >> "$TEST_RESUMEN" 2>/dev/null || printf '(log vacío)\n' >> "$TEST_RESUMEN"
            fi
            printf 'JSON: %s/prueba%s.json\n' "$prueba_dir" "$i" >> "$TEST_RESUMEN"
            printf 'Sesión: %s/sesion-prueba%s.md\n' "$prueba_dir" "$i" >> "$TEST_RESUMEN"
        fi
    done

    # Pruebas largo / gorda
    if [[ -n "$sufijo" ]]; then
        prueba_dir="$HOME/test/$TEST_MODEL/prueba_${num}"
        [[ -d "$prueba_dir" ]] && {
            printf '---\n' >> "$TEST_RESUMEN"
            printf 'PRUEBA %s (%s)\n' "$num" "$sufijo" >> "$TEST_RESUMEN"
            printf 'Directorio: %s\n' "$prueba_dir" >> "$TEST_RESUMEN"
            if [[ -f "$TEST_LOG_FILE" ]]; then
                printf 'Log (líneas finales):\n' >> "$TEST_RESUMEN"
                tail -n 3 "$TEST_LOG_FILE" >> "$TEST_RESUMEN" 2>/dev/null || printf '(log vacío)\n' >> "$TEST_RESUMEN"
            fi
            printf 'JSON: %s/prueba_%s.json\n' "$prueba_dir" "$num" >> "$TEST_RESUMEN"
            printf 'Sesión: %s/sesion-prueba_%s.md\n' "$prueba_dir" "$num" >> "$TEST_RESUMEN"
        }
    fi
done

printf '\nRecopilación completa generada: %s\n' "$TEST_RESUMEN"
