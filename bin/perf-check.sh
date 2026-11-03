#!/usr/bin/env bash
# =============================================================================
# perf-check.sh — el estado REAL de los ajustes de rendimiento en esta máquina.
#
# POR QUÉ ESTE SCRIPT
#
# "setup performance" es idempotente: cada paso está guardado por "si el
# fichero existe, omito" y los "omito" se registran a nivel debug. En una
# máquina donde ya está todo aplicado, el script no hace NADA y no dice NADA:
# parece que no funciona, pero es que ya estaba hecho. Y lo que NO puede
# aplicar (el cmdline del kernel en un bootloader que no es sdboot, el
# PowerMizer en un portátil con TGP dinámico) se avisa pero no se distingue
# de "ya estaba".
#
# Este script responde a las preguntas que el setup no contesta:
#   · ¿está cada ajuste puesto de verdad, con el VALOR que importa?
#   · ¿qué falta?
#   · ¿hay algo que esté degradando el rendimiento AHORA MISMO?
#
# Y sobre todo, lo último: sirve para mirar la máquina DESPUÉS de un rato de
# trabajo, que es cuando aparece el "antes iba a 100 t/s y ahora va a 60".
# Los tres sospechosos de esa caída son:
#   1. la GPU con thermal/power throttle (y por qué),
#   2. el page cache: si el GGUF no cabe en la RAM, cada token va al NVMe,
#   3. el swap lleno.
#
# USO
#   perf-check.sh                 informe
#   perf-check.sh --gpu           sólo la GPU (rápido)
#   perf-check.sh --json          salida JSON (para comparar dos momentos)
#
# SALIDA: 0 si todo está en orden, 1 si hay algo que revisar.
# =============================================================================
set -uo pipefail

SOLO_GPU=0
JSON=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --gpu)  SOLO_GPU=1; shift ;;
        --json) JSON=1; shift ;;
        --help|-h) sed -n '2,30p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done

WARN=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
falta(){ printf '  \033[33m!\033[0m %s\n' "$*"; WARN=1; }
mal()  { printf '  \033[31m✗\033[0m %s\n' "$*"; WARN=1; }
tit()  { printf '\n\033[1m%s\033[0m\n' "$*"; }

# ── 1. Los ajustes del sistema ───────────────────────────────────────────────
check_sistema() {
    tit "Ajustes del sistema (setup performance)"

    # limits.d
    local lim="/etc/security/limits.d/99-llm.conf"
    if [[ -f "$lim" ]]; then
        if grep -q "memlock unlimited" "$lim" 2>/dev/null; then
            ok "memlock unlimited ($lim)"
        else
            falta "memlock NO está en unlimited ($lim)"
        fi
        grep -q "nofile 262144" "$lim" 2>/dev/null \
            && ok "nofile 262144 ($lim)" \
            || falta "nofile no está en 262144 ($lim)"
    else
        falta "no existe $lim (setup performance)"
    fi

    # sysctl: cada clave con su valor esperado
    check_sysctl vm.max_map_count 1048576
    check_sysctl vm.numa_balancing 0
    check_sysctl vm.page-cluster 0
    check_sysctl kernel.nmi_watchdog 0
    check_sysctl vm.compaction_proactiveness 0
    check_sysctl vm.swappiness 5

    # min_free_kbytes (el colchón anti-OOM)
    local mfk
    mfk="$(cat /proc/sys/vm/min_free_kbytes 2>/dev/null || echo 0)"
    if (( mfk >= 1048576 )); then
        ok "min_free_kbytes = $(( mfk / 1024 )) MiB (colchón anti-OOM)"
    else
        falta "min_free_kbytes = $(( mfk / 1024 )) MiB (bajo 1 GiB: el OOM killer puede matar al server)"
    fi

    # THP: el formato es "always [madvise] never" — lo activo va entre
    # corchetes. Buscar "always" a secas daría un OK falso.
    local thp
    thp="$(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo '?')"
    if [[ "$thp" == *"[always]"* ]]; then
        ok "THP: $thp (always activo)"
    else
        falta "THP: $thp (lo ideal: [always] activo)"
    fi

    # read-ahead del NVMe (la carga del GGUF es secuencial)
    local dev kb
    for dev in /sys/block/nvme*n1; do
        [[ -e "$dev" ]] || continue
        kb="$(cat "$dev/queue/read_ahead_kb" 2>/dev/null || echo 0)"
        if (( kb >= 2048 )); then
            ok "read-ahead de $(basename "$dev") = $(( kb / 1024 )) MiB"
        else
            falta "read-ahead de $(basename "$dev") = $(( kb / 1024 )) MiB (lo ideal: 4 MiB)"
        fi
    done

    # NVIDIA: gestión dinámica de potencia (fine-grained) + persistencia
    if [[ -f /etc/modprobe.d/nvidia-pm.conf ]]; then
        grep -q "NVreg_DynamicPowerManagement=0x02" /etc/modprobe.d/nvidia-pm.conf 2>/dev/null \
            && ok "nvidia-pm.conf: DPM fine-grained (0x02)" \
            || falta "nvidia-pm.conf existe pero no fija DynamicPowerManagement=0x02"
    else
        falta "no existe /etc/modprobe.d/nvidia-pm.conf"
    fi
    if command -v systemctl >/dev/null 2>&1; then
        if systemctl is-enabled nvidia-persistenced >/dev/null 2>&1; then
            ok "nvidia-persistenced habilitado"
        else
            falta "nvidia-persistenced NO está habilitado"
        fi
    fi

    # El cmdline del kernel: ¿dónde vive en ESTA máquina?
    local cmd_where="" cmd_val=""
    if [[ -r /proc/cmdline ]]; then
        cmd_val="$(cat /proc/cmdline)"
        if [[ "$cmd_val" == *rcutree.enable_rcu_lazy=1* ]]; then
            ok "rcutree.enable_rcu_lazy=1 en el cmdline ACTIVO"
            cmd_where="activo"
        else
            cmd_where="activo-sin-el-flag"
        fi
    fi
    # ¿está escrito en algún sitio (aún sin reiniciar)?
    local fuente=""
    if [[ -f /etc/kernel/cmdline ]]; then
        grep -q "rcutree.enable_rcu_lazy" /etc/kernel/cmdline 2>/dev/null \
            && fuente="/etc/kernel/cmdline (systemd-boot/mkinitcpio)"
    fi
    if [[ -z "$fuente" && -f /etc/default/grub ]]; then
        grep -q "rcutree.enable_rcu_lazy" /etc/default/grub 2>/dev/null \
            && fuente="/etc/default/grub (GRUB)"
    fi
    if [[ -n "$fuente" ]]; then
        ok "rcutree.enable_rcu_lazy=1 en $fuente"
        [[ "$cmd_where" == "activo" ]] || falta "  …pero NO está en el cmdline activo: reinicia para que aplique"
    else
        falta "rcutree.enable_rcu_lazy=1 no está en ningún sitio (ni /etc/kernel/cmdline ni /etc/default/grub)"
    fi
}

# check_sysctl <clave> <valor esperado>
check_sysctl() {
    local clave="$1" esperado="$2" actual
    actual="$(sysctl -n "$clave" 2>/dev/null || echo '?')"
    if [[ "$actual" == "$esperado" ]]; then
        ok "$clave = $actual"
    else
        falta "$clave = ${actual} (esperado: $esperado)"
    fi
}

# ── 2. La memoria: el GGUF tiene que caber en el page cache ─────────────────
check_memoria() {
    tit "Memoria (el GGUF se mmapea: vive en el page cache)"
    local total avail cached swap_t swap_f sw
    total="$(awk '/^MemTotal:/{printf "%.1f", $2/1048576}' /proc/meminfo 2>/dev/null)"
    avail="$(awk '/^MemAvailable:/{printf "%.1f", $2/1048576}' /proc/meminfo 2>/dev/null)"
    cached="$(awk '/^Cached:/{printf "%.1f", $2/1048576}' /proc/meminfo 2>/dev/null)"
    swap_t="$(awk '/^SwapTotal:/{printf "%.1f", $2/1048576}' /proc/meminfo 2>/dev/null)"
    swap_f="$(awk '/^SwapFree:/{printf "%.1f", $2/1048576}' /proc/meminfo 2>/dev/null)"
    sw="$(cat /proc/sys/vm/swappiness 2>/dev/null || echo '?')"
    printf '  RAM total %s GiB · disponible %s GiB · page cache %s GiB\n' "${total:-?}" "${avail:-?}" "${cached:-?}"
    printf '  swap %s GiB (libre %s GiB) · swappiness %s\n' "${swap_t:-?}" "${swap_f:-?}" "$sw"
    if [[ -n "$swap_t" && "$swap_t" != "0.0" && "$swap_f" != "$swap_t" ]]; then
        mal "hay swap en uso: con el page cache lleno, el modelo se va a swap y el decode se desploma"
    fi
    # El page cache debería poder coger el GGUF más grande del catálogo.
    local mayor=0 f
    for f in "$HOME"/models/gguf/*/*.gguf; do
        [[ -f "$f" ]] || continue
        local sz
        sz="$(stat -c %s "$f" 2>/dev/null || echo 0)"
        (( sz > mayor )) && mayor="$sz"
    done
    if (( mayor > 0 )); then
        printf '  mayor GGUF en disco: %.1f GiB\n' "$(awk -v b="$mayor" 'BEGIN{printf "%.1f", b/1073741824}')"
        if awk -v m="$mayor" -v c="${cached:-0}" -v t="${total:-0}" \
             'BEGIN{exit (c + t) < m ? 0 : 1}'; then
            mal "el GGUF más grande NO cabe en RAM+cache: cada token va al NVMe"
        else
            ok "hay sitio de RAM para el GGUF"
        fi
    fi
}

# ── 3. La GPU: aquí está el "va más lento" ───────────────────────────────────
check_gpu() {
    tit "GPU"
    command -v nvidia-smi >/dev/null 2>&1 || { falta "nvidia-smi no está (¿driver NVIDIA?)"; return; }

    local name sm power temp util mem
    name="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)"
    sm="$(nvidia-smi --query-gpu=clocks.sm,clocks.max.sm --format=csv,noheader,nounits 2>/dev/null | head -1)"
    power="$(nvidia-smi --query-gpu=power.draw,power.limit --format=csv,noheader,nounits 2>/dev/null | head -1)"
    temp="$(nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader 2>/dev/null | head -1)"
    util="$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>/dev/null | head -1)"
    mem="$(nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null | head -1)"
    printf '  %s\n' "${name:-?}"
    printf '  SM %s MHz · %s W · %s C · %s%% · VRAM %s MiB\n' \
        "${sm:-?}" "${power:-?}" "${temp:-?}" "${util:-?}" "${mem:-?}"

    # Las razones de throttle: EL dato que explica una caída de rendimiento.
    local reasons
    reasons="$(nvidia-smi -q -d PERFORMANCE 2>/dev/null | sed -n '/Clocks Event Reasons/,/^$/p' \
        | grep -E "Active|Not Active" | sed 's/^[[:space:]]*//' | head -12)"
    if [[ -n "$reasons" ]]; then
        printf '\n  Clocks Event Reasons:\n'
        printf '%s\n' "$reasons" | sed 's/^/    /'
        if printf '%s' "$reasons" | grep -q "Slowdown: Active"; then
            mal "THROTTLE ACTIVO: la GPU está frenada por temperatura o por el tope de potencia"
            mal "  el culpable suelen ser el firmware/TGP del portátil (nvidia-smi -pl no se automatiza)"
        fi
    fi

    # El techo de potencia: si oscila, algo lo está reescribiendo.
    local ceil req
    ceil="$(nvidia-smi -q -d POWER 2>/dev/null | grep -iE "GPU Ceiling Power Limit" | head -2 | tr -s ' ')"
    req="$(nvidia-smi -q -d POWER 2>/dev/null | grep -iE "Requested Power Limit" | head -1 | tr -s ' ')"
    printf '\n  %s\n' "${ceil:-ceiling power limit: ?}"
    printf '  %s\n' "${req:-requested power limit: ?}"

    # Persistencia
    local pm
    pm="$(nvidia-smi -q 2>/dev/null | grep -i "Persistence Mode" | head -1 | tr -s ' ')"
    printf '  %s\n' "${pm:-persistence mode: ?}"
    if [[ "$pm" == *"Enabled"* ]]; then
        ok "persistencia activa"
    else
        falta "persistencia OFF: el driver puede descargar la GPU entre peticiones"
    fi
}

# ── 4. Los procesos de inferencia que siguen vivos ──────────────────────────
check_procesos() {
    tit "Motores de inferencia vivos"
    local p n=0
    for p in $(pgrep -x llama-server 2>/dev/null); do
        n=$((n + 1))
        printf '  llama-server pid %s · %s\n' "$p" "$(ps -o etime= -p "$p" 2>/dev/null | tr -d ' ')"
    done
    (( n == 0 )) && ok "no hay ningún llama-server corriendo" || \
        printf '  %s motor(es) en marcha (mira el VRAM que usan: dos a la vez se pelean)\n' "$n"
}

# ── main ─────────────────────────────────────────────────────────────────────
if (( JSON == 0 )); then
    printf '\033[1mperf-check\033[0m — estado de los ajustes de rendimiento\n'
fi
if (( SOLO_GPU == 0 )); then
    check_sistema
    check_memoria
fi
check_gpu
(( SOLO_GPU == 0 )) && check_procesos

if (( JSON == 0 )); then
    printf '\n'
    if (( WARN == 0 )); then
        printf '\033[32mTodo en orden\033[0m\n'
    else
        printf '\033[33mHay cosas que revisar\033[0m (las marcadas con ! o ✗)\n'
    fi
fi
exit $(( WARN > 0 ? 1 : 0 ))
