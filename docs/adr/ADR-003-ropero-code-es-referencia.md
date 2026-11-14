# ADR-003 · `ropero-code` es referencia, no dependencia

**Estado**: aceptada
**Fecha**: 2026-11-14

## Contexto
La v1 (`ropero-code/`, Bash + Python) funciona, pero arrastra contratos que la v2 quiere rehacer: si la v2 se desarrolla leyendo la v1, cada decisión acaba heredando su forma. El catálogo tampoco puede seguir siendo el de la v1: `ropero validate` sobre `ropero-code/models` daba seis violaciones.

## Decisión
`ropero-code/` es **sólo referencia** (aprender, consultar, contrastar, comparar): todo el desarrollo de la v2 se hace **como si esa carpeta no existiera**. El catálogo de la v2 es **nuevo e independiente**, vive en `models/` (raíz del workspace) y **no se integra en el ejecutable**.

## Alternativas consideradas
- Seguir con `ropero-code/models` — arrastra las seis violaciones y ata la v2 a los datos de la v1.
- Portar los TOML tal cual — congela los typos (`context = 133072`) y el nombre de fichero que no casa con `name`.
- Integrar el catálogo de la v1 en el binario — convierte la v1 en dependencia de runtime.

## Consecuencias
El discovery de la v2 busca `models/` por ancestrales y **no** mira `ropero-code/`; la v1 se consulta a mano, nunca como fuente de verdad. El catálogo nuevo valida `OK: 45 modelo(s) validados` con exit 0. La convivencia v1/v2 sigue hasta F9 (ADR-002).
