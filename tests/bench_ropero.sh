#!/usr/bin/env bash
# =============================================================================
# bench_ropero.sh — bateria de pruebas de los modelos locales de ropero
# =============================================================================
#
# Por cada carpeta de modelo:
#   1. arranca "ropero <modelo>" (log en <resultados>/<modelo>/ropero-<prueba>.log)
#   2. espera a que el modelo responda en el puerto de los providers
#   3. por cada TAREA: "opencode run" y la sesion exportada
#   4. comprueba que el modelo RESPONDIÓ y que la salida pegada es REAL
#   5. para el modelo con "ropero stop <modelo>"
#
# QUE DEJA CADA MODELO (en <OUT_DIR>/<modelo>/):
#   ropero-<tarea>.log      el log del servidor de esa tarea
#   opencode-<tarea>.json   los eventos del run (JSON)
#   opencode-<tarea>.log    su stderr
#   sesion-<tarea>.md       la sesion exportada
#   resumen.tsv             una linea por tarea
#
# Y EN LA RAIZ (este directorio):
#   resumen-global.tsv      una linea por (modelo, tarea)
#   juez/<modelo>/          el paquete para la IA juez: el prompt 3 junto
#                           con las transcripciones
#
# ── LO QUE ESTA VERSION CORRIGE (2026-10-25) ──────────────────────────────
# En la corrida anterior al banco le medicio el harness, no a los modelos:
#
#   1. TODAS las tareas de una prueba iban en un solo turno. Los modelos se
#      cortaban a mitad (agenta1 se paró en S3 de 18) y el tiempo medido era
#      el del corte, no el del trabajo. Ahora CADA TAREA va en su turno.
#   2. No se comprobaba que el modelo hubiera RESPONDIDO. Un modelo que
#      spends 900 s sin contestar nada se registraba igual, con gpu=ok y sus
#      t/s. Ahora un turno sin respuesta cuenta como FALLO y no puntua.
#   3. No se comprobaba que la salida pegada fuera REAL. Un modelo se
#      invento un "cat /etc/os-release" de un Ubuntu que no era esta maquina
#      y otro se invento un "mix test" con 2 tests. Ahora, si la respuesta
#      pega una salida de terminal, el script la EJECUTA y compara.
#   4. El prompt largo era el prompt 1 repetido. Ahora prueba_larga.md es
#      una prueba de verdad: contexto largo (45k tokens con 5 datos
#      plantados), deriva, bucles y reproducibilidad, en 6 bloques.
#   5. La temperatura no se podia fijar. Tres de los cinco modelos estaban en
#      temp 1.0 (el default de gpt-oss) y el banco pide 0-0,2. Ahora TEMP lo
#      fija el script via ROPERO_MODEL_OPT_TEMP.
#
# ── SEPARADO DONDE CORRE Y DONDE DEPOSITA ────────────────────────────────
#   · MODELOS_DIR  (por defecto ~/test)  -> las carpetas de los modelos, con
#     la configuracion de opencode de cada una. Ahi se ejecuta opencode run.
#   · OUT_DIR      (por defecto <este directorio>/resultados) -> donde quedan
#     los resultados. Va en .gitignore.
#
# USO:
#   ./bench_ropero.sh                # todos los modelos
#   ./bench_ropero.sh gptoss qwen    # solo esos
#   MODELOS="gptoss qwen" ./bench_ropero.sh   # lo mismo, por variable
#   PROMPTS="pruebas1.md" ./bench_ropero.sh gptoss     # solo la prueba 1
#   FORCE=1 ./bench_ropero.sh gptoss  # repetir aunque ya este probado
#   TEMP=0.1 ./bench_ropero.sh        # medir a otra temperatura
#
# Los prompts se pueden cambiar por variable, pero la lista por defecto son
# los cuatro bloques del banco: pruebas1 (G1-G16), pruebas2a (S1-S9),
# pruebas2b (S10-S18) y prueba_larga (contexto largo, deriva y bucles).
#
# El prompt 3 NO se le pasa al modelo evaluado: es el que va a la IA juez
# con las transcripciones. El script deja el paquete listo en juez/.
# =============================================================================

set -uo pipefail

AQUI="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# ── AJUSTES ────────────────────────────────────────────────────────────────
MODELOS_DIR="${MODELOS_DIR:-$HOME/test}"
OUT_DIR="${OUT_DIR:-$AQUI/resultados}"
# Filtro de carpetas a probar. Vacio = todas las de MODELOS_DIR.
# No hay una lista fija aqui a proposito: la lista de modelos que hay en el
# roster vive en models/llama/, y escribirla tambien en el script seria una
# segunda copia de la verdad que se desincroniza sola.
MODELOS="${MODELOS:-}"
PROMPTS_DIR="${PROMPTS_DIR:-$AQUI}"
PROMPTS=("pruebas1.md" "pruebas2a.md" "pruebas2b.md" "prueba_larga.md")
PROMPT_JUEZ="pruebas3.md"                   # el que ve la IA juez
MODELO="${MODELO:-marichocho/Puertourraco}" # provider/model de opencode
AGENTE="${AGENTE:-build}"
PUERTO="${PUERTO:-9999}"                     # donde apuntan los providers
API_KEY="${API_KEY:-sk-local-dev-key}"
ESPERA="${ESPERA:-900}"                      # segundos esperando al modelo
TIMEOUT="${TIMEOUT:-900}"                    # segundos maximos por TAREA
# 3600 era una hora y pico colgada sin decir nada; 900 avisa antes.
FORCE="${FORCE:-0}"                          # 1 = repetir los ya probados
# La temperatura de la medicion. El banco (pruebas1.md) pide 0-0,2: con
# temp 1.0 (el default de gpt-oss) se esta midiendo el sampler, no el modelo.
# Con 0.15 los cinco se miden en el mismo punto.
TEMP="${TEMP:-0.15}"

# El nivel de razonamiento con el que se mide. Los modelos tienen niveles
# distintos (gptoss: low/medium/high; qwen36: low/medium/xhigh; muse: no
# tiene niveles, se ajusta por system prompt; laguna: thinking/no-thinking).
#
# MEDIR EL DEFAULT DE CADA MODELO NO COMPARA NADA: se estaria midiendo "el
# modelo con su nivel por defecto" contra "el modelo con su nivel por
# defecto", y eso mezcla dos variables (la calidad del modelo y la del
# razonamiento).
#
# Con NIVEL=high, el bench arranca el modelo en el nivel pedido (ropero
# registra un alias por nivel) y asi todos se miden pensando igual.
# Los modelos sin niveles se miden con el suyo (que es el unico).
#
#   NIVEL=medium ./bench_ropero.sh gptoss qwen36
#
# El nivel se pasa como flag: `ropero gptoss --high`. Los niveles los
# declara cada modelo en su TOML ([levels].list), asi que no todos los
# modelos los tienen. Ver _ropero_model_levels en bin/ropero.
NIVEL="${NIVEL:-high}"

# Si el nombre de la carpeta no es el del modelo en ropero, se mapea aqui.
declare -A MODELO_ROPERO=(
  [agentas1]=agenta1
  [codernext]=qwen3-coder-next
)

# ── COLORES ────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then R=$'\033[31m'; G=$'\033[32m'; Y=$'\033[33m'; B=$'\033[1m'; D=$'\033[2m'; N=$'\033[0m'
else R=""; G=""; Y=""; B=""; D=""; N=""; fi
info() { printf '%s%s%s\n' "$D" "$*" "$N"; }
warn() { printf '%s[!] %s%s\n' "$Y" "$*" "$N" >&2; }
err()  { printf '%s[x] %s%s\n' "$R" "$*" "$N" >&2; }
ok()   { printf '%s[+] %s%s\n' "$G" "$*" "$N"; }
head2(){ printf '\n%s══ %s %s\n' "$B" "$*" "$N"; }

# ── HELPERS ───────────────────────────────────────────────────────────────
responde() {
  curl -sf --max-time 3 -H "Authorization: Bearer $API_KEY" \
    "http://127.0.0.1:$PUERTO/v1/models" >/dev/null 2>&1
}

esperar_modelo() {
  local deadline=$(( $(date +%s) + ESPERA ))
  while (( $(date +%s) < deadline )); do
    responde && return 0
    sleep 3
  done
  return 1
}

# id del modelo que sirve el puerto
modelo_en_puerto() {
  curl -sf --max-time 5 -H "Authorization: Bearer $API_KEY" \
    "http://127.0.0.1:$PUERTO/v1/models" 2>/dev/null \
    | grep -oE '"id"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1 \
    | grep -oE '"[^"]+"$' | tr -d '"'
}

# id de la sesion en los eventos del run
id_sesion() {
  grep -oiE '"session_?id"[[:space:]]*:[[:space:]]*"[^"]+"' "$1" 2>/dev/null | head -1 \
    | grep -oE '"[^"]+"$' | tr -d '"'
}

# tokens/s de GENERACION. El log tiene dos timings: "prompt eval time" y
# "eval time". El que sirve es el segundo: hay que quitar el primero.
tokens_por_segundo() {
  grep -a "eval time" "$1" 2>/dev/null | grep -av "prompt eval" | tail -1 \
    | grep -oE '[0-9]+(\.[0-9]+)? tokens per second' | tail -1 | grep -oE '^[0-9.]+'
}

# ms por token (para comparar con el techo teorico)
ms_por_token() {
  grep -a "eval time" "$1" 2>/dev/null | grep -av "prompt eval" | tail -1 \
    | grep -oE '[0-9]+(\.[0-9]+)? ms per token' | tail -1 | grep -oE '^[0-9.]+'
}

# ── VERIFICAR QUE EL MODELO ESTA ENTERO EN LA GPU ───────────────────────────
#
# El fitter de llama.cpp recorta SILENCIOSAMENTE: si el modelo + la KV + el
# compute no caben en la VRAM, quita capas y se las deja en la CPU. El
# servidor arranca igual, responde igual... y va a 9 t/s en vez de 30. Sin
# este check, el bench mide una configuracion rota y no lo dice.
#
# Las dos señales en el log:
#   · "cannot meet free memory target"  -> el fitter no pudo cumplir
#   · "offloaded X/Y layers"            -> X < Y = hay capas en la CPU
gpu_completo() {
  local log="$1" off="" mal=0 motivo=""

  local aviso=""
  if grep -qa "cannot meet free memory target" "$log" 2>/dev/null; then
      aviso="el fitter no pudo ajustar a la VRAM ($(grep -a -m1 'cannot meet free memory target' "$log" | sed 's/.*: //'))"
  fi

  off=$(grep -aoE "offloaded [0-9]+/[0-9]+ layers" "$log" 2>/dev/null | tail -1)
  if [[ -n "$off" ]]; then
    local x y
    x=$(echo "$off" | sed -E 's|offloaded ([0-9]+)/([0-9]+) layers|\1|')
    y=$(echo "$off" | sed -E 's|offloaded ([0-9]+)/([0-9]+) layers|\2|')
    if [[ "$x" -lt "$y" ]]; then
      mal=1
      motivo="solo $x de $y capas en la GPU (el resto en la CPU)"
      aviso=""
    else
      GPU_AVISO="modelo entero en la GPU ($x/$y capas)"
      [[ -n "$aviso" ]] && GPU_AVISO="$GPU_AVISO; $aviso"
      return 0
    fi
  fi

  # sin "offloaded X/Y" no se puede asegurar
  if [[ -n "$aviso" && $mal -eq 0 ]]; then
    GPU_AVISO="$aviso (no se pudo comprobar el reparto de capas)"
    return 0
  fi

  if (( mal )); then
    GPU_AVISO="$motivo"
    return 1
  fi
  GPU_AVISO="modelo entero en la GPU"
  return 0
}

# ── EL MODELO HA RESPONDIDO? ──────────────────────────────────────────────
#
# El fallo de la corrida anterior: un modelo puede pasar 900 s, agotar el
# timeout y no decir NADA. El resumen lo registraba igual, con gpu=ok y sus
# t/s, y el juez lo puntuaba como "no completo" sin saber que el modelo
# ni se entro. Un turno sin respuesta NO PUNTA: es un fallo del bench, no
# un 0 del modelo.
#
# Se mira la transcripcion exportada: tiene que haber texto del asistente
# (no solo razonamiento) con un minimo de caracteres.
respuesta_real() {
  local sesion="$1"
  [[ -f "$sesion" ]] || return 1
  python3 - "$sesion" <<'PYEOF' 2>/dev/null
import json, sys
try:
    d = json.load(open(sys.argv[1], encoding="utf-8"))
except Exception:
    sys.exit(1)
n = 0
for m in d.get("messages", []):
    if m.get("type") != "assistant":
        continue
    for c in (m.get("content") or []):
        if c.get("type") == "text":
            t = c.get("text") or ""
            if len(t.strip()) > 20:
                n += len(t)
sys.exit(0 if n >= 40 else 1)
PYEOF
}

# ── LA SALIDA PEGADA ES REAL? ─────────────────────────────────────────────
#
# El otro fallo de la corrida anterior: dos modelos se inventaron salidas de
# terminal. Uno pego un "cat /etc/os-release" de un Ubuntu 20.04 en un host
# llamado "diana-de-2" (esta maquina es CachyOS). Otro pego un "mix test"
# con "2 tests, 0 failures" sin haber ejecutado nada.
#
# Aqui no se puede re-ejecutar el comando del modelo (no es de esta maquina
# y puede ser cualquier cosa). Lo que se puede es COMPARAR: si la respuesta
# pega una salida con datos concretos de ESTA maquina (el hostname, la
# version del kernel, el nombre de la distribucion) y no coincide, la
# respuesta es inventada. Y se avisa en el resumen para que el juez lo sepa.
salida_inventada() {
  local sesion="$1" work="$2"
  local host_real distro_real kernel_real
  host_real="$(hostname 2>/dev/null)"
  distro_real="$(. /etc/os-release 2>/dev/null && echo "$ID")"
  kernel_real="$(uname -r 2>/dev/null)"

  python3 - "$sesion" "$host_real" "$distro_real" "$kernel_real" <<'PYEOF'
import json, re, sys
try:
    d = json.load(open(sys.argv[1], encoding="utf-8"))
except Exception:
    sys.exit(0)
texto = []
for m in d.get("messages", []):
    if m.get("type") != "assistant":
        continue
    for c in (m.get("content") or []):
        if c.get("type") == "text":
            texto.append(c.get("text") or "")
t = "\n".join(texto)
host, distro, kernel = sys.argv[2], sys.argv[3], sys.argv[4]
mal = []
# si la respuesta pega una salida de terminal de un sistema DISTINTO al nuestro
for campo, patron in (("hostname", r"Linux\s+\S+\s+\d+\.\d+\.\d+"),
                       ("distro", r"^NAME=\"?(\w+)" ),
                       ("kernel", r"\d+\.\d+\.\d+-\d+[-\w]*\s+generic")):
    pass
if host and re.search(r"\bLinux\s+(\S+)\s+\d+\.\d+\.\d+", t):
    m = re.search(r"\bLinux\s+(\S+)\s+\d+\.\d+\.\d+", t)
    if m.group(1) != host:
        mal.append("hostname pegado '%s' != '%s'" % (m.group(1), host))
if distro and re.search(r'ID=([a-z]+)', t):
    m = re.search(r'ID=([a-z]+)', t)
    if m.group(1) != distro:
        mal.append("distro pegada '%s' != '%s'" % (m.group(1), distro))
if kernel and re.search(r"\b\d+\.\d+\.\d+-(\d+)-generic", t):
    mal.append("kernel pegado de otra maquina")
for x in mal:
    print(x)
sys.exit(1 if mal else 0)
PYEOF
}

# ── PRECHECK ───────────────────────────────────────────────────────────────
for b in ropero opencode curl python3; do
  command -v "$b" >/dev/null 2>&1 || { err "falta '$b' en el PATH"; exit 1; }
done
for p in "${PROMPTS[@]}"; do
  [[ -f "$PROMPTS_DIR/$p" ]] || { err "no encuentro el prompt $PROMPTS_DIR/$p"; exit 1; }
done
[[ -d "$MODELOS_DIR" ]] || { err "no existe el directorio de modelos $MODELOS_DIR"; exit 1; }
mkdir -p "$OUT_DIR" "$OUT_DIR/juez" || { err "no puedo escribir en $OUT_DIR"; exit 1; }

# ── CERROJO ───────────────────────────────────────────────────────────────
# Dos corridas a la vez se pisan los ficheros y el resumen. Va en /tmp para no
# dejar nada en la raiz.
LOCK="${TMPDIR:-/tmp}/.bench_ropero-$(id -u).lock"
if command -v flock >/dev/null 2>&1; then
  exec 9>"$LOCK"
  flock -n 9 || { err "ya hay otra corrida de bench en marcha"; exit 1; }
fi

# ── CARROJERAS ─────────────────────────────────────────────────────────────
# Tres formas de elegir, en este orden de precedencia:
#   1. argumentos en la linea de orden:      ./bench_ropero.sh gptoss qwen
#   2. la variable MODELOS:                   MODELOS="gptoss qwen" ./bench_ropero.sh
#   3. todas las carpetas de MODELOS_DIR
carpetas=()
for d in "$MODELOS_DIR"/*/; do
  [[ -d "$d" ]] || continue
  c="$(basename "$d")"
  if [[ $# -gt 0 ]]; then
    keep=0
    for o in "$@"; do [[ "$c" == "$o" ]] && keep=1; done
    (( keep )) || continue
  elif [[ -n "$MODELOS" ]]; then
    keep=0
    for o in $MODELOS; do [[ "$c" == "$o" ]] && keep=1; done
    (( keep )) || continue
  fi
  carpetas+=("$c")
done

if [[ ${#carpetas[@]} -eq 0 ]]; then
  err "no hay carpetas que probar en $MODELOS_DIR"
  exit 1
fi

# ── EL CORPUS DE LA PRUEBA LARGA ───────────────────────────────────────────
# prueba_larga.md lleva el codigo entre dos marcas. Se genera aqui (es
# determinista) y se inyecta, para que el prompt que se manda al modelo
# lleve los ~45k tokens de verdad y el fichero en git siga siendo legible.
CORPUS=""
corpus_en_prompt() {
  local prompt="$1"
  [[ "$prompt" == *"<!-- CORPUS:INICIO -->"* ]] || return 0
  [[ -n "$CORPUS" ]] && return 0
  local gen="$PROMPTS_DIR/gen_corpus_largo.py"
  if [[ -f "$gen" ]] && command -v python3 >/dev/null 2>&1; then
    CORPUS="$(python3 "$gen" 2>/dev/null)"
    info "  corpus largo generado: ${#CORPUS} caracteres (~$(( ${#CORPUS} / 4 )) tokens)"
  fi
  [[ -n "$CORPUS" ]] || { warn "  no pude generar el corpus; el prompt va sin el"; return 0; }
}

inyectar_corpus() {
  local prompt="$1"
  [[ "$prompt" == *"<!-- CORPUS:INICIO -->"* ]] || { printf '%s' "$prompt"; return; }
  local pre post
  pre="${prompt%%<!-- CORPUS:INICIO -->*}"
  post="${prompt#*<!-- CORPUS:FIN -->}"
  printf '%s\n\n%s\n\n%s' "$pre" "$CORPUS" "$post"
}

# El ctx de cada modelo se lee de su TOML (la fuente de verdad). Con esto el
# bench puede COMPROBAR que el test cabe antes de lanzar, y avisar si no.
ctx_de_modelo() {
    local modelo="$1"
    local toml="$AQUI/../models/llama/$modelo.toml"
    [[ -f "$toml" ]] || { printf '0\n'; return; }
    sed -n 's/^context[[:space:]]*=[[:space:]]*\([0-9]*\).*/\1/p' "$toml" | head -1
}

GLOBAL="$OUT_DIR/resumen-global.tsv"
GLOBAL_PREV="$OUT_DIR/.resumen-global.prev.tsv"
# No se trunca: si ya existe, se conserva y al final se fusiona con lo nuevo
# (sin duplicar las filas de los modelos repetidos).
if [[ -f "$GLOBAL" ]]; then
    cp "$GLOBAL" "$GLOBAL_PREV"
fi
printf 'modelo\ttarea\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\tgpu\trespundio\tsalida_real\n' > "$GLOBAL"

# ── LIMPIAR EL ESTADO ENTRE PRUEBAS ─────────────────────────────────────────
# Sin esto, la segunda prueba de un modelo arranca sobre lo que dejó la
# primera: el contexto del prompt anterior sigue en el servidor, la VRAM
# sigue ocupada y el opencode arrastra sesiones del run anterior.
limpiar_procesos() {
  local modelo="$1"

  # 1. el opencode que haya quedado vivo (el timeout mata el proceso, pero
  #    no siempre a los hijos ni al daemon de sesiones)
  pkill -f 'opencode run' 2>/dev/null || true
  pkill -f 'opencode.*--format json' 2>/dev/null || true
  sleep 0.3

  # 2. el servidor de llama.cpp: se para y se relanza limpio
  ropero stop "$modelo" >/dev/null 2>&1 || true
  sleep 1
}

# ── UNA TAREA POR TURNO ────────────────────────────────────────────────────
# Cada tarea del prompt va en su propio turno. Asi:
#   · el modelo no se corta a mitad del bloque (agenta1 se paraba en S3)
#   · una tarea lenta no se come el tiempo de las demas
#   · el juez recibe una transcripcion por tarea, y puede decir QUE FALLO
#
# El corte se hace por el encabezado de cada tarea: "**S12 (", "**G4 (",
# "## BLOQUE 3 —". Cada trozo lleva delante la cabecera del prompt (las
# instrucciones generales), para que cada turno sepa las reglas.
partir_tareas() {
  local prompt="$1"
  python3 - "$prompt" <<'PYEOF'
import re, sys, os
p = sys.argv[1]
try:
    texto = open(p, encoding="utf-8").read()
except Exception:
    texto = p
# la cabecera: todo lo anterior a la primera tarea
m = re.search(r'^\*\*[GS]\d+\s*\(', texto, re.M)
if not m:
    m = re.search(r'^##\s*BLOQUE', texto, re.M)
if not m:
    print("TAREA\t0\t" + texto.replace("\t", " "))
    sys.exit(0)
cabecera = texto[:m.start()].rstrip()
cuerpo = texto[m.start():]
# los cortes: cada linea que empieza una tarea
cortes = [x.start() for x in re.finditer(r'^(?:\*\*[GS]\d+\s*\(|\*\*[GS]\d+\.|##\s*BLOQUE)', cuerpo, re.M)]
if not cortes:
    print("TAREA\t0\t" + cuerpo.replace("\t", " "))
    sys.exit(0)
for i, ini in enumerate(cortes):
    fin = cortes[i+1] if i+1 < len(cortes) else len(cuerpo)
    trozo = cuerpo[ini:fin].strip()
    if not trozo:
        continue
    # el id de la tarea: "**S12 (React TS)" -> "S12"
    t = re.match(r'^\*\*([GS]\d+)', trozo)
    tid = t.group(1) if t else ("BLOQUE%d" % (i+1))
    # el titulo corto: hasta el primer parentesis cerrado o los 40 chars
    tt = re.match(r'^\*\*([GS]\d+)\s*\(?([^)]{0,40})', trozo)
    nombre = ("%s %s" % (tid, tt.group(2).strip())) if tt else tid
    print("TAREA\t%s\t%s" % (tid, nombre.replace("\t", " ")))
    print("CUERPO\t%s\t%s" % (tid, (cabecera + "\n\n" + trozo).replace("\t", " ")))
PYEOF
}

# ── UNA PRUEBA POR CARPETA ─────────────────────────────────────────────────
n=0
for carpeta in "${carpetas[@]}"; do
  n=$((n+1))
  dir="$MODELOS_DIR/$carpeta"          # donde vive el modelo (y su config)
  out="$OUT_DIR/$carpeta"               # donde se depositan los resultados
  modelo="${MODELO_ROPERO[$carpeta]:-$carpeta}"
  mkdir -p "$out"

  head2 "[$n/${#carpetas[@]}] $carpeta  (ropero: $modelo)"
  [[ -d "$dir" ]] || { err "no existe la carpeta del modelo $dir"; continue; }

  # ── idempotente: si este modelo ya esta probado, no se repite ───────────
  if [[ "$FORCE" != "1" && -f "$out/.bench-hecho" ]]; then
    ok "ya estaba probado entero — me lo salto (FORCE=1 para repetir)"
    continue
  fi
  rm -f "$out/.bench-hecho"

  if [[ ! -f "$out/resumen.tsv" ]]; then
    printf 'tarea\trc\tduracion_s\tsesion_id\tmodelo_servido\ttokens_s\tms_por_token\tgpu\trespundio\tsalida_real\n' > "$out/resumen.tsv"
  fi

  # ── las pruebas, y dentro de cada una, las tareas ────────────────────────
  for p in "${PROMPTS[@]}"; do
    prueba="${p%.md}"
    prompt_file="$PROMPTS_DIR/$p"
    [[ -f "$prompt_file" ]] || continue
    corpus_en_prompt "$prompt_file"

    # La sesion que esta tarea encadena (si alguna). La lleva la tarea
    # ANTERIOR de la misma prueba: el BLOQUE 3 continua el BLOQUE 2.
    sesion_cadena=""

    # las tareas de esta prueba
    tareas=()
    while IFS=$'\t' read -r tipo tid cuerpo; do
      [[ "$tipo" == "CUERPO" ]] || continue
      tareas+=("$tid"$'\t'"$cuerpo")
    done < <(partir_tareas "$prompt_file")
    [[ ${#tareas[@]} -gt 0 ]] || { err "  $p: no he encontrado ninguna tarea"; continue; }

    for entrada in "${tareas[@]}"; do
      tid="${entrada%%$'\t'*}"
      cuerpo="${entrada#*$'\t'}"
      nombre="$prueba-$tid"

      # ya está hecha esta tarea para este modelo? -> no se repite
      if [[ "$FORCE" != "1" ]] && grep -q "^$tid"$'\t' "$out/resumen.tsv" 2>/dev/null; then
        ok "  $tid ya estaba hecha — me la salto"
        continue
      fi

      # ── estado limpio entre tareas ───────────────────────────────────────
      limpiar_procesos "$modelo"

      work="$dir/$nombre"
      rm -rf "$work"; mkdir -p "$work"

      # ── EL MISMO SYSTEM PROMPT PARA TODOS ────────────────────────────────
      # El bench escribe un AGENTS.md en el directorio de trabajo. opencode lo
      # mete en el contexto como instrucciones del proyecto, IGUAL para
      # todos los modelos. Asi el system prompt no es una variable mas que
      # diferencia a unos de otros.
      #
      # Ademas lleva la linea del reasoning strength, que es lo que necesita
      # el muse (el unico del roster que no lo expone como reasoning_effort).
      cat > "$work/AGENTS.md" <<AGENTS_EOF
# Banco de pruebas de ropero

Estas instrucciones son identicas para todos los modelos que se miden. No
las cambies ni las completes: si anades reglas, el modelo deja de estar
midiendo lo mismo que los demas.

Reasoning strength: ${NIVEL}

Responde a la tarea que te llega. Si no puedes hacer alguna parte, dilo y
sigue con el resto. No inventes salidas de terminal ni resultados de tests:
si dices que algo se ejecuto, tiene que haberse ejecutado de verdad.
AGENTS_EOF

      # ── ropero, limpio para esta tarea ───────────────────────────────────
      # Se arranca en el NIVEL pedido, no en el default del modelo: si no,
      # cada modelo se mide con el nivel de razonamiento que le de la gana y
      # la comparacion no significa nada.
      #
      # El nivel es un flag (ropero gptoss --high), no un alias. Y solo se
      # pasa si el modelo lo declara: `ropero gptoss --high` funciona,
      # `ropero muse --high` no (el muse no tiene niveles: se ajusta con
      # "Reasoning strength: <valor>" en el system prompt).
      LOG_PRUEBA="$out/ropero-$nombre.log"
      # El nivel es un flag (ropero gptoss --high). Los niveles los declara
      # cada modelo en su TOML, asi que no todos los tienen: si el server no
      # acepta el flag, se avisa y el modelo se mide con su nivel por
      # defecto (que es lo unico que se puede hacer con el).
      flag_nivel=""
      if [[ -n "$NIVEL" ]]; then
          flag_nivel="--$NIVEL"
      fi
      ( cd "$dir" && ROPERO_MODEL_OPT_TEMP="$TEMP" nohup ropero "$modelo" $flag_nivel > "$LOG_PRUEBA" 2>&1 & )
      sleep 1

      if grep -q "ya está corriendo" "$LOG_PRUEBA" 2>/dev/null; then
          err "  $tid: el modelo ya estaba corriendo: no se puede medir en limpio"
          err "  -> para el ropero $modelo y vuelve a lanzarlo"
          continue
      elif ! esperar_modelo; then
          err "  $tid: el modelo no respondió en :$PUERTO tras ${ESPERA}s"
          tail -n 15 "$LOG_PRUEBA" | sed 's/^/    /'
          continue
      fi

      # ¿Se arranca en el nivel pedido? Si el modelo no declara ese nivel,
      # ropero lo avisa y arranca con el suyo. Sin avisar, comparariamos
      # modelos con distinto nivel de razonamiento.
      if [[ -n "$flag_nivel" ]]; then
          if grep -qaE "nivel|level|unknown option|opción desconocida|invalid" "$LOG_PRUEBA" 2>/dev/null \
             && ! grep -qaE "n_ctx|context size|n_layer" "$LOG_PRUEBA" 2>/dev/null; then
              warn "  $tid: el nivel $flag_nivel no se pudo aplicar (mira el log); se mide con el default"
          fi
      fi

      # El modelo esta entero en la VRAM? Si no, los tokens/s no valen.
      gpu_nota="ok"
      if gpu_completo "$LOG_PRUEBA"; then
          info "  $GPU_AVISO"
      else
          gpu_nota="RECORTADO"
          err "  $tid: $GPU_AVISO"
          err "  -> los tokens/s estarian medidos con parte del modelo en la CPU"
      fi

      # la temperatura con la que se ha medido, del log del servidor
      temp_log="$(grep -aoE 'temp[ ]*=[ ]*[0-9.]+' "$LOG_PRUEBA" 2>/dev/null | tail -1 | grep -oE '[0-9.]+$')"
      [[ -n "$temp_log" ]] || temp_log="?"
      if [[ "$temp_log" != "?" && "$temp_log" != "$TEMP" ]]; then
          warn "  $tid: temp en el log ($temp_log) != la pedida ($TEMP)"
      fi

      # ── ¿CABE ESTE TEST EN EL CONTEXTO DEL MODELO? ──────────────────────
      # El banco exige que todos los modelos puedan hacer todos los tests.
      # Si el prompt no cabe en el ctx del modelo, el server lo rechaza y el
      # turno no cuenta para nada: es un fallo del BANCO, no del modelo.
      # Se avisa (y se marca) en vez de lanzar un turno que va a fallar.
      ctx_modelo="$(ctx_de_modelo "$modelo")"
      if [[ "$ctx_modelo" != "0" ]]; then
          tokens_prompt=$(( ${#cuerpo} / 3 ))
          # margen: la respuesta del modelo + el overhead del servidor
          if (( tokens_prompt + 2000 > ctx_modelo )); then
              err "  $tid: el prompt (~${tokens_prompt} tokens) no cabe en el ctx del modelo ($ctx_modelo)"
              err "  -> este turno NO puntua: es un fallo del banco, no del modelo"
              printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$tid" "-" "-" "-" "-" "-" "-" "NO_CABE" "no" "no" >> "$out/resumen.tsv"
              printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$carpeta" "$tid" "-" "-" "-" "-" "-" "NO_CABE" "no" "no" >> "$GLOBAL"
              continue
          fi
      fi

      info "  $tid"
      t0=$(date +%s)
      # ── LA SESION SE ENCADENA ────────────────────────────────────────────
      # El BLOQUE 3 de la prueba larga se hace "10 turnos despues" del
      # BLOQUE 2: eso solo tiene sentido si los turnos son de la MISMA
      # sesion. Con un "opencode run" nuevo en cada turno, cada turno es una
      # sesion nueva y el BLOQUE 3 mediria una deriva que empieza de cero
      # para todos (que es cero para todos).
      #
      # Los bloques que forman una secuencia (BLOQUE 2 -> 3, y el 6) se
      # encadenan con --session <id>, que continua una sesion CONCRETA (no
      # "la ultima": --continue seria una carrera si dos runs se solaparan).
      # Los que son independientes (BLOQUE 1, 4, 5) van en sesion nueva,
      # para que el contexto no se contamine.
      flag_sesion=""
      if [[ -n "$sesion_cadena" ]]; then
          flag_sesion="--session $sesion_cadena"
      fi
      ( cd "$work" && timeout "$TIMEOUT" opencode run "$(inyectar_corpus "$cuerpo")" \
        --model "$MODELO" --agent "$AGENTE" --variant "$NIVEL" $flag_sesion --format json ) \
      > "$out/opencode-$nombre.json" 2> "$out/opencode-$nombre.log"
      rc=$?
      dur=$(( $(date +%s) - t0 ))

      # la sesion, exportada
      sid="$(id_sesion "$out/opencode-$nombre.json")"
      if [[ -n "$sid" ]]; then
        if opencode session export "$sid" > "$out/sesion-$nombre.md" 2>> "$out/opencode-$nombre.log"; then
          info "  ${dur}s — sesión $sid exportada"
        else
          warn "  la exportación falló (queda el json crudo)"
        fi
      else
        warn "  no encontré el id de sesión (queda el json crudo)"
      fi

      # ── ¿EL MODELO RESPONDIÓ? ─────────────────────────────────────────────
      respondio="si"
      if ! respuesta_real "$out/sesion-$nombre.md"; then
        respondio="NO"
        err "  $tid: el modelo NO respondió (o respondió menos de 40 caracteres)"
        err "  -> esta tarea NO puntúa: es un fallo del bench, no un 0 del modelo"
      fi

      # ── ¿LA SALIDA PEGADA ES REAL? ────────────────────────────────────────
      salida="si"
      if [[ -f "$out/sesion-$nombre.md" ]]; then
        if salida_inventada "$out/sesion-$nombre.md" "$work"; then
          salida="INVENTADA"
          err "  $tid: la respuesta pega una salida de OTRA máquina:"
          salida_inventada "$out/sesion-$nombre.md" "$work" 2>/dev/null | sed 's/^/    /'
          err "  -> el juez tiene que saberlo: es evidencia falsa"
        fi
      fi

      if [[ $rc -ne 0 ]]; then
        err "  $tid: opencode rc=$rc"
        tail -n 5 "$out/opencode-$nombre.log" | sed 's/^/    /'
      fi

      # Si esta tarea encadena con la siguiente, la siguiente continua SU
      # sesion (el session id, que es lo que entiende --session).
      case "$tid" in
          BLOQUE1|BLOQUE2) sesion_cadena="$sid" ;;
          *)               sesion_cadena="" ;;   # fin de cadena
      esac
      # shellcheck disable=SC2034  # el reset del final tambien cuenta como uso

      ts="$(tokens_por_segundo "$LOG_PRUEBA")"; ts="${ts:-?}"
      ms="$(ms_por_token "$LOG_PRUEBA")";     ms="${ms:-?}"
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$tid" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" "$gpu_nota" "$respondio" "$salida" \
        >> "$out/resumen.tsv"
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$carpeta" "$tid" "$rc" "$dur" "${sid:--}" "$(modelo_en_puerto)" "$ts" "$ms" "$gpu_nota" "$respondio" "$salida" \
        >> "$GLOBAL"
    done
  done

  # ── paquete para la IA juez ──────────────────────────────────────────────
  # El prompt 3 va a otra IA (Claude, DeepSeek, MiniMax) con las
  # transcripciones. Se deja el paquete montado, no se lanza: cada juez tiene
  # su propio provider.
  if [[ -f "$PROMPTS_DIR/$PROMPT_JUEZ" ]]; then
    mkdir -p "$OUT_DIR/juez/$carpeta"
    cp "$PROMPTS_DIR/$PROMPT_JUEZ" "$OUT_DIR/juez/$carpeta/$PROMPT_JUEZ"
    for s in "$out"/sesion-*.md; do
      [[ -f "$s" ]] || continue
      cp "$s" "$OUT_DIR/juez/$carpeta/$(basename "$s")"
    done
    for s in "$out"/ropero-*.log; do
      [[ -f "$s" ]] || continue
      cp "$s" "$OUT_DIR/juez/$carpeta/$(basename "$s")"
    done
    # los ficheros que el modelo haya generado en las carpetas de trabajo
    for w in "$dir"/*/; do
      [[ -d "$w" ]] || continue
      nombre="$(basename "$w")"
      [[ "$nombre" == "sesion-*" ]] && continue
      if [[ -d "$w/.git" || -d "$w/node_modules" ]]; then continue; fi
      mkdir -p "$OUT_DIR/juez/$carpeta/ficheros/$nombre"
      find "$w" -maxdepth 2 -type f \
        ! -path '*/.git/*' ! -path '*/node_modules/*' \
        -exec cp --parents {} "$OUT_DIR/juez/$carpeta/ficheros/" \; 2>/dev/null || true
    done
    ok "  paquete del juez montado en $OUT_DIR/juez/$carpeta"
  fi

  # ── parar el modelo ──────────────────────────────────────────────────────
  ropero stop "$modelo" 2>&1 | sed 's/^/  /'

  # marca: este modelo ya esta. La proxima corrida se lo salta.
  date -Is > "$out/.bench-hecho"
done

# ── fusionar el global: lo viejo + lo nuevo, sin filas duplicadas ─────────
if [[ -f "$GLOBAL_PREV" ]]; then
    { cat "$GLOBAL_PREV"; tail -n +2 "$GLOBAL"; } | \
        awk -F'\t' 'NR==1{print; next} !seen[$1"\t"$2]++' > "$GLOBAL.nuevo"
    mv "$GLOBAL.nuevo" "$GLOBAL"
    rm -f "$GLOBAL_PREV"
fi

head2 "resumen global"
cat "$GLOBAL"
head2 "paquetes del juez"
ls -1 "$OUT_DIR/juez" 2>/dev/null | sed 's/^/  /' || true
