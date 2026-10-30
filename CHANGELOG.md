# Changelog

## 1.0.0 — 2026-10-30

Refactor completo del monolito a módulos. Mismo producto, una sola fuente de
verdad por pieza.

### Añadido
- `ropero test <modelo>`: baterías `prueba1..prueba_gorda` con opencode,
  confirmación previa y markdown autocontenido (juez + prueba + log + JSON +
  sesión).
- `ropero status <modelo>` (exit 0/1/2), `ropero doctor` (alias `check`),
  `ropero --version`.
- `install engines` / `install models` con selección interactiva; sin TTY
  exigen selección explícita (nunca instalan todo por accidente).
- `[download.aux.*]` para mtp, mmproj, chat_template, imatrix, tokenizer y
  draft; `[build]` integrado con `ropero gguffer`.
- Engines `mlx-serve` (macOS) y `vllm`; los motores sin server propio
  (airllm, TensorRT-LLM, FastFlowLM) van por `bin/server/endpoint.py`.
- TOMLs de `metalnemotron` y `metalqwen`.
- Suite offline `tests/suite/run.sh` con fakes (sin red ni GPU).

### Cambiado
- `bin/ropero` es un dispatcher fino; la lógica vive en `bin/lib/`.
- Un motor = `bin/lib/engines/<id>.sh` con interfaz de 8 hooks.
- `quant/` → `bin/quant/`; `endpoints/server.py` → `bin/server/endpoint.py`.
- `install.sh` sólo obtiene ropero (tarball/clone HTTPS, `--version`, `--dir`,
  symlink); las dependencias son `ropero install setup`.
- Clone del motor por HTTPS; API key de mlx-serve vía `--api-key-env` (nunca
  en argv); `--cpu` sólo en Linux.
- Objetivo de sintaxis bash 3.2 (sin namerefs ni arrays asociativos).

### Corregido
- `ropero gguffer` ya no trata como error una cuantización ya aplicada en el
  repo: coloca el GGUF y sale 0 sin borrar nada.
- `ropero_install_mlx` con la lógica invertida; choques alias↔nombre;
  `status` sin dependencia obligatoria de `alaja`.
- Código muerto eliminado (`_authproxy.py`, hooks sin llamada, iGPU).

### Eliminado
- Tests artesanales 0.x (`test-*.sh`, `bench_*`, `probar-*`), `ropero.d/`,
  `prueba.json` y el monolito legado.
