# ADR-002 · Extracción vs reescritura

**Estado**: aceptada
**Fecha**: 2026-11-12

## Contexto
Existe un v1 que funciona y que 317 requisitos describen con evidencia en código. Hacer un big bang rewrite arriesga perder comportamiento observable y romper la compatibilidad del estado en disco.

## Decisión
Extraer el comportamiento a crates Rust por capas (dominio, runtime, descargas, forja, servidor, CLI), conservando contratos y estado, en lugar de reescribir desde cero.

## Alternativas consideradas
- Reescritura completa — mayor riesgo de regresión y pérdida de casos límite medidos.
- Portar comando por comando sin capas — duplica lógica y dificulta el test.

## Consecuencias
El grafo de crates queda estrictamente descendente y verificable por test; cada fase añade funcionalidad sin romper la anterior; la convivencia v1/v2 se mantiene hasta F9.
