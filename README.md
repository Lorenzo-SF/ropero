# ropero

Gestor de modelos locales: lee modelos declarados en TOML, arranca el motor
adecuado en un puerto que le pertenece, espera a que esté de verdad listo y
expone una única puerta compatible con la API de OpenAI.

> **Este repo está en reorganización.** El contenido se ha movido a dos
> carpetas para poder consultarlo con claridad:
>
> | Carpeta | Qué contiene |
> |---|---|
> | [`ropero-code/`](ropero-code/) | **El código**: el launcher, los motores, el catálogo de modelos, la batería de tests y el instalador. |
> | [`docs-especificacion/`](docs-especificacion/) | **La documentación**: los dos documentos que especifican el sistema, en Markdown y en PDF. |
>
> Esta rama (`feat/estructura-en-carpetas`) es sólo eso: el mismo código, en
> otra disposición. El comportamiento no cambia — ver
> [`docs-especificacion/`](docs-especificacion/) para la especificación
> completa.

## La documentación

Son dos documentos que se leen **juntos**, y cada uno es normativo en su
ámbito:

| Documento | Responde a | Formato |
|---|---|---|
| [Especificación y requerimientos](docs-especificacion/ropero-especificacion.md) | **¿Qué hace ropero y con qué semántica exacta?** 317 requisitos, cada uno con su evidencia (`fichero:línea`) en el código actual. | [PDF](docs-especificacion/ropero-especificacion.pdf) · 52 págs. |
| [Especificación de construcción](docs-especificacion/ropero-construccion.md) | **¿Cómo se construye?** Stack con versiones exactas, arquitectura en 6 crates, metodología, Definition of Done de los 317 requisitos y plan de 48 tareas en 10 fases. | [PDF](docs-especificacion/ropero-construccion.pdf) · 115 págs. |

**Regla de conflicto**: si discrepan sobre *qué debe hacer el sistema*, manda
el primero. Si discrepan sobre *cómo construirlo*, manda el segundo.

## Empezar

```bash
git clone https://github.com/Lorenzo-SF/ropero.git
cd ropero/ropero-code
bash install.sh --local
```

Eso deja `~/.local/bin/ropero → <repo>/ropero-code/bin/ropero` (lo único en el
PATH) y `~/.local/share/ropero → <repo>/ropero-code/bin`.

```bash
ropero install setup      # dependencias base (python3, git, uv, hf, jq, cmake…)
ropero install engines    # elige motores e instala SUS dependencias
ropero install models     # elige modelos (descarga o fabrica el GGUF)
```

`ropero install` sin argumentos hace las tres cosas en orden.

El resto de la guía de uso está en [`ropero-code/README.md`](ropero-code/README.md).
