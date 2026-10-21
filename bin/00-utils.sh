#!/usr/bin/env bash
# =============================================================================
# 00-utils.sh — réplica mínima de setup 00-utils.sh para ropero (spec migración)
# Solo lo que ropero necesita: logging a stderr, prompts, detect_os.
# Seguro de sourcear (solo definiciones, cero ejecución).
# =============================================================================

_ropero_log() { printf '%s\n' "$*" >&2; }
ropero_info() { _ropero_log "  [i] $*"; }

# Contadores del contrato. Se incrementan en los mismos helpers que ya usaba
# todo ropero, así que `--install` y `--check` traen su recuento sin tocar las
# decenas de llamadas que hay repartidas por el proyecto. `ropero_counters_reset`
# los pone a cero al entrar en cada comando del contrato; fuera de ellos los
# valores no se leen y no molestan.
ROPERO_OK=0; ROPERO_WARN=0; ROPERO_FAIL=0
ropero_ok()   { _ropero_log "  [✓] $*"; ROPERO_OK=$((ROPERO_OK+1)); }
ropero_warn() { _ropero_log "  [!] $*"; ROPERO_WARN=$((ROPERO_WARN+1)); }
ropero_err()  { _ropero_log "  [✗] $*"; ROPERO_FAIL=$((ROPERO_FAIL+1)); }

ropero_counters_reset() { ROPERO_OK=0; ROPERO_WARN=0; ROPERO_FAIL=0; }

# Numeración de fases. Se cuenta en vez de escribirse a mano porque las rutas
# de --install tienen distinto número de fases (all=4, engines=3, models=3,
# link=3) y a mano la lista salía como [1/5] ... [2/5] ... [5/5], saltando el
# 3 y el 4. Un "[5/5] Resumen" después de un "[2/5]" hace pensar que se perdió
# trabajo, y no se perdió: es que no Applicable.
ROPERO_STEP_N=0
ROPERO_STEP_TOTAL=0
ropero_phases_begin() { ROPERO_STEP_N=0; ROPERO_STEP_TOTAL="$1"; }
ropero_phase() {
    ROPERO_STEP_N=$((ROPERO_STEP_N+1))
    # Log directo y no ropero_step: ese ya pone sus propios corchetes, y
    # pasarle "[1/3]" ya formateado salía como [[1/3]].
    _ropero_log ""; _ropero_log "[$ROPERO_STEP_N/$ROPERO_STEP_TOTAL] $1"
}
ropero_step()  { _ropero_log ""; _ropero_log "[$1] $2"; }
ropero_counters() { _ropero_log "  $ROPERO_OK ok · $ROPERO_WARN avisos · $ROPERO_FAIL fallos"; }

# respeta CI / AUTO_YES / --full desatendido (igual que setup ask_*)
# LECTURA DIRECTA de AUTO_YES / SETUP_MODE / CI sin pasar por la lógica
# original. Si cualquiera está activo, no preguntamos. El entrypoint ropero
# también parchó esto, pero lo hacemos aquí en la fuente para que NO HAYA
# FORMA de que un parche posterior se pierda.
ropero_ask() {
    if [[ "${AUTO_YES:-0}" == "1" || "${SETUP_MODE:-}" == "full" || "${CI:-0}" == "1" ]]; then
        return 0
    fi
    if [[ ! -t 0 ]]; then
        return 0
    fi
    local prompt="$1"
    local ans
    printf '%s [S/n]: ' "$prompt" >&2
    read -r ans || ans="s"
    ans="$(printf '%s' "$ans" | tr '[:upper:]' '[:lower:]')"
    [[ -z "$ans" || "$ans" == "s" || "$ans" == "y" ]]
}

# destructivo (rm -rf, reclone): convención setup ask_if_reinstall —
# en desatendido CONSERVA (no), en interactivo default N. Nunca auto-yes.
ropero_ask_keep() {
    if [[ "${AUTO_YES:-0}" == "1" || "${SETUP_MODE:-}" == "full" || "${CI:-0}" == "1" ]]; then
        return 1
    fi
    if [[ ! -t 0 ]]; then
        return 1
    fi
    local prompt="$1"
    local ans
    printf '%s (s/N): ' "$prompt" >&2
    read -r ans || ans="n"
    ans="$(printf '%s' "$ans" | tr '[:upper:]' '[:lower:]')"
    [[ "$ans" == "s" || "$ans" == "y" ]]
}

ropero_detect_os() {
    if [[ -n "${PREFIX:-}" && -d "${PREFIX:-}" ]] && command -v pkg &>/dev/null; then echo "termux"; return 0; fi
    if [[ "$(uname -s)" == "Darwin" ]]; then echo "macos"; return 0; fi
    # WSL2 ANTES de cachy: una WSL2 cachyoseada sigue siendo wsl2 (sin paquetes
    # gaming/GPU bare metal). Detectamos por /proc/sys/kernel/osrelease con
    # "microsoft" o "WSL" en el kernel string — Microsoft usa "microsoft-standard-WSL2"
    # en kernels recientes.
    if grep -qiE 'microsoft|wsl' /proc/sys/kernel/osrelease 2>/dev/null \
       || [[ -f /run/WSL1 ]] || [[ -f /run/WSL2 ]] \
       || [[ -f /proc/version ]] && grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null; then
        echo "wsl2"; return 0
    fi
    if [[ -f /etc/cachyos-release ]] || grep -qi cachyos /etc/os-release 2>/dev/null; then echo "cachy"; return 0; fi
    if [[ -f /etc/arch-release ]]; then echo "arch"; return 0; fi
    echo "unknown"
}

# Descarga GGUF idempotente, con check individual (omite si ya existe).
#
#   ropero_download_gguf <repo> <filename> <name> [size] [dest_dir] [dest_name]
#                              ^^^^^^^  ^^^^^^^^  ^^^^  ^^^^  ^^^^^^^^  ^^^^^^^^^
#                              HF repo  fichero   nombre tamaño ruta     nombre
#                                       en el     legible        destino final
#                                       repo
#
#   dest_dir   dónde acaba el modelo. Por defecto $GGUF_DIR. Acepta "~".
#   dest_name  nombre final del fichero. Por defecto <filename>. Es lo que
#              permite renombrar al descargar: "mmproj-F16.gguf" choca entre
#              repos y cada modelo quiere el suyo.
#
# Las llamadas reales están en bin/21-download-models.sh.
#
# IMPORTANTE: bajo `set -u` (entrypoint ropero lo tiene), `local var=$1` con
# $1 vacío falla con "var: unbound variable" en algunas versiones de bash.
# Por eso validamos primero que los args requeridos no estén vacíos, y
# usamos asignaciones que absorben el unset (`:="${var:-default}"`).
ropero_download_gguf() {
    local repo="${1:-}" filename="${2:-}" name="${3:-}" size="${4:-}"
    local dest_dir="${5:-}" dest_name="${6:-}"

    if [[ -z "$repo" || -z "$filename" || -z "$name" ]]; then
        ropero_err "ropero_download_gguf: argumentos incompletos (repo='$repo' filename='$filename' name='$name')"
        return 1
    fi

    # ── Validación de los argumentos opcionales ─────────────────────────────
    # El bug de siempre: los args 4º/5º se escribían a ojo, así que una llamada
    # que se saltara el `name` acababa con el TAMAÑO en `name` y la RUTA en
    # `size`. Entonces dest_dir recibía el nombre del fichero de destino
    # ("Qwen2.5-VL-mmproj.gguf"), y el resultado era una CARPETA con ese nombre
    # conteniendo el gguf, en el cwd. Parecía funcionar (el fichero estaba ahí)
    # pero en el sitio equivocado, y el renombrado nunca ocurría porque
    # dest_name se quedaba vacío.
    #
    # Se detecta por forma: un tamaño tiene unidades o un punto ("1.6 GB",
    # "900MB"), una ruta no. Se normaliza al esquema correcto en vez de fallar,
    # que es lo que quiere quien llama: los 6 argumentos son fijos y el que se
    # olvidó no va a beneficiarse de un error.
    #
    # Casos que cubre:
    #   (... repo file name SIZE PATH NAME)      → 6 args, bien
    #   (... repo file name SIZE NAME)           → 5 args: falta dest_dir
    #   (... repo file name PATH NAME)           → 5 args: falta size
    if [[ -n "$dest_dir" && -z "$dest_name" ]]; then
        # 5 args: el 5º es el nombre final (o un tamaño mal colocado), no un
        # directorio. El destino es el de por defecto.
        dest_name="$dest_dir"
        dest_dir=""
    fi

    [[ -n "$dest_dir" ]] || dest_dir="${GGUF_DIR:-$HOME/models/gguf}"
    [[ -n "$dest_name" ]] || dest_name="$filename"

    # ── Expandir ~ ──────────────────────────────────────────────────────────
    # Los llamadores pasan "~/models/gguf" con comillas, y dentro de comillas
    # bash NO expande ~. Sin esto, mkdir -p crea un directorio LITERAL llamado
    # "~" en el cwd y la descarga se va al sitio equivocado. (hf download sí
    # expandía su --local-dir, pero el `mv` de Rename no: de ahí que el fichero
    # acabara descargado pero sin renombrar.)
    case "$dest_dir" in
        "~")     dest_dir="$HOME" ;;
        "~/"*)   dest_dir="$HOME/${dest_dir#\~/}" ;;
    esac

    # ── Nombre final, sin directorios ───────────────────────────────────────
    # dest_name es un NOMBRE de fichero. Si alguien pasa una ruta con /
    # (ej. "sub/mi-modelo.gguf"), lo respetamos creando el subdirectorio, para
    # que "MTP/x.gguf" en dest_name no crees una carpeta MTP sorpresa.
    case "$dest_name" in
        */*) dest_dir="$dest_dir/$(dirname "$dest_name")"
             dest_name="$(basename "$dest_name")" ;;
    esac

    local final="$dest_dir/$dest_name"

    # ── ¿Ya está? ──────────────────────────────────────────────────────────
    # Se mira SIEMPRE el nombre final primero. Con rename, el fichero que
    # descarga hf es el original y el que nos interesa es el renombrado; si
    # solo mirásemos el original, un rename ya aplicado se redescargaría cada
    # vez (y con `~` sin expandir, la comprobación nunca encontraba nada).
    if [[ -f "$final" && -s "$final" ]]; then
        # --force: borra y vuelve a bajar. Antes `--force` solo ponía AUTO_YES,
        # así que la pregunta desaparecía pero el "ya descargado" cortaba
        # igualmente: pedías forzar y no pasaba nada, en silencio.
        if [[ "${ROPERO_FORCE:-0}" == "1" ]]; then
            ropero_info "--force: borrando $final"
            rm -f "$final" || { ropero_err "no se pudo borrar $final"; return 1; }
        else
            ropero_ok "$dest_name ya descargado"; return 0
        fi
    fi
    if [[ "$dest_name" != "$filename" ]]; then
        # `filename` puede traer subdirectorio del repo ("MTP/x.gguf"), así que
        # el fichero original NO está en la raíz de dest_dir.
        local orig="$dest_dir/$filename"
        if [[ -f "$orig" && -s "$orig" ]]; then
            # Descargado pero sin renombrar (una versión anterior falló aquí):
            # el fichero está, solo falta el rename: se aplica y listo.
            if mv "$orig" "$final" 2>/dev/null; then
                ropero_ok "$dest_name renombrado (ya estaba descargado)"
                return 0
            fi
        fi
    fi
    if ! ropero_ask "Descargar $name (~$size)? Necesario para ropero"; then
        ropero_info "$name omitido por el usuario"; return 0
    fi

    mkdir -p "$dest_dir" || { ropero_err "no se pudo crear $dest_dir"; return 1; }

    # ── Descarga ────────────────────────────────────────────────────────────
    # `filename` puede traer subdirectorios en el repo ("MTP/x.gguf"). hf los
    # reproduce bajo dest_dir, así que hay que buscarlos ahí, no en la raíz.
    #
    # QUÉ DESCARGADOR SE USA
    #   auto (por defecto) → hf si está en el PATH, curl si no.
    #   hf                  → obliga a usar hf (error si no está).
    #   curl                → obliga a usar curl (error si no está).
    #
    # El override existe por dos motivos: hay máquinas donde el `hf` está
    # instalado pero roto (o es una versión antigua que no habla con el repo),
    # y los tests necesitan fijar el camino para no depender de lo que haya
    # instalado en la máquina donde corren.
    local dl="${ROPERO_DOWNLOADER:-auto}" use_hf=0
    case "$dl" in
        auto) command -v hf &>/dev/null && use_hf=1 ;;
        hf)   command -v hf &>/dev/null || { ropero_err "ROPERO_DOWNLOADER=hf pero no hay 'hf' en el PATH"; return 1; }
              use_hf=1 ;;
        curl) use_hf=0 ;;
        *)    ropero_err "ROPERO_DOWNLOADER inválido: '$dl' (valores: auto, hf, curl)"; return 1 ;;
    esac

    local staged_dir="$dest_dir"
    if (( use_hf == 1 )); then
        hf download "$repo" "$filename" --local-dir "$staged_dir" \
            || { ropero_err "falló $name"; return 1; }
        local downloaded="$staged_dir/$filename"
        # hf a veces normaliza el nombre; si no está donde lo esperamos,
        # buscamos por basename dentro de lo recién descargado.
        if [[ ! -f "$downloaded" ]]; then
            downloaded="$(find "$staged_dir" -type f -name "$(basename "$filename")" -print -quit 2>/dev/null || true)"
        fi
    else
        # Sin hf: se descarga directo al nombre final. No hay staging ni rename
        # porque curl no crea la subestructura del repo.
        local dest="$final"
        mkdir -p "$(dirname "$dest")"
        curl -fL "https://huggingface.co/$repo/resolve/main/$filename" -o "$dest" \
            || { ropero_err "falló $name"; return 1; }
        ropero_ok "$name descargado"
        return 0
    fi

    if [[ -z "$downloaded" || ! -f "$downloaded" ]]; then
        ropero_err "falló $name (hf dijo OK pero no aparece $filename)"
        return 1
    fi

    # ── Renombrado ──────────────────────────────────────────────────────────
    # Se comparan AMBAS rutas ya normalizadas a absoluta. Con comparación
    # directa, `mv X X` no es un no-op limpio: si las cadenas difieren (una
    # relativa y otra absoluta, un "~" sin expandir) el rename se dispara y
    # puede fallar o, peor, dejar el fichero a medias.
    local abs_downloaded abs_final
    abs_downloaded="$(cd "$(dirname "$downloaded")" && pwd)/$(basename "$downloaded")"
    abs_final="$(cd "$(dirname "$final")" 2>/dev/null && pwd)/$(basename "$final")" || abs_final="$final"

    if [[ "$abs_downloaded" != "$abs_final" ]]; then
        mkdir -p "$(dirname "$abs_final")" 2>/dev/null || true
        if mv "$abs_downloaded" "$abs_final"; then
            ropero_ok "$name → $dest_name"
        else
            ropero_err "no se pudo renombrar $abs_downloaded a $abs_final"
            return 1
        fi
    else
        ropero_ok "$name descargado"
    fi
    return 0
}


# _ropero_dir_has_weights <dir> → 0 si hay pesos de CUALQUIER formato que un
# engine sepa cargar.
#
# Why no basta con "*.safetensors": porque hay checkpoints perfectamente válidos
# que solo traen pytorch_model.bin. Comprobado con
# hf-internal-testing/tiny-random-GPTNeoXForCausalLM: su único fichero de pesos
# es pytorch_model.bin, sin un solo safetensors. Con el chequeo restringido a
# safetensors, la función NUNCA daba el modelo por descargado, no tocaba el
# marcador y cada `ropero --install` volvía a bajar los mismos 12 GB. Un bug de
# idempotencia que solo se ve probando contra un repo de verdad.
#
# Los tres formatos:
#   *.safetensors     lo normal (HF moderno, y lo que usa mlx_lm)
#   pytorch_model*.bin checkpoints antiguos o de publishers que no migraron
#   *.npz             conversiones MLX muy viejas, antes de safetensors
#
# Un shard suelto NO cuenta: un repo de 3 shards con solo el primero descargado
# (interrumpido) tiene pesos y no es utilizable. Por eso se exige el índice
# cuando hay más de un shard, igual que hace transformers al cargar.
_ropero_dir_has_weights() {
    local dir="${1:-}"
    [[ -d "$dir" ]] || return 1

    local n=0 f
    for f in "$dir"/*.safetensors "$dir"/pytorch_model*.bin "$dir"/*.npz; do
        [[ -f "$f" ]] && n=$((n + 1))
    done
    (( n > 0 )) || return 1

    # Si hay varios shards y además hay un índice, el índice tiene que estar.
    # Sin él, transformers no sabe qué shard va con qué y falla al cargar.
    if (( n > 1 )); then
        if [[ -f "$dir/model.safetensors.index.json" || -f "$dir/pytorch_model.bin.index.json" ]]; then
            return 0
        fi
        # Sin ningún índice: solo es válido si hay un único fichero de pesos.
        (( n == 1 ))
        return $?
    fi
    return 0
}

# ropero_download_safetensors <repo> <modelname> <size> [dest_dir] [excludes]
#                                   ^^^^^^^  ^^^^^^^^^  ^^^^  ^^^^^^^^^  ^^^^^^^^^
#                                   HF repo  nombre    tam. ruta        globs extra
#                                           legible    destino   a excluir
#
# Descarga un checkpoint COMPLETO de HuggingFace (safetensors + config +
# tokenizer) a un directorio, que es lo que necesitan los engines con servidor
# propio: mlx_lm, airllm, tensorrt-llm.
#
# ── IDEMPOTENCIA POR CONTENIDO REAL, NO POR UN MARCADOR ─────────────────────
# La versión anterior decidía con un fichero `.download_complete` que se tocaba
# al final. Eso miente en los dos sentidos:
#   · Si borras un shard, el marcador sigue ahí y dice "ya descargado" para
#     siempre. El modelo no arranca y el `--install` no vuelve a intentarlo.
#   · Si el directorio se creó por otra vía (un `hf download` a mano, un symlink,
#     un montaje), no hay marcador y descarga 12 GB encima de 12 GB.
# Ahora la pregunta es "¿hay pesos ahí?", y se responde contando ficheros:
# al menos un *.safetensors y un config.json. El marcador se conserva, pero como
# optimisation, no como fuente de verdad: si existe y el contenido está bien, se
# refresca; si el marcador dice que sí y el contenido dice que no, gana el
# contenido y se vuelve a descargar.
#
# ── POR QUÉ `hf` Y NO `curl` ─────────────────────────────────────────────────
# Los checkpoints son 3-15 shards con un `model.safetensors.index.json` que dice
# cuál va con cuál, y las URLs de HF caducan. `hf download` resuelve el
# índice, reanuda una descarga a medias y verifica etags. Con curl habría que
# replicar las tres cosas y fallar en silencio en cuanto el repo se reescriba.
#
# ── `excludes` ───────────────────────────────────────────────────────────────
# Globs relativos a la raíz del repo, separados por espacio. Se suman a una
# lista fija de basura. Existe por un motivo concreto: hay checkpoints que
# duplican los pesos dentro del repo (`openai/gpt-oss-120b` trae `original/` en
# bf16 junto a la versión cuantizada). Sin excluir eso descargas el doble de
# disco para nada. Por eso el default NO excluye `original/*` —sería magia— y
# quien lo sepa, lo pasa explícitamente.
ropero_download_safetensors() {
    local repo="${1:-}" modelname="${2:-}" size="${3:-}"
    local dest_dir="${4:-}" excludes="${5:-}"

    if [[ -z "$repo" || -z "$modelname" ]]; then
        ropero_err "ropero_download_safetensors: argumentos incompletos (repo='$repo' modelname='$modelname')"
        return 1
    fi

    # ── Destino ───────────────────────────────────────────────────────────
    [[ -n "$dest_dir" ]] || dest_dir="${SAFETENSORS_DIR:-$HOME/models/safetensors}/$modelname"
    # `~` sin expandir crearía un directorio LITERAL llamado "~" en el cwd. Los
    # llamadores pasan las rutas entre comillas, y bash NO expande ~ ahí dentro.
    case "$dest_dir" in
        "~")     dest_dir="$HOME" ;;
        "~/"*)   dest_dir="$HOME/${dest_dir#\~/}" ;;
    esac

    # ── ¿Está ya? ─────────────────────────────────────────────────────────
    # Lo que tiene que haber para que un engine pueda cargarlo: pesos y config.
    # El tokenizer solo se avisa: hay checkpoints que lo traen embebido en
    # tokenizer_config.json y no tienen tokenizer.json suelto, así que exigirlo
    # daría un falso negativo en modelos perfectamente buenos.
    local have_weights=0 have_config=0
    if [[ -d "$dest_dir" ]]; then
        _ropero_dir_has_weights "$dest_dir" && have_weights=1
        [[ -f "$dest_dir/config.json" ]] && have_config=1
    fi

    if (( have_weights == 1 && have_config == 1 )); then
        # --force borra el checkpoint entero y lo vuelve a bajar (mismo motivo
        # que en el GGUF: sin esto, forzar no re-descargaba nada).
        if [[ "${ROPERO_FORCE:-0}" == "1" ]]; then
            ropero_info "--force: borrando $dest_dir"
            rm -rf "$dest_dir" || { ropero_err "no se pudo borrar $dest_dir"; return 1; }
            have_weights=0; have_config=0
        else
            # Refrescar el marcador si falta, para que las versiones viejas del
            # repo (que solo miraban el marcador) sigan funcionando.
            [[ -f "$dest_dir/.download_complete" ]] || touch "$dest_dir/.download_complete" 2>/dev/null || true
            local wmb
            wmb=$(du -sm "$dest_dir" 2>/dev/null | awk '{print $1; exit}')
            ropero_ok "$modelname ya descargado (${wmb:-?} MB en $dest_dir)"
            return 0
        fi
    fi

    # Hay marcador pero no pesos: alguien borró los shards, o una descarga
    # anterior se quedó a medias y llegó a tocar el marcador. Hay que decirlo,
    # porque si no parece un bug de ropero cuando es un directorio roto.
    if [[ -f "$dest_dir/.download_complete" ]] && (( have_weights == 0 )); then
        ropero_warn "$modelname: el directorio $dest_dir dice estar descargado pero no tiene pesos."
        ropero_warn "  Se vuelve a descargar (los shards que quedaran se sobrescriben)."
        rm -f "$dest_dir/.download_complete" 2>/dev/null || true
    fi

    # ── hf ANTES de preguntar ──────────────────────────────────────────────
    # Preguntar "¿descargo 12 GB?" y luego decir que no se puede es la peor
    # secuencia posible: el usuario contesta sí y no entiende el fallo.
    #
    # Un checkpoint de safetensors NO se puede bajar con curl: son varios
    # shards más un índice, y el índice es lo que dice cuál va con cuál.
    if [[ "${ROPERO_DOWNLOADER:-auto}" == "curl" ]]; then
        ropero_err "ROPERO_DOWNLOADER=curl pero $modelname es un checkpoint de safetensors:"
        ropero_info "  los checkpoints con shards solo se pueden bajar con 'hf' (curl no resuelve el índice)"
        return 1
    fi
    if ! command -v hf >/dev/null 2>&1; then
        ropero_err "no se puede descargar $modelname: falta 'hf' (huggingface_hub)"
        ropero_info "  instalarlo:  pip install -U 'huggingface_hub[cli]'"
        ropero_info "  o descarga manual y ponlo en: $dest_dir"
        return 1
    fi

    if ! ropero_ask "Descargar $modelname (~$size) en $dest_dir?"; then
        ropero_info "$modelname omitido por el usuario"
        return 0
    fi

    mkdir -p "$dest_dir" || { ropero_err "no se pudo crear $dest_dir"; return 1; }

    # ── Descarga ──────────────────────────────────────────────────────────
    # Se acumulan los excludes: los de basura, siempre, y los que pase el
    # llamador. `hf download` es reanudable, así que si esto se corta a mitad
    # el siguiente `--install` continúa donde lo dejó en vez de rehacer 12 GB.
    local -a hf_args=(download "$repo" --local-dir "$dest_dir")
    local e
    for e in ".gitattributes" "*.md" ".DS_Store" "*.png" "*.jpg" "*.gif" $excludes; do
        [[ -n "$e" ]] || continue
        hf_args+=(--exclude "$e")
    done

    ropero_info "  hf ${hf_args[*]}"
    # Progreso por stderr: hf escribe mucho y si se fuera a stdout, --install
    # parecería escupir basura en vez de modelo.
    if ! hf "${hf_args[@]}" >&2; then
        ropero_err "falló la descarga de $modelname (hf)"
        ropero_info "  hf reanuda solo: vuelve a ejecutar el install y continúa."
        ropero_info "  para ir más rápido:  pip install hf_transfer"
        ropero_info "  y luego  HF_HUB_ENABLE_HF_TRANSFER=1 ropero --install"
        return 1
    fi

    # ── Verificación ──────────────────────────────────────────────────────
    # hf puede salir con 0 y dejar el directorio a medias (fallo de red tratado
    # como fin, o un repo sin pesos). El marcador solo se toca si los pesos
    # están de verdad: es lo que hace que la idempotencia sea de fiar.
    have_weights=0 have_config=0
    _ropero_dir_has_weights "$dest_dir" && have_weights=1
    [[ -f "$dest_dir/config.json" ]] && have_config=1

    if (( have_weights == 0 || have_config == 0 )); then
        ropero_err "hf terminó pero $dest_dir está incompleto"
        [[ $have_weights -eq 0 ]] && ropero_info "  faltan los pesos (*.safetensors / pytorch_model*.bin / *.npz)"
        [[ $have_config -eq 0 ]] && ropero_info "  falta config.json (¿repo equivocado?)"
        rm -f "$dest_dir/.download_complete" 2>/dev/null || true
        return 1
    fi

    touch "$dest_dir/.download_complete" 2>/dev/null || true
    local wmb
    wmb=$(du -sm "$dest_dir" 2>/dev/null | awk '{print $1; exit}')
    # En MB mientras sea pequeño: decir "0 GB" de un modelo de 15 MB parece un
    # error, y el redondeo a GB solo tiene sentido a partir del gigabyte.
    if (( ${wmb:-0} >= 1024 )); then
        ropero_ok "$modelname descargado ($(( ${wmb:-0} / 1024 )) GB en $dest_dir)"
    else
        ropero_ok "$modelname descargado (${wmb:-?} MB en $dest_dir)"
    fi
    return 0
}

# -----------------------------------------------------------------------------
# Resolver herramientas de build ignorando asdf shims (bypass homebrew/system)
# Réplica del setup_resolve_build_tool de setup.d/lib/common.sh para que ropero
# sea standalone (no depender del sistema setup). Mismo orden de búsqueda:
# /opt/homebrew/bin (Apple Silicon brew), /usr/local/bin (Intel brew), /usr/bin,
# y como fallback `command -v`. Lo usa 20-compile-llama.sh en la rama macOS
# para evitar que cmake/ninja caigan en un shim de asdf sin .tool-versions.
# -----------------------------------------------------------------------------
setup_resolve_build_tool() {
    local tool="$1"
    [[ -n "$tool" ]] || { ropero_err "setup_resolve_build_tool: nombre de tool vacío"; return 1; }

    for d in /opt/homebrew/bin /usr/local/bin /usr/bin; do
        if [[ -x "$d/$tool" ]]; then
            echo "$d/$tool"
            return 0
        fi
    done

    local found
    found=$(command -v "$tool" 2>/dev/null) || {
        ropero_err "setup_resolve_build_tool: '$tool' no encontrado (ni en /opt/homebrew/bin, /usr/local/bin, /usr/bin ni en PATH)"
        return 1
    }
    echo "$found"
    return 0
}
