#!/usr/bin/env bash
set -euo pipefail

BASE=$HOME/test
OUT=$BASE/resultados
TSV=$OUT/resumen-global.tsv

printf 'modelo\tprueba\tinput\treasoning\toutput\tprompt_tps\tgen_tps\n' > "$TSV"

for ses in "$BASE"/*/*/session_opencode.json; do
  [ -f "$ses" ] || continue
  prueba_dir=$(dirname "$ses")
  prueba=$(basename "$prueba_dir")
  modelo=$(basename "$(dirname "$prueba_dir")")
  [ "$modelo" = "resultados" ] && continue

  in=$(jq -r '[.. | objects | select(has("usage")) | .usage
               | (.input_tokens // .prompt_tokens // 0)] | add // 0' "$ses" 2>/dev/null)
  out=$(jq -r '[.. | objects | select(has("usage")) | .usage
                | (.output_tokens // .completion_tokens // 0)] | add // 0' "$ses" 2>/dev/null)
  reas=$(jq -r '[.. | objects | select(has("usage")) | .usage
                 | (.reasoning_tokens // 0)] | add // 0' "$ses" 2>/dev/null)

  log="$prueba_dir/log_llama.log"
  p_tps="—"; g_tps="—"
  if [ -f "$log" ]; then
    p_tps=$(grep 'prompt eval time' "$log" \
            | grep -oP '[\d.]+(?= tokens per second)' \
            | sort -g | tail -n1 || true)
    g_tps=$(grep 'eval time' "$log" \
            | grep -v 'prompt eval' \
            | grep -oP '[\d.]+(?= tokens per second)' \
            | sort -g | tail -n1 || true)
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$modelo" "$prueba" \
    "${in:-0}" "${reas:-0}" "${out:-0}" \
    "${p_tps:-—}" "${g_tps:-—}" >> "$TSV"
done

echo "[+] $TSV"
column -t -s $'\t' "$TSV"