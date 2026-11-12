#!/usr/bin/env bash
# =============================================================================
# test/test-gguffer.sh — el gguffer con checkpoints que YA VIENEN CUANTIZADOS
# =============================================================================
# El caso que motiva todo esto: hay checkpoints (los de NVIDIA, los de
# modelopt, el gpt-oss) cuyos pesos YA vienen cuantizados de fabrica. Con
# ellos, el gguffer fallaba siempre con "el repo YA VIENE CUANTIZADO",
# incluso sin pedir --quant: no habia forma de convertir y colocar un
# NVFP4 sin que se quejara.
#
# Y el otro caso: pedir un quant DISTINTO al que ya traen los pesos. El
# motor no puede re-cuantizar ("requantizing from type nvfp4 is disabled"),
# y hay que decirlo ANTES de bajar 60 GB, diciendo cual es el repo hermano
# sin cuantizar que si sirve.
#
# El test es HERMETICO: no toca la red ni el disco del usuario. Todo lo que
# el gguffer necesita de fuera esta falseado:
#
#   · curl  -> la metadata de los repos (test/fakes/gguffer/curl)
#   · hf    -> la descarga de los pesos y del imatrix (test/fakes/gguffer/hf)
#   · el motor (LLAMA_DIR) -> un conversor y un cuantizador falsos
#     (test/fakes/gguffer/motor/)
#
# Y se comprueba lo que de verdad importa:
#   · que NO se cuantice lo que ya esta en el quant pedido;
#   · que se error TARDE (antes de descargar) cuando no se puede;
#   · que el imatrix se exija cuando se cuantiza, y no se baje cuando no.
# =============================================================================
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAKES="$ROOT/test/fakes/gguffer"
TMP="${TMPDIR:-/tmp}/gguffer-test.$$"
mkdir -p "$TMP" || exit 2
trap 'rm -rf "$TMP"' EXIT

fallos=0
ok()  { printf 'ok   %s\n' "$1"; }
mal() { printf 'FALLO %s\n' "$1" >&2; fallos=$((fallos + 1)); }

# ── el entorno falso ────────────────────────────────────────────────────────
export FAKE_HF_DIR="$TMP/hf"
export FAKE_HF_LOG="$TMP/hf.log"
export FAKE_QUANT_LOG="$TMP/quant.log"
export FAKE_GGUF_PY="$FAKES/gguf_fake.py"
export LLAMA_DIR="$FAKES/motor"
export ROPERO_ROOT="$ROOT"
export HOME="$TMP/home"
export GGUF_DIR="$TMP/gguf"
export GGUF_WORK_DIR="$TMP/work"
export PATH="$FAKES:$PATH"
mkdir -p "$HOME" "$GGUF_DIR" "$GGUF_WORK_DIR" "$TMP/out"

# ── los repos de ejemplo ────────────────────────────────────────────────────
mkrepo() {
    mkdir -p "$FAKE_HF_DIR/$1"
    cat > "$FAKE_HF_DIR/$1/config.json"
}

# Un repo con los pesos YA en NVFP4 (compressed-tensors, formato NVIDIA).
mkrepo "acme/modelo-NVFP4" <<'EOF'
{
  "_id": "acme/modelo-NVFP4",
  "_name": "modelo-nvfp4",
  "architectures": ["GrootForCausalLM"],
  "quantization_config": {
    "quant_method": "compressed-tensors",
    "format": "nvfp4-pack-quantized",
    "config_groups": {
      "group_0": {"format": "nvfp4-pack-quantized", "weights": {"num_bits": 4, "group_size": 16}}
    }
  }
}
EOF

# Un repo con los pesos en FP8: el conversor los DEQUANTIZA a F16.
mkrepo "acme/modelo-FP8" <<'EOF'
{
  "_id": "acme/modelo-FP8",
  "_name": "modelo-fp8",
  "architectures": ["Qwen3ForCausalLM"],
  "quantization_config": {
    "quant_method": "compressed-tensors",
    "format": "float-quantized",
    "config_groups": {
      "group_0": {"format": "float-quantized", "weights": {"num_bits": 8, "strategy": "tensor"}}
    }
  }
}
EOF

# El hermano SIN cuantizar del de arriba: es lo que el gguffer tiene que
# sugerir cuando le pides un quant que no se puede (re-cuantizar).
mkrepo "acme/modelo-BF16" <<'EOF'
{
  "_id": "acme/modelo-BF16",
  "_name": "modelo-BF16",
  "architectures": ["GrootForCausalLM"]
}
EOF
# Un repo normal, sin cuantizar (BF16).
mkrepo "acme/modelo-plano" <<'EOF'
{
  "_id": "acme/modelo-plano",
  "_name": "modelo-bf16",
  "architectures": ["Qwen3ForCausalLM"]
}
EOF

# El imatrix, publicado en otro repo.
mkdir -p "$FAKE_HF_DIR/otros/imatrixes"
python3 "$FAKE_GGUF_PY" create "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" 1

# ── el motor de mentira ─────────────────────────────────────────────────────
# Los GGUFs de entrada (para las pruebas con --gguf).
python3 "$FAKE_GGUF_PY" create "$TMP/entrada-nvfp4.gguf" 39
python3 "$FAKE_GGUF_PY" create "$TMP/entrada-f16.gguf" 1

# ── correr el gguffer ───────────────────────────────────────────────────────
OUT=""
RC=0
gguffer() {
    : > "$FAKE_HF_LOG"
    : > "$FAKE_QUANT_LOG"
    OUT="$(bash "$ROOT/bin/ropero" gguffer "$@" 2>&1)"
    RC=$?
    return 0
}

ftype() { python3 "$FAKE_GGUF_PY" file_type "$1" 2>/dev/null || true; }
# OJO: "grep -c" imprime 0 Y sale con codigo 1 cuando no hay nada. Con un
# "|| echo 0" se imprimen DOS ceros y la comparacion falla siempre.
descarga_pesos() {
    local n
    n="$(grep -c "hf download $1 " "$FAKE_HF_LOG" 2>/dev/null || true)"
    printf '%s' "${n:-0}"
}
cuantizaciones() {
    local n
    n="$(wc -l < "$FAKE_QUANT_LOG" 2>/dev/null || true)"
    printf '%s' "${n:-0}" | tr -d ' '
}

printf '\n=== 1. un repo que YA VIENE en NVFP4, sin --quant ===\n'
gguffer --repo acme/modelo-NVFP4 --out "$TMP/out/nvfp4-sin-quant.gguf"
[[ $RC -eq 0 ]] && ok "convierte y coloca sin quejarse" || mal "rc=$RC: $OUT"
[[ -f "$TMP/out/nvfp4-sin-quant.gguf" ]] && ok "el GGUF esta donde se pidio" || mal "no hay GGUF"
[[ "$(ftype "$TMP/out/nvfp4-sin-quant.gguf")" == "39" ]] && ok "el GGUF esta en NVFP4 (39)" || mal "file_type=$(ftype "$TMP/out/nvfp4-sin-quant.gguf")"
[[ "$(cuantizaciones)" == "0" ]] && ok "NO se cuantiza nada" || mal "se cuantizo: $(cat "$FAKE_QUANT_LOG")"
grep -q "NVFP4" <<<"$OUT" && ok "lo dice en el mensaje" || mal "el mensaje no menciona NVFP4"

printf '\n=== 2. un repo que YA VIENE en NVFP4, con --quant NVFP4 ===\n'
gguffer --repo acme/modelo-NVFP4 --quant NVFP4 --out "$TMP/out/nvfp4-mismo-quant.gguf"
[[ $RC -eq 0 ]] && ok "acepta el mismo quant" || mal "rc=$RC: $OUT"
[[ "$(cuantizaciones)" == "0" ]] && ok "NO se cuantiza otra vez" || mal "se cuantizo: $(cat "$FAKE_QUANT_LOG")"
[[ "$(ftype "$TMP/out/nvfp4-mismo-quant.gguf")" == "39" ]] && ok "el GGUF esta en NVFP4" || mal "file_type=$(ftype "$TMP/out/nvfp4-mismo-quant.gguf")"

printf '\n=== 3. un repo en NVFP4, con un --quant DISTINTO ===\n'
gguffer --repo acme/modelo-NVFP4 --quant Q4_K_M --out "$TMP/out/nvfp4-otro-quant.gguf"
[[ $RC -ne 0 ]] && ok "falla (no se puede re-cuantizar)" || mal "deberia fallar: $OUT"
grep -qiE "re-cuantizar|no puede" <<<"$OUT" && ok "el error lo dice" || mal "el error no lo dice: $OUT"
[[ "$(descarga_pesos acme/modelo-NVFP4)" == "0" ]] && ok "NO baja los pesos (el error es ANTES de los 60 GB)" || mal "bajo los pesos antes de fallar"
grep -q "acme/modelo-BF16" <<<"$OUT" && ok "dice cual es el repo sin cuantizar" || mal "no dice que repo usar: $OUT"

printf '\n=== 4. un repo normal, con --quant y SIN imatrix ===\n'
gguffer --repo acme/modelo-plano --quant Q4_K_M --out "$TMP/out/bf16-sin-imatrix.gguf"
[[ $RC -ne 0 ]] && ok "falla (cuantizar sin imatrix no puede ser)" || mal "deberia fallar: $OUT"
grep -qi "imatrix" <<<"$OUT" && ok "el error habla del imatrix" || mal "el error no habla del imatrix: $OUT"
[[ "$(descarga_pesos acme/modelo-plano)" == "0" ]] && ok "NO baja los pesos (el error es ANTES de los 60 GB)" || mal "bajo los pesos antes de fallar"

printf '\n=== 5. un repo normal, con --quant y el imatrix del repo ===\n'
gguffer --repo acme/modelo-plano --quant Q4_K_M \
    --imatrix-repo otros/imatrixes --imatrix-file modelo.imatrix.gguf \
    --out "$TMP/out/bf16-con-imatrix.gguf"
[[ $RC -eq 0 ]] && ok "cuantiza" || mal "rc=$RC: $OUT"
[[ "$(cuantizaciones)" == "1" ]] && ok "llama al cuantizador una vez" || mal "cuantizaciones=$(cuantizaciones)"
grep -q "Q4_K_M" "$FAKE_QUANT_LOG" && ok "pide el Q4_K_M" || mal "pide: $(cat "$FAKE_QUANT_LOG")"
[[ "$(ftype "$TMP/out/bf16-con-imatrix.gguf")" == "15" ]] && ok "el GGUF sale en Q4_K_M (15)" || mal "file_type=$(ftype "$TMP/out/bf16-con-imatrix.gguf")"
[[ -f "$TMP/out/bf16-con-imatrix.gguf.fuente.json" ]] && ok "deja la procedencia" || mal "no hay .fuente.json"
grep -q '"quant": "Q4_K_M"' "$TMP/out/bf16-con-imatrix.gguf.fuente.json" 2>/dev/null \
    && ok "la procedencia dice el quant (Q4_K_M, no un numero)" \
    || mal "la procedencia no dice el quant: $(cat "$TMP/out/bf16-con-imatrix.gguf.fuente.json" 2>/dev/null)"

printf '\n=== 6. un repo normal, con --quant y el imatrix en local ===\n'
gguffer --repo acme/modelo-plano --quant Q4_K_M \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --out "$TMP/out/bf16-imatrix-local.gguf"
[[ $RC -eq 0 ]] && ok "cuantiza" || mal "rc=$RC: $OUT"
[[ "$(cuantizaciones)" == "1" ]] && ok "llama al cuantizador una vez" || mal "cuantizaciones=$(cuantizaciones)"

printf '\n=== 7. la matriz de argumentos del imatrix ===\n'
gguffer --repo acme/modelo-plano --quant Q4_K_M \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --imatrix-repo otros/imatrixes --imatrix-file modelo.imatrix.gguf \
    --out "$TMP/out/x.gguf"
[[ $RC -ne 0 ]] && ok "--imatrix y --imatrix-repo son excluyentes" || mal "deberia fallar: $OUT"

gguffer --repo acme/modelo-plano --quant Q4_K_M --imatrix-repo otros/imatrixes \
    --out "$TMP/out/x.gguf"
[[ $RC -ne 0 ]] && ok "--imatrix-repo sin --imatrix-file falla" || mal "deberia fallar: $OUT"

gguffer --repo acme/modelo-plano --quant Q4_K_M --imatrix-file modelo.imatrix.gguf \
    --out "$TMP/out/x.gguf"
[[ $RC -ne 0 ]] && ok "--imatrix-file sin --imatrix-repo falla" || mal "deberia fallar: $OUT"

gguffer --repo acme/modelo-plano \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --out "$TMP/out/x.gguf"
[[ $RC -ne 0 ]] && ok "un imatrix sin --quant falla (no hay nada que cuantizar)" || mal "deberia fallar: $OUT"

gguffer --repo acme/modelo-plano \
    --imatrix-repo otros/imatrixes --imatrix-file modelo.imatrix.gguf \
    --out "$TMP/out/x.gguf"
[[ $RC -ne 0 ]] && ok "el par del repo sin --quant tambien falla" || mal "deberia fallar: $OUT"

printf '\n=== 8. un GGUF que ya esta en NVFP4 (--gguf) ===\n'
gguffer --gguf "$TMP/entrada-nvfp4.gguf" --quant NVFP4 --out "$TMP/out/gguf-nvfp4.gguf"
[[ $RC -eq 0 ]] && ok "acepta el mismo quant" || mal "rc=$RC: $OUT"
[[ "$(cuantizaciones)" == "0" ]] && ok "NO se cuantiza" || mal "se cuantizo: $(cat "$FAKE_QUANT_LOG")"

gguffer --gguf "$TMP/entrada-nvfp4.gguf" --quant Q4_K_M --out "$TMP/out/gguf-nvfp4-otro.gguf"
[[ $RC -ne 0 ]] && ok "con otro quant falla (no se puede re-cuantizar)" || mal "deberia fallar: $OUT"
grep -qiE "re-cuantizar|no puede" <<<"$OUT" && ok "el error lo dice" || mal "el error no lo dice: $OUT"

printf '\n=== 9. un GGUF en F16 con --quant: aqui si cuantiza ===\n'
gguffer --gguf "$TMP/entrada-f16.gguf" --quant Q4_K_M \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --out "$TMP/out/gguf-f16-q4.gguf"
[[ $RC -eq 0 ]] && ok "cuantiza" || mal "rc=$RC: $OUT"
[[ "$(cuantizaciones)" == "1" ]] && ok "llama al cuantizador" || mal "cuantizaciones=$(cuantizaciones)"

gguffer --gguf "$TMP/entrada-f16.gguf" --quant Q4_K_M --out "$TMP/out/gguf-f16-sin-imatrix.gguf"
[[ $RC -ne 0 ]] && ok "sin imatrix falla" || mal "deberia fallar: $OUT"

printf '\n=== 10. un repo que viene en FP8 (se dequantiza) ===\n'
gguffer --repo acme/modelo-FP8 --quant Q4_K_M \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --out "$TMP/out/fp8-cuantizado.gguf"
[[ $RC -eq 0 ]] && ok "se puede cuantizar desde el F16 dequantizado" || mal "rc=$RC: $OUT"
grep -qiE "dequantiz|DEQUANTIZA" <<<"$OUT" && ok "avisa de que se dequantiza" || mal "no avisa: $OUT"

printf '\n=== 11. --diagnostico con imatrix y sin --quant ===\n'
gguffer --diagnostico --repo acme/modelo-plano \
    --imatrix "$FAKE_HF_DIR/otros/imatrixes/modelo.imatrix.gguf" \
    --out "$TMP/out/diag.gguf"
[[ $RC -eq 0 ]] && ok "el diagnostico se exime de la regla del imatrix" || mal "rc=$RC: $OUT"

printf '\n'
if (( fallos == 0 )); then
    printf 'todo ok\n'
    exit 0
fi
printf '%s comprobacion(es) fallida(s)\n' "$fallos" >&2
exit 1
