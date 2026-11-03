#!/usr/bin/env bash
# =============================================================================
# diagnostico.sh — recoge TODO lo necesario para diagnosticar el motor local
# en UN solo fichero, para pegarlo (o adjuntarlo) y poder mirarlo.
#
# POR QUÉ UN RECOLECTOR
#
# Preguntar por trozos es lento y se pierde contexto: el log del server, la
# config real del modelo, el estado del sistema, el de la GPU y el estado de
# ropero son cinco fuentes que hay que leer JUNTAS. Este script las junta.
#
# REGLAS
#   · NUNCA pide contraseña: si hace falta sudo, usa `sudo -n` (no interactivo)
#     y anota que no pudo.
#   · NUNCA para nada, ni reinicia, ni escribe en la config.
#   · NUNCA se cuelga: todo con timeout.
#   · El fichero sale con el nombre y la ruta al final, para pegarlo.
#
# USO
#   bin/diagnostico.sh                 # -> /tmp/ropero-diag-<fecha>.txt
#   bin/diagnostico.sh ~/mi-diag.txt   # -> esa ruta
#   bin/diagnostico.sh --modelos nemotron,qwen38,flint
# =============================================================================
set -uo pipefail

OUT=""
MODELOS="nemotron,qwen38,flint,gptoss,agenta1"
LOG_DIR="${LOG_DIR:-$HOME/.logs/ropero}"
MODELS_DIR="${RP_MODELS_DIR:-$HOME/workspace/github/lasaca/ropero/models}"
GGUF_DIR="${GGUF_DIR:-$HOME/models/gguf}"

# El home de ropero: si no está en el sitio de siempre, se deduce del binario
# que está en el PATH (el entrypoint es un symlink al repo).
ROPERO_HOME="${ROPERO_HOME:-}"
if [[ -z "$ROPERO_HOME" ]]; then
    for cand in "$HOME/workspace/github/lasaca/ropero" "$HOME/ropero" "$HOME/src/ropero"; do
        if [[ -d "$cand" ]]; then ROPERO_HOME="$cand"; break; fi
    done
fi
if [[ -z "$ROPERO_HOME" ]]; then
    _rp="$(command -v ropero 2>/dev/null || true)"
    if [[ -n "$_rp" ]]; then
        _rp="$(readlink -f "$_rp" 2>/dev/null || echo "$_rp")"
        ROPERO_HOME="$(cd "$(dirname "$_rp")/.." 2>/dev/null && pwd || echo "")"
    fi
fi
ROPERO_HOME="${ROPERO_HOME:-$HOME/workspace/github/lasaca/ropero}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --modelos) MODELOS="$2"; shift 2 ;;
        --out)     OUT="$2"; shift 2 ;;
        --help|-h) sed -n '2,28p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done
[[ -n "$OUT" ]] || OUT="/tmp/ropero-diag-$(date +%Y%m%d-%H%M%S).txt"

mkdir -p "$(dirname "$OUT")" 2>/dev/null
: >"$OUT" || { echo "no pude escribir en $OUT" >&2; exit 1; }

# ── Utilidades ───────────────────────────────────────────────────────────────
seccion() { printf '\n\n══════════ %s ══════════\n' "$*" >>"$OUT"; }
sub()     { printf '\n--- %s ---\n' "$*" >>"$OUT"; }
cmd()     { printf '\n$ %s\n' "$*" >>"$OUT"; timeout 25 bash -c "$*" >>"$OUT" 2>&1 || printf '(rc=%s)\n' "$?" >>"$OUT"; }
volcar()  { # volcar <fichero> [máx líneas]
    local f="$1" n="${2:-200}"
    if [[ -f "$f" ]]; then
        printf '\n### %s (%s líneas, últimas %s)\n' "$f" "$(wc -l <"$f" 2>/dev/null)" "$n" >>"$OUT"
        tail -n "$n" "$f" >>"$OUT" 2>/dev/null
    else
        printf '\n### %s — NO EXISTE\n' "$f" >>"$OUT"
    fi
}
volcar_dir() { # volcar_dir <dir> <filtro> [máx]
    local d="$1" f="$2" n="${3:-200}" x
    if [[ -d "$d" ]]; then
        printf '\n### %s/*.%s\n' "$d" "$f" >>"$OUT"
        for x in "$d"/*."$f"; do
            [[ -f "$x" ]] || continue
            printf '\n--- %s ---\n' "$x" >>"$OUT"
            tail -n "$n" "$x" >>"$OUT" 2>/dev/null
        done
    else
        printf '\n### %s — NO EXISTE\n' "$d" >>"$OUT"
    fi
}
tiene_sudo() { sudo -n true 2>/dev/null; }

# ── 0. Cabecera ──────────────────────────────────────────────────────────────
{
    printf 'diagnóstico de motores locales\n'
    printf 'generado: %s\n' "$(date -Is)"
    printf 'host: %s\n' "$(hostname -f 2>/dev/null || hostname)"
    printf 'usuario: %s\n' "$(id -un)"
    printf 'kernel: %s\n' "$(uname -r)"
    printf 'distro: %s\n' "$(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")"
    printf 'locale: %s\n' "$(locale 2>/dev/null | head -1)"
    printf 'sudo sin contraseña: %s\n' "$(tiene_sudo && echo SÍ || echo NO)"
    printf 'nvidia-smi: %s\n' "$(command -v nvidia-smi || echo NO ESTÁ)"
    printf 'lsof: %s\n' "$(command -v lsof || echo NO ESTÁ)"
    printf 'python3: %s\n' "$(command -v python3 || echo NO)"
} >>"$OUT"

# ── 1. ropero ────────────────────────────────────────────────────────────────
seccion "ROPERO"
if [[ -d "$ROPERO_HOME" ]]; then
    sub "versión y commit"
    cmd "cd '$ROPERO_HOME' && git log --oneline -5 && git status --short | head -20 && git remote -v"
else
    printf 'ropero no está en %s\n' "$ROPERO_HOME" >>"$OUT"
fi

sub "doctor"
cmd "cd '$ROPERO_HOME' && bash bin/ropero doctor"
sub "status"
cmd "cd '$ROPERO_HOME' && bash bin/ropero status"
sub "config llama-server"
cmd "cd '$ROPERO_HOME' && bash bin/ropero config llama-server"
sub "config (todos los engines)"
cmd "cd '$ROPERO_HOME' && bash bin/ropero config"
sub "list"
cmd "cd '$ROPERO_HOME' && bash bin/ropero list --all"
sub "path de los modelos"
for m in ${MODELOS//,/ }; do
    printf '\n### %s\n' "$m" >>"$OUT"
    cmd "cd '$ROPERO_HOME' && bash bin/ropero path '$m'"
    cmd "cd '$ROPERO_HOME' && bash bin/ropero args '$m'"
done

# ── 2. El motor: dónde está y con qué flags se compiló ─────────────────────
seccion "EL MOTOR (llama-server)"
cmd "which -a llama-server; ls -la '$HOME/.local/bin/llama-server' '$HOME/.local/share/ropero/llama-server' '$HOME/llama.cpp/build/bin/llama-server' 2>/dev/null"
cmd "ls -la '$HOME'/atomic*/llama.cpp/build/bin/llama-server '$HOME'/llama*/build/bin/llama-server 2>/dev/null"
cmd "find '$HOME' -maxdepth 5 -name 'llama-server' -type f -perm -u+x 2>/dev/null | head -20"

MOTOR_BIN="$(command -v llama-server 2>/dev/null || true)"
[[ -z "$MOTOR_BIN" ]] && MOTOR_BIN="$(ls "$HOME"/llama*/build/bin/llama-server "$HOME"/atomic*/llama.cpp/build/bin/llama-server 2>/dev/null | head -1)"
if [[ -n "$MOTOR_BIN" && -x "$MOTOR_BIN" ]]; then
    sub "versión del motor: $MOTOR_BIN"
    cmd "'$MOTOR_BIN' --version"
    sub "de dónde viene (git)"
    MOTOR_DIR="$(dirname "$(dirname "$MOTOR_BIN")")"
    cmd "cd '$MOTOR_DIR' 2>/dev/null && git log --oneline -3 && git status --short | head -10 && git remote -v"
    sub "flags con las que se compiló (CMakeCache)"
    for c in "$MOTOR_DIR"/build/CMakeCache.txt "$MOTOR_DIR"/CMakeCache.txt "$MOTOR_DIR"/../CMakeCache.txt; do
        [[ -f "$c" ]] || continue
        printf '\n### %s\n' "$c" >>"$OUT"
        grep -iE "GGML_CUDA|CMAKE_BUILD_TYPE|CMAKE_CUDA_FLAGS|CMAKE_CXX_FLAGS|GGML_NATIVE|LTO|INTERPROCEDURAL|CUDA_ARCH" "$c" >>"$OUT" 2>/dev/null
    done
else
    printf '\nNO ENCUENTRO ningún llama-server ejecutable\n' >>"$OUT"
fi

# ── 3. Los procesos vivos: la config REAL con la que se arrancaron ──────────
seccion "MOTORES VIVOS (la línea de órdenes completa = la config real)"
cmd "ps -eo pid,ppid,etimes,rss,args | grep -E '[l]lama-server|[v]llm|[t]ensorrt' | head -20"
cmd "ps -eo pid,etimes,args | grep -E '[o]pencode|[m]code|[p]i ' | head -20"

# ── 4. Los logs del arranque: donde está la verdad ──────────────────────────
seccion "LOGS DE ARRANQUE (las últimas líneas y las marcas clave)"
volcar_dir "$LOG_DIR" "log" 120
sub "las marcas que importan (moe-cache, memory breakdown, KV, timings, assert)"
for f in "$LOG_DIR"/*.log; do
    [[ -f "$f" ]] || continue
    printf '\n### %s\n' "$f" >>"$OUT"
    grep -iE "moe.cache|memory breakdown|KV cache size|auto-asymmetric|eval time|prompt eval time|tokens per second|GGML_ASSERT|error|failed|sw thermal|clock" "$f" 2>/dev/null | tail -60 >>"$OUT"
done

# ── 5. Los TOML de los modelos ──────────────────────────────────────────────
seccion "LOS TOML DE LOS MODELOS (la config declarada)"
for m in ${MODELOS//,/ }; do
    f=""
    while IFS= read -r x; do f="$x"; break; done < <(find "$MODELS_DIR" -name "$m.toml" 2>/dev/null | head -1)
    if [[ -n "$f" ]]; then
        printf '\n### %s\n' "$f" >>"$OUT"
        cat "$f" >>"$OUT" 2>/dev/null
    else
        printf '\n### %s.toml — no encontrado en %s\n' "$m" "$MODELS_DIR" >>"$OUT"
    fi
done

# ── 6. El sistema: los ajustes que tocan el rendimiento ─────────────────────
seccion "AJUSTES DEL SISTEMA"
sub "lo que pone el setup performance"
for f in /etc/security/limits.d/99-llm.conf /etc/sysctl.d/99-llm.conf /etc/sysctl.d/99-llm-mem.conf \
         /etc/sysctl.d/99-energy.conf /etc/udev/rules.d/60-llm-nvme-readahead.rules \
         /etc/modprobe.d/nvidia-pm.conf /etc/anacrontab /etc/ananicy.d/99-llm.rules; do
    printf '\n### %s\n' "$f" >>"$OUT"
    if [[ -f "$f" ]]; then cat "$f" >>"$OUT"; else printf '(no existe)\n' >>"$OUT"; fi
done

sub "cmdline del kernel (activo) y donde está escrito"
cmd "cat /proc/cmdline"
cmd "cat /etc/kernel/cmdline 2>/dev/null; grep -n 'GRUB_CMDLINE' /etc/default/grub 2>/dev/null; ls -la /etc/kernel/ 2>/dev/null"

sub "sysctl relevantes (los que tocan el decode)"
cmd "sysctl vm.swappiness vm.min_free_kbytes vm.max_map_count vm.numa_balancing kernel.numa_balancing vm.page-cluster vm.compaction_proactiveness kernel.nmi_watchdog vm.dirty_ratio vm.dirty_background_ratio 2>/dev/null"

sub "THP"
cmd "cat /sys/kernel/mm/transparent_hugepage/enabled /sys/kernel/mm/transparent_hugepage/defrag 2>/dev/null"

sub "read-ahead del NVMe"
cmd "for d in /sys/block/nvme*n1; do echo -n \"\$d: \"; cat \$d/queue/read_ahead_kb 2>/dev/null; done; cat /sys/block/nvme*n1/queue/scheduler 2>/dev/null"

sub "memoria y swap"
cmd "grep -E 'MemTotal|MemAvailable|Cached|Buffers|SwapTotal|SwapFree|Dirty|Writeback' /proc/meminfo"
cmd "swapon --show 2>/dev/null; ls -la /swapfile* 2>/dev/null; cat /proc/sys/vm/pressure/* 2>/dev/null | head -20"

sub "los GGUF y su tamaño (el page cache tiene que coger el modelo)"
cmd "ls -la '$GGUF_DIR'/*/*.gguf 2>/dev/null | head -30"

# ── 7. La GPU ────────────────────────────────────────────────────────────────
seccion "GPU"
if command -v nvidia-smi >/dev/null 2>&1; then
    sub "nvidia-smi"
    cmd "nvidia-smi"
    sub "PERFORMANCE (razones de throttle)"
    cmd "nvidia-smi -q -d PERFORMANCE"
    sub "POWER (techo de potencia: si oscila, algo lo reescribe)"
    cmd "nvidia-smi -q -d POWER"
    sub "CLOCK (relojes actuales y máximos)"
    cmd "nvidia-smi -q -d CLOCK"
    sub "TEMPERATURE"
    cmd "nvidia-smi -q -d TEMPERATURE"
else
    printf 'nvidia-smi no está\n' >>"$OUT"
fi

# ── 8. El setup: el script que hay que arreglar ─────────────────────────────
seccion "EL SCRIPT DE SETUP (para poder arreglarlo)"
for s in "$HOME/setup" "$HOME/setup.sh" "$HOME/bin/setup" "$HOME/bin/setup.sh" \
         "$HOME/.local/bin/setup" /usr/local/bin/setup /usr/local/bin/setup.sh; do
    if [[ -f "$s" ]]; then
        printf '\n### %s\n' "$s" >>"$OUT"
        cat "$s" >>"$OUT" 2>/dev/null
    fi
done
printf '\n### bin/80-performance.sh (si está en un repo de setup)\n' >>"$OUT"
find "$HOME" -maxdepth 6 -name "8*-performance.sh" 2>/dev/null | head -5 | while IFS= read -r f; do
    printf '\n--- %s ---\n' "$f" >>"$OUT"
    cat "$f" >>"$OUT" 2>/dev/null
done

# ── 9. El kernel: OOM kills y throttling ────────────────────────────────────
seccion "KERNEL (OOM kills, NVIDIA, errores)"
if tiene_sudo; then
    cmd "sudo -n dmesg -T 2>/dev/null | grep -iE 'oom|killed process|nvrm|Xid|nvidia' | tail -60"
else
    cmd "dmesg -T 2>/dev/null | grep -iE 'oom|killed process|nvrm|Xid|nvidia' | tail -60"
    printf '\n(sin sudo: puede faltar parte del log del kernel)\n' >>"$OUT"
fi
cmd "journalctl -k --no-pager -n 200 2>/dev/null | grep -iE 'oom|killed process|nvrm|Xid' | tail -40"

# ── 10. Resumen ──────────────────────────────────────────────────────────────
seccion "RESUMEN RÁPIDO"
{
    printf 'moe-cache (última línea con granted):\n'
    grep -ih "moe.cache.*granted" "$LOG_DIR"/*.log 2>/dev/null | tail -3
    printf '\nasserts/errores en los logs:\n'
    grep -ihE "GGML_ASSERT|error:|failed" "$LOG_DIR"/*.log 2>/dev/null | tail -10
    printf '\nswappiness: %s\n' "$(sysctl -n vm.swappiness 2>/dev/null)"
    printf 'min_free_kbytes: %s\n' "$(cat /proc/sys/vm/min_free_kbytes 2>/dev/null)"
    printf 'swap en uso: %s\n' "$(swapon --show=USED --noheadings 2>/dev/null | tr -s ' ')"
    printf 'llama-server vivos: %s\n' "$(pgrep -x llama-server 2>/dev/null | tr '\n' ' ')"
} >>"$OUT"

printf '\n\nFichero: %s (%s líneas)\n' "$OUT" "$(wc -l <"$OUT")" >&2
printf 'Pega ese fichero (o adjúntalo) y lo diagnose.\n' >&2
