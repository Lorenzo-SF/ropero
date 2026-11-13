# ADR-001 · Por qué Rust

**Estado**: aceptada
**Fecha**: 2026-11-12

## Contexto
ropero v1 está escrito en bash y Python: un dispatcher de ~200 líneas, módulos con carga diferida y un parser de TOML propio. Funciona, pero el arranque de motores de inferencia es lento y opaco, el estado en disco es frágil y distinguir «mi motor» de «un motor ajeno» (P-3) es difícil de garantizar en bash.

## Decisión
Reescribir el núcleo de ropero en Rust, manteniendo la CLI pública y el estado en disco compatibles.

## Alternativas consideradas
- Mantener bash/Python — no da tipado ni tests rápidos del dominio.
- Reescribir en Go — viable, pero Rust da control de memoria y un único binario estático sin runtime.
- Reescribir en Python — no resuelve el problema de distribución ni de procesos.

## Consecuencias
Se gana un dominio tipado y testeable en milisegundos; se pierde la facilidad de editar scripts en caliente; el estado sigue siendo `CLAVE=valor` para compatibilidad.
