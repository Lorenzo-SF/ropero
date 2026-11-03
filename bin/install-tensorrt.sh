#!/usr/bin/env bash
# =============================================================================
# install-tensorrt.sh — Docker + NVIDIA Container Toolkit + TensorRT-LLM
# =============================================================================
# Opciones de shell: SÓLO `pipefail`.
#
# SIN `errexit` (-e) NI `nounset` (-u): un fallo implícito no aborta el
# script; los abortos críticos son EXPLÍCITOS (`|| { ...; exit 1; }`) y sí lo
# terminan, así que la secuencia se detiene en el primer fallo CON un mensaje
# claro. Este script SE EJECUTA, no se sourcea: su `exit` mata tu shell.
# NO reintroducir errexit ni nounset (`pipefail` no puede terminar una shell).
# =============================================================================
set -o pipefail

# ============================================================
# 1. Instalar Docker y NVIDIA Container Toolkit en CachyOS
# ============================================================
sudo pacman -Syu --needed --noconfirm \
  docker \
  docker-compose \
  nvidia-container-toolkit \
  nvidia-utils \
  || { echo "ERROR: pacman no pudo instalar los paquetes (docker, nvidia-container-toolkit)" >&2; exit 1; }

# ============================================================
# 2. Habilitar Docker y configurar el runtime de NVIDIA
# ============================================================
sudo systemctl enable --now docker \
  || { echo "ERROR: no se pudo habilitar ni arrancar docker" >&2; exit 1; }

# Añadir tu usuario al grupo docker (permite usar docker sin sudo)
# Cierra sesión y vuelve a entrar después de ejecutar esto.
#
# $USER no siempre está definido (cron, systemd, una shell sin login): sin
# valor por defecto, `usermod -aG docker ""` fallaría con un mensaje que no
# dice nada útil. Se usa el usuario real del proceso.
DOCKER_USER="${USER:-$(id -un 2>/dev/null)}"
if [[ -z "$DOCKER_USER" ]]; then
    echo "ERROR: no se pudo determinar el usuario actual (define USER o ejecuta esto en una shell con sesión)" >&2
    exit 1
fi
sudo usermod -aG docker "$DOCKER_USER" \
  || { echo "ERROR: no se pudo añadir '$DOCKER_USER' al grupo docker" >&2; exit 1; }

# Configurar el runtime de NVIDIA para Docker
sudo nvidia-ctk runtime configure --runtime=docker \
  || { echo "ERROR: no se pudo configurar el runtime de NVIDIA para Docker" >&2; exit 1; }

# Reiniciar Docker para aplicar el runtime
sudo systemctl restart docker \
  || { echo "ERROR: no se pudo reiniciar docker" >&2; exit 1; }

# ============================================================
# 3. Descargar la imagen oficial de TensorRT-LLM
#    Versión 1.4.0rc2 – soporte Blackwell / NVFP4
# ============================================================
docker pull nvcr.io/nvidia/tensorrt-llm/release:1.4.0rc2 \
  || { echo "ERROR: no se pudo descargar la imagen de TensorRT-LLM" >&2; exit 1; }

# ============================================================
# 4. Lanzar el contenedor interactivo
#    - Monta ~/modelos en /models (ajusta la ruta si quieres)
#    - Expone el puerto 8000 para la API OpenAI-compatible
# ============================================================
mkdir -p "$HOME/modelos" \
  || { echo "ERROR: no se pudo crear $HOME/modelos" >&2; exit 1; }

docker run --rm -it \
  --name trtllm \
  --ipc host \
  --gpus all \
  --ulimit memlock=-1 \
  --ulimit stack=67108864 \
  -p 8000:8000 \
  -v "$HOME/modelos:/models" \
  nvcr.io/nvidia/tensorrt-llm/release:1.4.0rc2
rc=$?
if (( rc != 0 )); then
    echo "ERROR: el contenedor de TensorRT-LLM terminó con código $rc" >&2
    exit "$rc"
fi

# ============================================================
# Dentro del contenedor puedes verificar que la GPU está visible:
#   nvidia-smi
#   python3 -c "import tensorrt_llm; print('OK')"
# ============================================================
