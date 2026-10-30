#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# 1. Instalar Docker y NVIDIA Container Toolkit en CachyOS
# ============================================================
sudo pacman -Syu --needed --noconfirm \
  docker \
  docker-compose \
  nvidia-container-toolkit \
  nvidia-utils

# ============================================================
# 2. Habilitar Docker y configurar el runtime de NVIDIA
# ============================================================
sudo systemctl enable --now docker

# Añadir tu usuario al grupo docker (permite usar docker sin sudo)
# Cierra sesión y vuelve a entrar después de ejecutar esto.
sudo usermod -aG docker "$USER"

# Configurar el runtime de NVIDIA para Docker
sudo nvidia-ctk runtime configure --runtime=docker

# Reiniciar Docker para aplicar el runtime
sudo systemctl restart docker

# ============================================================
# 3. Descargar la imagen oficial de TensorRT-LLM
#    Versión 1.4.0rc2 – soporte Blackwell / NVFP4
# ============================================================
docker pull nvcr.io/nvidia/tensorrt-llm/release:1.4.0rc2

# ============================================================
# 4. Lanzar el contenedor interactivo
#    - Monta ~/modelos en /models (ajusta la ruta si quieres)
#    - Expone el puerto 8000 para la API OpenAI-compatible
# ============================================================
mkdir -p "$HOME/modelos"   # crea la carpeta local si no existe

docker run --rm -it \
  --name trtllm \
  --ipc host \
  --gpus all \
  --ulimit memlock=-1 \
  --ulimit stack=67108864 \
  -p 8000:8000 \
  -v "$HOME/modelos:/models" \
  nvcr.io/nvidia/tensorrt-llm/release:1.4.0rc2

# ============================================================
# Dentro del contenedor puedes verificar que la GPU está visible:
#   nvidia-smi
#   python3 -c "import tensorrt_llm; print('OK')"
# ============================================================
