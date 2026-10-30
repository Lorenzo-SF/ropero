#!/usr/bin/env bash
# =============================================================================
# 05-download.sh — descargas de artefactos: GGUFs, checkpoints y auxiliares
# (spec §5.6, fase 5)
# =============================================================================
# Porta ropero_download_gguf / ropero_download_safetensors /
# _ropero_dir_has_weights (la versión 0.x (historial git)) con tres
# decisiones 1.0:
#
#   · SOLO 'hf' como downloader. El fallback curl no resuelve shards ni
#     reanuda descargas, y ROPERO_DOWNLOADER era una vía de fuga de los
#     tests: una máquina con hf roto debe FALLAR, no degradarse en silencio.
#     Si falta hf: error con la receta (ropero install setup).
#   · El consentimiento lo da rp_confirm (RP_YES=1 lo salta). Lo fija
#     cmd-models al procesar una selección: ELEGIR un modelo ya es
#     consentir su descarga; la pregunta no se repite dentro del install.
#   · ROPERO_FORCE=1 fuerza el re-download (lo fija --force). Sin él, un
#     artefacto "ya descargado" corta en seco: es la idempotencia.
#
# La verificación de "ya está" es por CONTENIDO REAL, no por marcadores:
# contar ficheros de pesos. Un .download_complete sin pesos es un directorio
# roto, no un modelo instalado.
#
# bash 3.2: sin namerefs, sin arrays asociativos, sin mapfile (spec §3).
# Seguro de sourcear: sólo definiciones, cero ejecución.
# =============================================================================

# rp_dir_has_weights <dir> — 0 si hay pesos de CUALQUIER formato que un
# engine sepa cargar.
#
# Por qué no basta "*.safetensors": hay checkpoints perfectamente válidos
# que sólo traen pytorch_model.bin (verificado contra
# hf-internal-testing/tiny-random-GPTNeoXForCausalLM). Los tres formatos:
#   *.safetensors      lo normal (HF moderno, y lo que usa mlx)
#   pytorch_model*.bin checkpoints antiguos o de publishers que no migraron
#   *.npz              conversiones MLX muy viejas
#
# Un shard suelto NO cuenta: un repo de 3 shards con sólo el primero
# descargado (interrumpido) tiene pesos y no es utilizable. Con más de un
# shard se exige el índice, igual que hace transformers al cargar.
rp_dir_has_weights() {
    local dir="${1:-}"
    [[ -d "$dir" ]] || return 1

    local n=0 f
    for f in "$dir"/*.safetensors "$dir"/pytorch_model*.bin "$dir"/*.npz; do
        [[ -f "$f" ]] && n=$((n + 1))
    done
    (( n > 0 )) || return 1

    if (( n > 1 )); then
        if [[ -f "$dir/model.safetensors.index.json" || -f "$dir/pytorch_model.bin.index.json" ]]; then
            return 0
        fi
        (( n == 1 ))
        return $?
    fi
    return 0
}

# _rp_dl_expand_home <ruta> — expande "~"/"~/" a $HOME. Los llamadores pasan
# las rutas ENTRE COMILLAS y bash no expande ~ dentro de comillas: sin esto,
# mkdir -p crearía un directorio LITERAL llamado "~" en el cwd (bug
# histórico del legado, documentado en la versión 0.x (historial git)).
_rp_dl_expand_home() {
    # shellcheck disable=SC2088  # los "~" son LITERALES: son los patrones que detectan la tilde.
    case "$1" in
        "~")   printf '%s' "$HOME" ;;
        "~/"*) printf '%s' "$HOME/${1#\~/}" ;;
        *)     printf '%s' "$1" ;;
    esac
    return 0
}

# _rp_dl_require_hf <qué> — exit-fallo limpio si no hay hf en PATH.
_rp_dl_require_hf() {
    if command -v hf >/dev/null 2>&1; then
        return 0
    fi
    rp_msg error "no se puede descargar $1: falta 'hf' (huggingface_hub)"
    rp_msg info "  instalarlo:  ropero install setup"
    rp_msg info "  o a mano:    uv tool install 'huggingface_hub[cli]'"
    return 1
}

# rp_download_gguf <repo> <fichero> <nombre> <size> [dest_dir] [dest_name]
#
#   <repo>      owner/repo de HuggingFace
#   <fichero>   el fichero DENTRO del repo (puede traer subdir: "MTP/x.gguf")
#   <nombre>    nombre legible (para mensajes y el consentimiento)
#   <size>      tamaño informativo ("12.7 GB"); sólo para el mensaje
#   [dest_dir]  destino (default $GGUF_DIR)
#   [dest_name] nombre final (default <fichero>; el rename de hf)
#
# Descarga con hf a un STAGING propio (mktemp -d DENTRO de dest_dir: mismo
# filesystem, mv atómico) y mueve al nombre final. El staging evita que dos
# descargas con el mismo nombre de fichero en repos distintos (el clásico
# "mmproj-BF16.gguf") pisen el fichero del otro, y se lleva consigo el
# .cache/huggingface que hf deja en --local-dir: $GGUF_DIR queda limpio.
rp_download_gguf() {
    local repo="${1:-}" filename="${2:-}" nombre="${3:-}" size="${4:-}"
    local dest_dir="${5:-}" dest_name="${6:-}"

    if [[ -z "$repo" || -z "$filename" || -z "$nombre" ]]; then
        rp_msg error "rp_download_gguf: argumentos incompletos (repo='$repo' fichero='$filename' nombre='$nombre')"
        return 1
    fi

    [[ -n "$dest_dir" ]] || dest_dir="${GGUF_DIR:-$HOME/models/gguf}"
    [[ -n "$dest_name" ]] || dest_name="$filename"
    dest_dir="$(_rp_dl_expand_home "$dest_dir")"

    # dest_name es un NOMBRE de fichero; si trae ruta ("MTP/x.gguf"), se
    # respeta creando el subdirectorio.
    case "$dest_name" in
        */*)
            dest_dir="$dest_dir/$(dirname "$dest_name")"
            dest_name="$(basename "$dest_name")"
            ;;
    esac
    local final="$dest_dir/$dest_name"

    # ── ¿Ya está? Se mira SIEMPRE el nombre final primero: con rename, lo
    # que descarga hf es el original y lo que importa es el renombrado; si
    # sólo se mirara el original, un rename ya aplicado se re-descargaría.
    if [[ -f "$final" && -s "$final" ]]; then
        if [[ "${ROPERO_FORCE:-0}" == "1" ]]; then
            rp_msg info "--force: borrando $final"
            rm -f "$final" || { rp_msg error "no se pudo borrar $final"; return 1; }
        else
            rp_msg ok "$nombre ya descargado"
            return 0
        fi
    fi
    if [[ "$dest_name" != "$filename" ]]; then
        # Descargado pero sin renombrar (una versión anterior falló aquí):
        # el fichero está, sólo falta el rename.
        local orig="$dest_dir/$filename"
        if [[ -f "$orig" && -s "$orig" ]]; then
            if mv "$orig" "$final" 2>/dev/null; then
                rp_msg ok "$nombre renombrado (ya estaba descargado)"
                return 0
            fi
        fi
    fi

    # hf ANTES de preguntar: prometer una descarga y luego confesar que no
    # se puede es la peor secuencia posible.
    _rp_dl_require_hf "$nombre" || return 1

    if ! rp_confirm "¿Descargar $nombre (~${size:-?})?"; then
        rp_msg info "$nombre omitido por el usuario"
        return 0
    fi

    mkdir -p "$dest_dir" || { rp_msg error "no se pudo crear $dest_dir"; return 1; }

    local staging downloaded
    staging="$(mktemp -d "$dest_dir/.rp-dl.XXXXXX" 2>/dev/null)" || {
        rp_msg error "no se pudo crear el staging en $dest_dir"
        return 1
    }
    # HF_TOKEN, si está en el entorno, lo lee hf por su cuenta.
    if ! hf download "$repo" "$filename" --local-dir "$staging" >&2; then
        rm -rf "$staging" 2>/dev/null || true
        rp_msg error "falló la descarga de $nombre (hf)"
        rp_msg info "  hf reanuda solo: vuelve a ejecutar el install y continúa"
        return 1
    fi

    downloaded="$staging/$filename"
    if [[ ! -f "$downloaded" ]]; then
        # hf a veces normaliza el nombre: se busca por basename dentro de lo
        # recién descargado.
        downloaded="$(find "$staging" -type f -name "$(basename "$filename")" -print -quit 2>/dev/null || true)"
    fi
    if [[ -z "$downloaded" || ! -f "$downloaded" ]]; then
        rm -rf "$staging" 2>/dev/null || true
        rp_msg error "falló $nombre (hf dijo OK pero no aparece $filename)"
        return 1
    fi

    # Renombrado comparando rutas absolutas: `mv X X` con cadenas distintas
    # (relativa vs absoluta) no es un no-op limpio.
    local abs_downloaded abs_final
    abs_downloaded="$(cd "$(dirname "$downloaded")" && pwd)/$(basename "$downloaded")"
    abs_final="$dest_dir/$dest_name"
    if [[ "$abs_downloaded" != "$abs_final" ]]; then
        if mv "$abs_downloaded" "$abs_final"; then
            rp_msg ok "$nombre → $dest_name"
        else
            rm -rf "$staging" 2>/dev/null || true
            rp_msg error "no se pudo renombrar $abs_downloaded a $abs_final"
            return 1
        fi
    else
        rp_msg ok "$nombre descargado"
    fi
    rm -rf "$staging" 2>/dev/null || true
    return 0
}

# rp_download_safetensors <repo> <dest_dir> <nombre> <size> [excludes]
#
# Descarga un checkpoint COMPLETO (safetensors + config + tokenizer) a
# <dest_dir>: es lo que necesitan los engines con server propio. NO usa
# staging: hf es reanudable con --local-dir fijo, y un staging mataría
# precisamente esa reanudación (una descarga de 12 GB corta a medias debe
# CONTINUAR, no empezar de cero).
#
# <excludes>: globs relativos a la raíz del repo, separados por ESPACIO
# (del dump llegan con comas: los convierte el llamador). Se suman a la
# lista fija de basura. Existe porque hay checkpoints que duplican los
# pesos dentro del repo (`openai/gpt-oss-120b` trae `original/` en bf16
# junto a la versión cuantizada): sin excluirlo, el doble de disco.
rp_download_safetensors() {
    local repo="${1:-}" dest_dir="${2:-}" nombre="${3:-}" size="${4:-}"
    local excludes="${5:-}"

    if [[ -z "$repo" || -z "$nombre" ]]; then
        rp_msg error "rp_download_safetensors: argumentos incompletos (repo='$repo' nombre='$nombre')"
        return 1
    fi

    [[ -n "$dest_dir" ]] || dest_dir="${SAFETENSORS_DIR:-$HOME/models/safetensors}/$nombre"
    dest_dir="$(_rp_dl_expand_home "$dest_dir")"

    # ── ¿Está ya? Pesos + config.json: lo mínimo para que un engine pueda
    # cargarlo. El tokenizer sólo se avisa: hay checkpoints que lo traen
    # embebido en tokenizer_config.json y no tienen tokenizer.json suelto.
    local have_weights=0 have_config=0
    if [[ -d "$dest_dir" ]]; then
        rp_dir_has_weights "$dest_dir" && have_weights=1
        [[ -f "$dest_dir/config.json" ]] && have_config=1
    fi

    if (( have_weights == 1 && have_config == 1 )); then
        if [[ "${ROPERO_FORCE:-0}" == "1" ]]; then
            rp_msg info "--force: borrando $dest_dir"
            rm -rf "$dest_dir" || { rp_msg error "no se pudo borrar $dest_dir"; return 1; }
            have_weights=0; have_config=0
        else
            local wmb
            wmb="$(du -sm "$dest_dir" 2>/dev/null | awk '{print $1; exit}')"
            rp_msg ok "$nombre ya descargado (${wmb:-?} MB en $dest_dir)"
            return 0
        fi
    fi

    # Marcador sin pesos: alguien borró los shards o una descarga quedó a
    # medias. Se dice, porque si no parece un bug de ropero.
    if [[ -f "$dest_dir/.download_complete" ]] && (( have_weights == 0 )); then
        rp_msg warn "$nombre: $dest_dir dice estar descargado pero no tiene pesos"
        rp_msg warn "  Se vuelve a descargar (los shards que quedaran se sobrescriben)"
        rm -f "$dest_dir/.download_complete" 2>/dev/null || true
    fi

    _rp_dl_require_hf "$nombre" || return 1

    if ! rp_confirm "¿Descargar $nombre (~${size:-?}) en $dest_dir?"; then
        rp_msg info "$nombre omitido por el usuario"
        return 0
    fi

    mkdir -p "$dest_dir" || { rp_msg error "no se pudo crear $dest_dir"; return 1; }

    # Excludes: los de basura siempre + los del llamador.
    local -a hf_args=(download "$repo" --local-dir "$dest_dir")
    local e
    # shellcheck disable=SC2086  # excludes: globs del TOML separados por espacio
    for e in ".gitattributes" "*.md" ".DS_Store" "*.png" "*.jpg" "*.gif" $excludes; do
        [[ -n "$e" ]] || continue
        hf_args+=(--exclude "$e")
    done

    rp_msg info "  hf ${hf_args[*]}"
    # El output de hf va a stderr: si se fuera a stdout, un install con
    # selección parecería escupir basura en vez de un modelo.
    if ! hf ${hf_args[@]+"${hf_args[@]}"} >&2; then
        rp_msg error "falló la descarga de $nombre (hf)"
        rp_msg info "  hf reanuda solo: vuelve a ejecutar el install y continúa"
        return 1
    fi

    # ── Verificación: hf puede salir con 0 y dejar el directorio a medias.
    # El marcador sólo se toca si los pesos están de verdad.
    have_weights=0; have_config=0
    rp_dir_has_weights "$dest_dir" && have_weights=1
    [[ -f "$dest_dir/config.json" ]] && have_config=1
    if (( have_weights == 0 || have_config == 0 )); then
        rp_msg error "hf terminó pero $dest_dir está incompleto"
        (( have_weights == 0 )) && rp_msg info "  faltan los pesos (*.safetensors / pytorch_model*.bin / *.npz)"
        (( have_config == 0 )) && rp_msg info "  falta config.json (¿repo equivocado?)"
        rm -f "$dest_dir/.download_complete" 2>/dev/null || true
        return 1
    fi

    touch "$dest_dir/.download_complete" 2>/dev/null || true
    local wmb
    wmb="$(du -sm "$dest_dir" 2>/dev/null | awk '{print $1; exit}')"
    # En MB mientras sea pequeño: decir "0 GB" de un modelo de 15 MB parece
    # un error; el redondeo a GB sólo tiene sentido a partir del gigabyte.
    if (( ${wmb:-0} >= 1024 )); then
        rp_msg ok "$nombre descargado ($(( ${wmb:-0} / 1024 )) GB en $dest_dir)"
    else
        rp_msg ok "$nombre descargado (${wmb:-?} MB en $dest_dir)"
    fi
    return 0
}
