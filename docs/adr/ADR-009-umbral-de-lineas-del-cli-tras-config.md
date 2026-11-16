# ADR-009 · El umbral de líneas del CLI, tras configuración externa, bootstrap y `remove`

**Estado**: aceptada
**Fecha**: 2026-11-15 (actualizada 2026-11-16)

## Contexto

[ADR-007](ADR-007-umbral-de-lineas-del-cli-tras-gguffer.md) fijó el umbral de
`ropero-cli` en **1200** líneas. Las etapas parciales 1-3 de la nueva
especificación introdujeron bootstrap/configuración/catálogo externo y elevaron
el umbral a **1600** líneas.

La incorporación de `ropero <modelo> remove` (subcomando de modelo con plan,
confirmación interactiva y dispatch dedicado) añade superficie de UI en la CLI;
la lógica de decisión vive en `ropero-core` y la ejecución/E/S en
`ropero-runtime`.

## Decisión

El umbral de `ropero-cli` pasa de **1200** a **1600** y, tras `remove`, a
**1700** líneas.

## Consecuencias

- El CLI sigue siendo un dispatcher: sin lógica de dominio ni E/S propia más
  allá de la frontera de presentación.
- El margen vuelve a ser acotado para obligar a que el crecimiento futuro se
  resuelva en `ropero-core`/`ropero-runtime`.
- La regla estructural del test de arquitectura se mantiene con el nuevo
  límite.