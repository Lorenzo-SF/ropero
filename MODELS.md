# MODELS — Capacidades y routing de modelos

## Modelos disponibles

### claude/opus-5-5
- **Tier**: Premium (máxima capacidad)
- **Fortalezas**: razonamiento complejo, código, análisis, síntesis
- **Debilidades**: lento, costoso, límite de rate
- **Usar para**: arquitectura, debugging profundo, revisión de código, decisiones irreversibles
- **No usar para**: tareas simples, chat casual, clasificación

### zai-org/glm-5-3
- **Tier**: Premium (alternativa a Claude)
- **Fortalezas**: razonamiento, código, multilingüe (chino/inglés)
- **Debilidades**: latencia, puede tener rate limits
- **Usar para**: cuando Claude no está disponible, tareas de código complejas
- **No usar para**: tareas simples, chat casual

### minimax/m3.1-flash
- **Tier**: Fast (velocidad sobre capacidad)
- **Fortalezas**: muy rápido, barato, buen chat
- **Debilidades**: razonamiento limitado, no ideal para código complejo
- **Usar para**: chat rápido, clasificación, resúmenes, tareas simples
- **No usar para**: arquitectura, debugging, código complejo

### marichocho/puertourraco (llama.cpp local)
- **Tier**: Local (GPU, sin límites de rate)
- **Fortalezas**: sin coste, sin rate limits, privacidad, modelos grandes
- **Debilidades**: requiere GPU libre, consumo energético
- **Usar para**: razonamiento pesado, código, análisis de documentos largos
- **No usar para**: cuando la GPU está ocupada

### flm/lfm2.5 (NPU)
- **Tier**: Edge (ultra-rápido, baja potencia)
- **Fortalezas**: 50-80 t/s, carga instantánea, bajo consumo
- **Debilidades**: 1.2B params, razonamiento limitado
- **Usar para**: Mavis (interfaz), clasificación, routing, chat simple
- **No usar para**: código complejo, razonamiento, análisis

### flm/d3:turbo (NPU)
- **Tier**: Decision (especializado)
- **Fortalezas**: clasificación multi-clase, routing, guardrails
- **Debilidades**: solo decisiones, no genera texto
- **Usar para**: routing de peticiones, selección de modelo, guardrails
- **No usar para**: generación de texto

## Fallback chain

1. **Primario**: modelo asignado por d3:turbo
2. **Fallback 1**: minimax/m3.1-flash (rápido, siempre disponible)
3. **Fallback 2**: flm/lfm2.5 (NPU, siempre disponible, sin rate limits)
4. **Fallback 3**: marichocho/puertourraco (GPU local, si está libre)

## Routing rules

| Tipo de tarea | Modelo primario | Fallback |
|---|---|---|
| Chat simple, saludos | flm/lfm2.5 | minimax/m3.1-flash |
| Código simple (<50 líneas) | minimax/m3.1-flash | flm/lfm2.5 |
| Código complejo (>50 líneas) | claude/opus-5-5 | zai-org/glm-5-3 |
| Arquitectura, ADR | claude/opus-5-5 | zai-org/glm-5-3 |
| Debugging | marichocho/puertourraco | claude/opus-5-5 |
| Análisis de documentos largos | marichocho/puertourraco | claude/opus-5-5 |
| Clasificación/routing | flm/d3:turbo | flm/lfm2.5 |
| Visión/OCR | (pendiente: gemma4e-flash:e4b en NPU) | claude/opus-5-5 |
| Traducción | (pendiente: translategemma en NPU) | minimax/m3.1-flash |

## Flujo de routing

```
1. Usuario envía mensaje a Mavis
2. Mavis (lfm2.5) clasifica la intención básica
3. Mavis envía la clasificación a d3:turbo (NPU, /v1/systemone)
4. d3:turbo selecciona el modelo según MODELS.md
5. Mavis delega al modelo seleccionado
6. Si falla, fallback chain
```
