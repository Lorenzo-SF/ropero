# MODELS — Capacidades, aliases y routing de modelos

## Concepto

Los agentes usan **aliases**, no nombres de provider/model. MODELS.md mapea
aliases a provider/model reales. El modelo de decisión (d3:turbo) usa este
mapeo para routing.

## Aliases

| Alias | Tier | Provider/Model actual | Fallback |
|---|---|---|---|
| `premium` | Máxima capacidad | claude/opus-5-5 (ICA) | zai-org/glm-5-3 |
| `fast` | Velocidad sobre capacidad | minimax/m3.1-flash (ICA) | flm/lfm2.5 |
| `local` | GPU local, sin rate limits | marichocho/puertourraco | premium |
| `edge` | NPU, ultra-rápido | flm/lfm2.5 | fast |
| `decision` | Solo decisiones (NPU) | flm/d3:turbo | edge |

## Capacidades por alias

### premium
- **Fortalezas**: razonamiento complejo, código, análisis, síntesis
- **Debilidades**: lento, costoso, límite de rate
- **Usar para**: arquitectura, debugging profundo, revisión, decisiones irreversibles
- **No usar para**: tareas simples, chat casual, clasificación

### fast
- **Fortalezas**: muy rápido, barato, buen chat
- **Debilidades**: razonamiento limitado, no ideal para código complejo
- **Usar para**: chat rápido, clasificación, resúmenes, tareas simples
- **No usar para**: arquitectura, debugging, código complejo

### local
- **Fortalezas**: sin coste, sin rate limits, privacidad, modelos grandes
- **Debilidades**: requiere GPU libre, consumo energético
- **Usar para**: razonamiento pesado, código, análisis de documentos largos
- **No usar para**: cuando la GPU está ocupada

### edge
- **Fortalezas**: 50-80 t/s, carga instantánea, bajo consumo
- **Debilidades**: 1.2B params, razonamiento limitado
- **Usar para**: Mavis (interfaz), clasificación, routing, chat simple
- **No usar para**: código complejo, razonamiento, análisis

### decision
- **Fortalezas**: clasificación multi-clase, routing, guardrails
- **Debilidades**: solo decisiones, no genera texto
- **Usar para**: routing de peticiones, selección de modelo, guardrails
- **No usar para**: generación de texto

## Fallback chain

1. **Primario**: alias asignado por d3:turbo
2. **Fallback 1**: fast (minimax/m3.1-flash)
3. **Fallback 2**: edge (flm/lfm2.5, NPU)
4. **Fallback 3**: local (marichocho/puertourraco, GPU)

## Routing rules (agnóstico a provider/model)

| Tipo de tarea | Alias primario | Fallback |
|---|---|---|
| Chat simple, saludos | edge | fast |
| Código simple (<50 líneas) | fast | edge |
| Código complejo (>50 líneas) | premium | local |
| Arquitectura, ADR | premium | local |
| Debugging | local | premium |
| Análisis de documentos largos | local | premium |
| Clasificación/routing | decision | edge |
| Visión/OCR | (pendiente: gemma4e-flash:e4b en NPU) | premium |
| Traducción | (pendiente: translategemma en NPU) | fast |

## Flujo de routing

```
1. Usuario envía mensaje a Mavis
2. Mavis (edge) clasifica la intención básica
3. Mavis envía la clasificación a decision (d3:turbo, NPU, /v1/systemone)
4. decision selecciona el alias según MODELS.md
5. Mavis resuelve el alias a provider/model y delega
6. Si falla, fallback chain
```

## Compact mapping para d3 (entrada a /v1/systemone)

```json
{
  "aliases": ["premium", "fast", "local", "edge", "decision"],
  "tasks": {
    "chat_simple": "edge",
    "code_simple": "fast",
    "code_complex": "premium",
    "architecture": "premium",
    "debugging": "local",
    "docs_long": "local",
    "classification": "decision",
    "vision": "premium",
    "translation": "fast"
  },
  "fallback": ["fast", "edge", "local"]
}
```
