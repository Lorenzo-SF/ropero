#!/usr/bin/env bash
# =============================================================================
# 80-performance.sh — tuning de CachyOS / WSL2 para motores locales
#
# Modos:
#   ./80-performance.sh              aplica (idempotente)
#   ./80-performance.sh --check      sólo verifica el estado (no cambia nada)
#   ./80-performance.sh --rollback   revierte lo que este script puso
#
# Si se integra en un setup.sh que despacha por función, este fichero define
# setup_performance() y setup_perf_rollback() y no hace nada al ser橱柜do.
#
# REGLAS DE ESTE FICHERO (las que faltaban antes)
#
# 1. NUNCA MUERE A MEDIAS. No hay `set -e`: cada paso va en su propia función
#    y un fallo se anota y se sigue. Antes, el script se paraba en el paso del
#    cmdline y todo lo que venía detrás (swappiness, DPM de NVIDIA…) se
#    quedaba sin aplicar, sin decir nada: "no hace nada" sin explanation.
#
# 2. NUNCA ES SILENCIOSO. Cada paso dice si lo APLICÓ, si YA ESTABA o si NO
#    APLICA, y al final hay un RESUMEN con las tres listas. Antes los "ya
#    estaba" iban a nivel debug (invisibles) y el backup salía vacío porque
#    no se registraba lo que se cambiaba.
#
# 3. EL BACKUP REGISTRA DE VERDAD. /var/lib/setup-llm-backup/<fecha>/
#    guarda los valores anteriores de todo lo que toca (para el rollback) y
#    un manifest.json con lo que se hizo. Antes el manifest era {"version":1}
#    y no había nada que revertir.
#
# 4. DETECTA EL BOOTLOADER. El cmdline del kernel vive en un sitio u otro:
#    systemd-boot → /etc/kernel/cmdline (+mkinitcpio)
#    GRUB         → /etc/default/grub (+update-grub)
#    Antes sólo se miraba el primero: en un portátil con GRUB el
#    rcutree.enable_rcu_lazy=1 se saltaba en silencio.
#
# 5. UN SOLO sudo. sudo -v al principio cachea la credencial y no vuelve a
#    pedir la contraseña a mitad.
# =============================================================================

# ── infra ────────────────────────────────────────────────────────────────────
if ! declare -F log_info >/dev/null 2>&1; then
    log_info()  { printf '  \033[36mℹ\033[0m %s\n' "$*"; }
    log_ok()    { printf '  \033[32m✓\033[0m %s\n' "$*"; }
    log_warn()  { printf '  \033[33m!\033[0m %s\n' "$*"; }
    log_error() { printf '  \033[31m✗\033[0m %s\n' "$*"; }
    log_debug() { :; }
    log_success(){ printf '\033[32m%s\033[0m\n' "$*"; }
    cmd_exists() { command -v "$1" >/dev/null 2>&1; }
    detect_os() {
        if [[ -f /etc/cachyos-release ]] || grep -qi cachyos /etc/os-release 2>/dev/null; then echo cachy
        elif [[ -f /etc/arch-release ]]; then echo arch
        elif grep -qi microsoft /proc/version 2>/dev/null; then echo wsl2
        else echo linux; fi
    }
    install_pacman() { sudo pacman -S --needed --noconfirm "$1" >/dev/null 2>&1; }
fi

# Si ya somos root, no hace falta sudo: y puede que sudo no esté instalado.
if [[ "$(id -u)" == "0" ]] && ! command -v sudo >/dev/null 2>&1; then
    sudo() { "$@"; }
fi

# Listas del resumen (aplicado / ya estaba / no aplica / falló)
declare -a _SP_APLICADO=() _SP_YA=() _SP_NO_APLICA=() _SP_FALLO=()
_SP_NOTAS=()

_sp_aplicado() { _SP_APLICADO+=("$1"); log_ok "$1"; }
_sp_ya()       { _SP_YA+=("$1");       log_info "$1 (ya estaba)"; }
_sp_no()       { _SP_NO_APLICA+=("$1"); log_info "$1 (no aplica en esta máquina)"; }
_sp_fallo()    { _SP_FALLO+=("$1");     log_warn "$1"; }
_sp_nota()     { _SP_NOTAS+=("$1");     log_info "$1"; }

# ── El backup ────────────────────────────────────────────────────────────────
SP_BACKUP_ROOT="/var/lib/setup-llm-backup"
SP_BACKUP=""
SP_MANIFEST=""

_sp_backup_init() {
    SP_BACKUP="$SP_BACKUP_ROOT/$(date +%Y%m%d-%H%M%S)"
    sudo mkdir -p "$SP_BACKUP" 2>/dev/null || { SP_BACKUP=""; return 1; }
    sudo chmod 700 "$SP_BACKUP" 2>/dev/null || true
    SP_MANIFEST="$SP_BACKUP/manifest.json"
    sudo touch "$SP_MANIFEST" 2>/dev/null || true
    sudo chmod 600 "$SP_MANIFEST" 2>/dev/null || true
    log_info "backup: $SP_BACKUP"
    return 0
}

# _sp_record <clave> <valor> — anota un valor anterior para el rollback.
_sp_record() {
    local k="$1" v="$2"
    [[ -n "$SP_MANIFEST" ]] || return 0
    printf '%s\t%s\n' "$k" "$v" >>"$SP_BACKUP/values.tsv" 2>/dev/null || \
        sudo tee -a "$SP_BACKUP/values.tsv" >/dev/null 2>&1 <<<"$k"$'\t'"$v"
    return 0
}

_sp_record_file() { # _sp_record_file <ruta> — el fichero no existía antes
    [[ -n "$SP_MANIFEST" ]] || return 0
    printf 'file\t%s\n' "$1" | sudo tee -a "$SP_BACKUP/files.tsv" >/dev/null 2>&1
    return 0
}

# ── Cada ajuste, por separado (y que un fallo NO pare el resto) ─────────────
_sp_limits() {
    local f="/etc/security/limits.d/99-llm.conf"
    if [[ -f "$f" ]] && grep -q "memlock unlimited" "$f" && grep -q "nofile 262144" "$f"; then
        _sp_ya "limits.d (memlock + nofile)"
        return 0
    fi
    [[ -f "$f" ]] && _sp_record_file "$f"
    cat <<'EOF' | sudo tee "$f" >/dev/null 2>&1
* soft memlock unlimited
* hard memlock unlimited
* soft nofile 262144
* hard nofile 262144
EOF
    if [[ -f "$f" ]]; then _sp_aplicado "limits.d (memlock unlimited + nofile 262144)"; else _sp_fallo "limits.d: no se pudo escribir $f"; fi
}

_sp_sysctl() {
    local f="/etc/sysctl.d/99-llm.conf" todo_ok=1
    if [[ -f "$f" ]] && grep -q "vm.max_map_count = 1048576" "$f" \
        && grep -q "vm.swappiness" "$f" 2>/dev/null; then
        _sp_ya "sysctl LLM"
        return 0
    fi
    [[ -f "$f" ]] && _sp_record_file "$f"
    cat <<'EOF' | sudo tee "$f" >/dev/null 2>&1
# Ajustes para motores locales (GGUF mmapeado). Ver bin/80-performance.sh.
vm.max_map_count = 1048576
vm.page-cluster = 0
kernel.nmi_watchdog = 0
vm.compaction_proactiveness = 0
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
EOF
    # numa_balancing: el knob se renombró a kernel.numa_balancing en los
    # kernels modernos (Linux 6.x). Se pone el que exista.
    if [[ -e /proc/sys/kernel/numa_balancing ]]; then
        echo "kernel.numa_balancing = 0" | sudo tee -a "$f" >/dev/null 2>&1
    elif [[ -e /proc/sys/vm/numa_balancing ]]; then
        echo "vm.numa_balancing = 0" | sudo tee -a "$f" >/dev/null 2>&1
    fi
    sudo sysctl --system >/dev/null 2>&1
    if [[ ! -f "$f" ]]; then
        _sp_fallo "sysctl LLM: no se pudo escribir $f"
        return 0
    fi
    # Escribir el fichero NO es aplicarlo: se comprueba el valor VIVO.
    local vivo
    vivo="$(sysctl -n vm.max_map_count 2>/dev/null || echo '?')"
    if [[ "$vivo" == "1048576" ]]; then
        _sp_aplicado "sysctl LLM (max_map_count, page-cluster, watchdog, numa)"
    else
        _sp_fallo "sysctl LLM: el fichero está escrito pero max_map_count sigue en $vivo (¿sysctl --system falló?)"
    fi
}

_sp_swappiness() {
    # EL MÁS IMPORTANTE PARA EL DECODE.
    #
    # El GGUF se mmapea: sus páginas viven en el page cache. Con
    # swappiness=100 el kernel mete el page cache en swap en cuanto hay
    # presión → cada token que toca un expert fuera del cache va al NVMe y
    # el decode se desploma. Con 5, el page cache sobrevive.
    local actual
    actual="$(sysctl -n vm.swappiness 2>/dev/null || echo '?')"
    if [[ "$actual" == "5" ]]; then
        _sp_ya "swappiness = 5"
        return 0
    fi
    _sp_record "sysctl.vm.swappiness" "$actual"
    # /etc/sysctl.d/99-energy.conf es un fichero de CachyOS: NO se pisa, se
    # añade un fichero propio con nombre inequívoco.
    local f="/etc/sysctl.d/99-llm-energy.conf"
    [[ -f "$f" ]] && _sp_record_file "$f"
    printf '# page cache del GGUF intacto: el modelo no se va a swap.\nvm.swappiness = 5\n' \
        | sudo tee "$f" >/dev/null 2>&1
    if sudo sysctl -w vm.swappiness=5 >/dev/null 2>&1; then
        _sp_aplicado "swappiness 100 → 5 (el page cache del GGUF no se va a swap)"
    else
        _sp_fallo "swappiness: no se pudo cambiar (sigue en ${actual})"
    fi
}

_sp_min_free() {
    # El colchón anti-OOM: con 30 GiB de RAM y un GGUF de 21 GiB en el page
    # cache, sin colchón el OOM killer se lleva el servidor de inferencia en
    # vez de soltar el cache.
    local total_kb actual
    total_kb="$(awk '/^MemTotal:/{print $2}' /proc/meminfo 2>/dev/null || echo 0)"
    local objetivo=$(( total_kb * 3 / 100 ))
    (( objetivo < 1048576 )) && objetivo=1048576
    (( objetivo > 4194304 )) && objetivo=4194304
    actual="$(cat /proc/sys/vm/min_free_kbytes 2>/dev/null || echo 0)"
    if (( actual >= objetivo )); then
        _sp_ya "min_free_kbytes = $(( actual / 1024 )) MiB"
        return 0
    fi
    _sp_record "sysctl.vm.min_free_kbytes" "$actual"
    local f="/etc/sysctl.d/99-llm-mem.conf"
    [[ -f "$f" ]] && _sp_record_file "$f"
    printf '# Colchon de RAM para el page cache del GGUF (motor local).\nvm.min_free_kbytes = %s\n' "$objetivo" \
        | sudo tee "$f" >/dev/null 2>&1
    if sudo sysctl -w vm.min_free_kbytes="$objetivo" >/dev/null 2>&1; then
        _sp_aplicado "min_free_kbytes $(( actual / 1024 )) → $(( objetivo / 1024 )) MiB (colchón anti-OOM)"
    else
        _sp_fallo "min_free_kbytes: no se pudo cambiar"
    fi
}

_sp_thp() {
    # THP "always": el GGUF se mmapea entero (21 GiB). Con [madvise] las
    # páginas del mapping no se agrupan → miles de TLB misses al leer pesos.
    # Con [always] el kernel usa páginas enormes y la lectura de los experts
    # en CPU va más rápido.
    local modo="${LLM_THP_MODE:-always}"
    local cur
    cur="$(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo '?')"
    if [[ "$cur" == *"[${modo}]"* ]]; then
        _sp_ya "THP = [${modo}]"
        return 0
    fi
    _sp_record "sysfs.THP.enabled" "$cur"
    echo "$modo" | sudo tee /sys/kernel/mm/transparent_hugepage/enabled >/dev/null 2>&1
    echo always | sudo tee /sys/kernel/mm/transparent_hugepage/defrag >/dev/null 2>&1
    local f="/etc/tmpfiles.d/llm-thp.conf"
    [[ -f "$f" ]] && _sp_record_file "$f"
    cat <<EOF | sudo tee "$f" >/dev/null 2>&1
w /sys/kernel/mm/transparent_hugepage/enabled - - - - $modo
w! /sys/kernel/mm/transparent_hugepage/defrag - - - - always
EOF
    local nuevo
    nuevo="$(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo '?')"
    if [[ "$nuevo" == *"[${modo}]"* ]]; then
        _sp_aplicado "THP [madvise] → [${modo}] (el mmap del GGUF usa páginas enormes)"
    else
        _sp_fallo "THP: sigue en $nuevo"
    fi
}

_sp_readahead() {
    local f="/etc/udev/rules.d/60-llm-nvme-readahead.rules"
    if [[ -f "$f" ]]; then
        _sp_ya "read-ahead del NVMe = 4 MiB"
        return 0
    fi
    _sp_record_file "$f"
    cat <<'EOF' | sudo tee "$f" >/dev/null 2>&1
# Read-ahead 4 MiB en los NVMe: la carga de un GGUF (12-20 GiB) es secuencial.
ACTION=="add|change", KERNEL=="nvme[0-9]n[0-9]", ATTR{queue/read_ahead_kb}="4096"
EOF
    sudo udevadm control --reload-rules >/dev/null 2>&1
    sudo udevadm trigger --subsystem-match=block --action=change >/dev/null 2>&1
    _sp_aplicado "read-ahead del NVMe = 4 MiB"
}

_sp_nvidia_dpm() {
    # Gestión fine-grained de potencia: la GPU baja de estado sola y sube
    # rápido cuando toca. En inferencia (100 % de GPU durante el decode) es
    # lo que evita que el driver se descargue entre peticiones.
    local f="/etc/modprobe.d/nvidia-pm.conf"
    if [[ -f "$f" ]] && grep -q "NVreg_DynamicPowerManagement=0x02" "$f"; then
        _sp_ya "NVIDIA DPM fine-grained (0x02)"
        return 0
    fi
    [[ -f "$f" ]] && _sp_record_file "$f"
    echo 'options nvidia NVreg_DynamicPowerManagement=0x02' | sudo tee "$f" >/dev/null 2>&1
    if [[ -f "$f" ]]; then
        _sp_aplicado "NVIDIA DPM fine-grained (0x02) — efectivo al recargar el módulo de NVIDIA"
        _sp_nota "para que surta efecto ahora: sudo modprobe -r nvidia && sudo modprobe nvidia (cierra la sesión gráfica)"
    else
        _sp_fallo "nvidia-pm.conf: no se pudo escribir"
    fi
}

_sp_persistenced() {
    if ! systemctl list-unit-files nvidia-persistenced.service &>/dev/null 2>&1; then
        _sp_no "nvidia-persistenced (el servicio no existe: no hay driver NVIDIA)"
        return 0
    fi
    if systemctl is-enabled nvidia-persistenced &>/dev/null 2>&1; then
        _sp_ya "nvidia-persistenced habilitado"
    else
        if sudo systemctl enable --now nvidia-persistenced 2>/dev/null; then
            _sp_aplicado "nvidia-persistenced habilitado (el driver no se descarga entre peticiones)"
        else
            _sp_fallo "nvidia-persistenced: no se pudo habilitar"
        fi
    fi
}

_sp_ananicy() {
    local f="/etc/ananicy.d/00-llm-server.rules"
    if ! cmd_exists ananicy-cpp; then
        install_pacman "ananicy-cpp" || true
    fi
    if ! cmd_exists ananicy-cpp; then
        _sp_no "ananicy-cpp (no instalado y no se pudo instalar)"
        return 0
    fi
    # sched=batch (NO rr: SCHED_RR con varios threads a rtprio 50 congela el
    # escritorio) · ioclass=best-effort (realtime deja al sistema sin disco
    # durante la carga de 20 GB) · oom_score_adj=-500 (el OOM killer mata
    # primero al page cache, no al servidor de inferencia).
    sudo tee "$f" >/dev/null 2>&1 <<'EOF'
{"name": "llama-server", "nice": -10, "sched": "batch", "ioclass": "best-effort", "ionice": 4, "oom_score_adj": -500}
{"name": "llama-bench", "nice": -5, "sched": "batch", "ioclass": "best-effort", "ionice": 4}
{"name": "llama-cli", "nice": -5, "sched": "batch", "ioclass": "best-effort", "ionice": 4}
EOF
    sudo systemctl restart ananicy-cpp 2>/dev/null || true
    _sp_aplicado "ananicy: llama-server con nice -10, sched batch y oom_score_adj -500"
}

# EL CMDLINE DEL KERNEL, EN EL BOOTLOADER QUE TENGA
_sp_rcu_lazy() {
    local flag="rcutree.enable_rcu_lazy=1"
    if grep -q "$flag" /proc/cmdline 2>/dev/null; then
        _sp_ya "rcutree.enable_rcu_lazy=1 (ya está en el cmdline activo)"
        return 0
    fi
    # 1) systemd-boot / mkinitcpio
    if [[ -f /etc/kernel/cmdline ]]; then
        if grep -q "rcutree.enable_rcu_lazy" /etc/kernel/cmdline; then
            _sp_ya "rcutree.enable_rcu_lazy=1 en /etc/kernel/cmdline (pendiente de reiniciar)"
            return 0
        fi
        _sp_record_file "/etc/kernel/cmdline"
        sudo sed -i "1 s/\$/ $flag/" /etc/kernel/cmdline
        sudo mkinitcpio -P >/dev/null 2>&1 || sudo dracut --force >/dev/null 2>&1 || true
        _sp_aplicado "rcutree.enable_rcu_lazy=1 en /etc/kernel/cmdline (systemd-boot) — REINICIA"
        return 0
    fi
    # 2) GRUB
    if [[ -f /etc/default/grub ]]; then
        if grep -q "rcutree.enable_rcu_lazy" /etc/default/grub; then
            _sp_ya "rcutree.enable_rcu_lazy=1 en /etc/default/grub (pendiente de reiniciar)"
            return 0
        fi
        _sp_record_file "/etc/default/grub"
        sudo python3 - "$flag" <<'PYEOF' 2>/dev/null
import re, sys
flag = sys.argv[1]
p = "/etc/default/grub"
s = open(p).read()
if "rcutree.enable_rcu_lazy" not in s:
    s = re.sub(r'(GRUB_CMDLINE_LINUX_DEFAULT=")([^"]*)(")',
               lambda m: m.group(1) + m.group(2) + " " + flag + m.group(3),
               s, count=1)
    open(p, "w").write(s)
PYEOF
        if grep -q "rcutree.enable_rcu_lazy" /etc/default/grub 2>/dev/null; then
            sudo update-grub >/dev/null 2>&1 || sudo grub-mkconfig -o /boot/grub/grub.cfg >/dev/null 2>&1 || true
            _sp_aplicado "rcutree.enable_rcu_lazy=1 en /etc/default/grub (GRUB) — REINICIA"
            return 0
        fi
        _sp_fallo "no se pudo escribir el cmdline de GRUB"
        return 0
    fi
    _sp_no "rcutree.enable_rcu_lazy=1 (no hay /etc/kernel/cmdline ni /etc/default/grub: bootloader desconocido)"
    _sp_nota "si usas otro bootloader, añade $flag a su línea de arranque a mano"
}

_sp_power_mizer() {
    # En un PORTÁTIL con TGP dinámico, GPUPowerMizerMode=1 no se queda: lo
    # negocia el firmware/driver. Y NO afecta al decode (el 100 % de GPU no
    # pasa por el PowerMizer). Se informa, no se卖家 como error.
    if ! cmd_exists nvidia-settings; then
        _sp_no "GPUPowerMizerMode (nvidia-settings no está instalado)"
        return 0
    fi
    if nvidia-settings -a "[gpu:0]/GPUPowerMizerMode=1" >/dev/null 2>&1; then
        _sp_aplicado "GPUPowerMizerMode=1 (preferir máximo rendimiento)"
    else
        _sp_nota "GPUPowerMizerMode=1 no se queda: en un portátil con TGP dinámico lo negocia el firmware (y no afecta al decode)"
    fi
}

# ── Informe ─────────────────────────────────────────────────────────────────
_llm_mem_report() {
    local mem_total mem_avail cached swap_total swap_free swappiness
    mem_total=$(LC_ALL=C awk '/^MemTotal:/{printf "%.1f GiB", $2/1048576}' /proc/meminfo 2>/dev/null || echo "?")
    mem_avail=$(LC_ALL=C awk '/^MemAvailable:/{printf "%.1f GiB", $2/1048576}' /proc/meminfo 2>/dev/null || echo "?")
    cached=$(LC_ALL=C awk '/^Cached:/{printf "%.1f GiB", $2/1048576}' /proc/meminfo 2>/dev/null || echo "?")
    swap_total=$(LC_ALL=C awk '/^SwapTotal:/{printf "%.1f GiB", $2/1048576}' /proc/meminfo 2>/dev/null || echo "?")
    swap_free=$(LC_ALL=C awk '/^SwapFree:/{printf "%.1f GiB", $2/1048576}' /proc/meminfo 2>/dev/null || echo "?")
    swappiness=$(cat /proc/sys/vm/swappiness 2>/dev/null || echo "?")
    log_info "  RAM total      : $mem_total"
    log_info "  RAM disponible : $mem_avail"
    log_info "  page cache     : $cached"
    log_info "  swap           : $swap_total (libre $swap_free, swappiness=$swappiness)"
    if [[ "$swap_total" != "0.0 GiB" && "$swap_free" != "$swap_total" ]]; then
        log_warn "  hay swap en uso: si el page cache se llena, el modelo se va a swap y el decode se desploma"
    fi
    log_info "  el GGUF (mmapeado) debe caber en la RAM: si no, el decode va al disco"
}

_sp_resumen() {
    printf '\n'
    log_info "═══ RESUMEN ═══"
    if (( ${#_SP_APLICADO[@]} )); then
        log_success "APLICADO (${#_SP_APLICADO[@]}):"
        printf '    %s\n' "${_SP_APLICADO[@]}"
    fi
    if (( ${#_SP_YA[@]} )); then
        log_info "YA ESTABA (${#_SP_YA[@]}):"
        printf '    %s\n' "${_SP_YA[@]}"
    fi
    if (( ${#_SP_NO_APLICA[@]} )); then
        log_info "NO APLICA (${#_SP_NO_APLICA[@]}):"
        printf '    %s\n' "${_SP_NO_APLICA[@]}"
    fi
    if (( ${#_SP_FALLO[@]} )); then
        log_warn "FALLÓ (${#_SP_FALLO[@]}):"
        printf '    %s\n' "${_SP_FALLO[@]}"
    fi
    if (( ${#_SP_NOTAS[@]} )); then
        printf '\n'
        log_info "NOTAS:"
        printf '    %s\n' "${_SP_NOTAS[@]}"
    fi
    printf '\n'
    if (( ${#_SP_FALLO[@]} == 0 )); then
        log_success "setup performance: todo aplicado o ya estaba en su sitio"
    else
        log_warn "setup performance: ${#_SP_FALLO[@]} ajuste(s) sin aplicar (ver arriba)"
    fi
    [[ -n "$SP_BACKUP" ]] && log_info "backup (para revertir): $SP_BACKUP"
}

# ── Modo check ───────────────────────────────────────────────────────────────
_sp_check() {
    printf '\n\033[1mEstado de los ajustes para motores locales\033[0m\n'
    local f v
    v="$(sysctl -n vm.swappiness 2>/dev/null || echo '?')"
    [[ "$v" == "5" ]] && log_ok "swappiness = $v (el page cache del GGUF no se va a swap)" \
        || log_warn "swappiness = $v (lo ideal es 5)"
    v="$(cat /proc/sys/vm/min_free_kbytes 2>/dev/null || echo 0)"
    (( v >= 1048576 )) && log_ok "min_free_kbytes = $(( v / 1024 )) MiB" \
        || log_warn "min_free_kbytes = $(( v / 1024 )) MiB (lo ideal ≥ 1024 MiB)"
    v="$(cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null || echo '?')"
    [[ "$v" == *"[always]"* ]] && log_ok "THP = $v" || log_warn "THP = $v (lo ideal: [always])"
    v="$(sysctl -n vm.max_map_count 2>/dev/null || echo '?')"
    [[ "$v" == "1048576" ]] && log_ok "max_map_count = $v" || log_warn "max_map_count = $v (lo ideal 1048576)"
    v="$(sysctl -n vm.page-cluster 2>/dev/null || echo '?')"
    [[ "$v" == "0" ]] && log_ok "page-cluster = $v" || log_warn "page-cluster = $v (lo ideal 0)"
    f="/etc/modprobe.d/nvidia-pm.conf"
    if [[ -f "$f" ]] && grep -q "0x02" "$f"; then log_ok "NVIDIA DPM fine-grained"; else log_warn "falta $f (DPM fine-grained)"; fi
    systemctl is-enabled nvidia-persistenced &>/dev/null && log_ok "nvidia-persistenced habilitado" \
        || log_warn "nvidia-persistenced no habilitado"
    grep -q "rcutree.enable_rcu_lazy" /proc/cmdline 2>/dev/null \
        && log_ok "rcutree.enable_rcu_lazy=1 activo" \
        || log_warn "rcutree.enable_rcu_lazy=1 no está en el cmdline activo"
    printf '\n'
    _llm_mem_report
    printf '\n'
    if command -v nvidia-smi >/dev/null 2>&1; then
        log_info "GPU: razones de throttle (vacías = la GPU no está frenada)"
        nvidia-smi -q -d PERFORMANCE 2>/dev/null | sed -n '/Clocks Event Reasons/,/^$/p' | sed 's/^/    /'
    fi
}

# ── Rollback ─────────────────────────────────────────────────────────────────
_sp_rollback() {
    local d="${1:-}"
    if [[ -z "$d" ]]; then
        d="$(ls -1dt "$SP_BACKUP_ROOT"/*/ 2>/dev/null | head -1)"
    fi
    [[ -n "$d" && -d "$d" ]] || { log_error "no hay backup que revertir"; return 1; }
    log_info "revirtiendo el backup: $d"
    if [[ -f "$d/files.tsv" ]]; then
        while IFS=$'\t' read -r _ ruta; do
            [[ "$ruta" == /* ]] || continue
            sudo rm -f "$ruta" && log_ok "eliminado $ruta"
        done <"$d/files.tsv"
    fi
    if [[ -f "$d/values.tsv" ]]; then
        while IFS=$'\t' read -r clave valor; do
            case "$clave" in
                sysctl.*)
                    sudo sysctl -w "${clave#sysctl.}=$valor" >/dev/null 2>&1 \
                        && log_ok "${clave#sysctl.} restaurado a $valor"
                    ;;
                sysfs.*)
                    local ruta="${clave#sysfs.}" v
                    v="${valor//\[always\]/always}"
                    printf '%s' "$v" | sudo tee "$ruta" >/dev/null 2>&1 \
                        && log_ok "$ruta restaurado a $v"
                    ;;
            esac
        done <"$d/values.tsv"
    fi
    sudo sysctl --system >/dev/null 2>&1 || true
    sudo udevadm control --reload-rules >/dev/null 2>&1 || true
    log_success "revertido (lo que no estaba en el backup se queda como estaba)"
}

# ── La función principal (para integrarlo en un setup.sh) ───────────────────
setup_performance() {
    local os
    os=$(detect_os) 2>/dev/null || os="linux"
    if [[ "$os" != "cachy" && "$os" != "arch" && "$os" != "wsl2" && "$os" != "linux" ]]; then
        log_info "performance: solo aplica a CachyOS / Arch / WSL2 (skip en $os)"
        return 0
    fi
    log_info "Aplicando optimizaciones para motores locales ($os)…"

    # UNA contraseña, cacheada: sudo -v la pide una vez y vale 15 min.
    # Si ya somos root, no hace falta (y puede que sudo ni esté instalado).
    if [[ "$(id -u)" != "0" ]]; then
        sudo -v >/dev/null 2>&1 || { log_error "hace falta sudo (autentícate y reintenta)"; return 1; }
    fi

    _sp_backup_init

    # Cada paso va en su propia función: un fallo en uno NO impide los demás.
    _sp_limits        || true
    _sp_sysctl        || true
    _sp_swappiness    || true
    _sp_min_free      || true
    _sp_thp           || true
    _sp_readahead     || true
    _sp_ananicy       || true
    _sp_nvidia_dpm    || true
    _sp_persistenced  || true
    _sp_rcu_lazy      || true
    _sp_power_mizer   || true

    # El manifest: lo que se hizo, en JSON (para el rollback y para el logs).
    if [[ -n "$SP_MANIFEST" ]]; then
        {
            printf '{"version":2,"date":"%s","applied":[' "$(date -Is)"
            local first=1 x
            for x in ${_SP_APLICADO[@]+"${_SP_APLICADO[@]}"}; do
                (( first )) || printf ','
                first=0
                printf '"%s"' "${x//\"/\'}"
            done
            printf '],"already":['
            first=1
            for x in ${_SP_YA[@]+"${_SP_YA[@]}"}; do
                (( first )) || printf ','
                first=0
                printf '"%s"' "${x//\"/\'}"
            done
            printf '],"not_applicable":['
            first=1
            for x in ${_SP_NO_APLICA[@]+"${_SP_NO_APLICA[@]}"}; do
                (( first )) || printf ','
                first=0
                printf '"%s"' "${x//\"/\'}"
            done
            printf '],"failed":['
            first=1
            for x in ${_SP_FALLO[@]+"${_SP_FALLO[@]}"}; do
                (( first )) || printf ','
                first=0
                printf '"%s"' "${x//\"/\'}"
            done
            printf ']}\n'
        } | sudo tee "$SP_MANIFEST" >/dev/null 2>&1
    fi

    _sp_resumen
    log_info "═══ Estado para motores locales ═══"
    _llm_mem_report
    log_info "reversible con: setup.sh perf-rollback  (o $0 --rollback)"
    return 0
}

setup_perf_rollback() {
    _sp_rollback "${1:-}"
}

# ── Modo autónomo ────────────────────────────────────────────────────────────
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    case "${1:-apply}" in
        --check)    _sp_check ;;
        --rollback) _sp_rollback "${2:-}" ;;
        apply|"")   setup_performance ;;
        *)          echo "uso: $0 [apply|--check|--rollback]" >&2; exit 2 ;;
    esac
fi
