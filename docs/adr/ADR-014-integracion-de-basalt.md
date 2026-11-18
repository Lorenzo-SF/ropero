# ADR-014: Integración de Basalt

- Estado: aceptado
- Fecha: 2026-11-18

## Contexto

Basalt es un servidor local para packs `.basalt`, optimizado para GPU NVIDIA
Blackwell y distribuido como archive de release que incluye el ejecutable, el
servidor Python, herramientas y datos. El servidor acepta APIs OpenAI y
Anthropic Messages. El pack oficial de Qwen3.8-Flash-Next ocupa alrededor de
122 GiB.

Ropero ya permite declarar engines mediante `EngineId` y un manifiesto, pero
sus formatos de artefacto existentes no describen este fichero monolítico.
Clasificarlo como GGUF, safetensors o ninfer falsearía la ruta y el contrato de
descarga.

## Decisiones

1. Añadir `basalt` como id canónico y formato de modelo independiente.
2. Guardar packs bajo `ARTIFACTS_DIR` con una ruta relativa en
   `[model].path`; descargarlos como fichero único con el downloader `hf` y
   staging atómico.
3. Lanzar el servidor directo con `basalt serve <pack>`, alias explícito,
   `--host 127.0.0.1`, argumentos declarados por modelo y el puerto asignado
   por Ropero.
4. Mantener la autenticación desactivada. Basalt sólo documenta `--api-key`
   por línea de comandos; Ropero no expone secretos en `argv`. Rechazar una
   clave explícita y forzar loopback.
5. Marcar el engine como `integrated`, no `proven`: el entorno de desarrollo
   no tiene Basalt ni una GPU Blackwell para ejecutar una prueba real.
6. Ropero enlaza una instalación de Basalt que el usuario extrajo y preparó.
   No descarga el archive, instala CUDA ni crea el entorno virtual Python.

## Compatibilidad

El manifiesto exige Linux y presencia de NVIDIA como preflight básico. Basalt
requiere además Blackwell (sm_120), CUDA 13, Python 3.10+ y RAM suficiente;
Basalt comprueba requisitos específicos al arrancar. Ropero no infiere ni
estima la RAM/VRAM requerida desde el tamaño del pack.

## Consecuencias

- El catálogo incluye un TOML de Qwen3.8-Flash-Next con `manual = true`: se
  puede instalar por nombre, pero no forma parte de las instalaciones masivas.
- La guía de compatibilidad documenta cómo colocar el archive, instalar sus
  requisitos Python, enlazar el binario y descargar el pack.
- La gestión de credenciales y exposición remota queda fuera de este adapter;
  usar el endpoint local no requiere una clave.
