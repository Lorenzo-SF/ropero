[SYSTEM]

Eres un **Principal/Staff Software Engineer** especializado en sistemas distribuidos, Elixir/OTP 28+, PostgreSQL, Phoenix, React + TypeScript, Rust, Bash, Docker y Linux.

Tu objetivo no es satisfacer literalmente cada instrucción si existe una alternativa técnicamente superior. Tu objetivo es producir la solución **más correcta, robusta, medible y mantenible posible**.

Principios obligatorios:

1. **Correctitud antes que complejidad.**
2. No introduzcas una tecnología, patrón o abstracción únicamente porque aparezca mencionada en el problema.
3. Si un requisito es técnicamente contraproducente, debes señalarlo y justificar una alternativa.
4. No inventes APIs, funciones, opciones de configuración ni librerías.
5. Todo el código debe ser completo y coherente entre archivos.
6. No uses `TODO`, `FIXME`, placeholders, pseudocódigo ni fragmentos omitidos.
7. El código debe poder compilarse/ejecutarse con las versiones indicadas.
8. Evita trabajo O(n) innecesario sobre ventanas de datos grandes.
9. Diseña explícitamente el comportamiento bajo saturación, errores, caídas de dependencias y recuperación.
10. Toda decisión relevante de rendimiento debe estar respaldada por una métrica, límite o razonamiento cuantificable.

Puedes asumir información que no esté especificada, pero debes:

* documentar cada suposición relevante;
* explicar cómo afecta al diseño;
* evitar asumir hardware o capacidad infinita.

No optimices prematuramente. Primero identifica los límites del sistema y después optimiza los verdaderos cuellos de botella.

---

[USER]

Diseña e implementa un sistema completo de **detección de anomalías de latencia para una plataforma de telemetría distribuida**.

El sistema debe soportar como objetivo inicial:

* 10.000 eventos/segundo por nodo.
* Hasta 100 nodos.
* Ventana de análisis de 60 segundos.
* Latencia de detección p99 < 200 ms.
* El sistema debe continuar aceptando tráfico durante degradaciones temporales del detector o PostgreSQL.
* La ingesta nunca debe quedar bloqueada esperando a que termine el análisis estadístico.

El objetivo es detectar incrementos anómalos de latencia por nodo y región y emitir alertas en tiempo casi real.

---

# 1. DATOS DE ENTRADA

Cada evento contiene:

```text
timestamp
node_id
region
latency_ms
request_id
```

Ejemplo:

```text
timestamp=2026-08-13T10:00:01.123Z
node_id=worker-1
region=eu-west
latency_ms=14
request_id=abc123
```

La distribución normal habitual de un nodo puede aproximarse a:

```text
media ≈ 15 ms
desviación estándar ≈ 5 ms
```

Durante un incidente puede aparecer algo similar a:

```text
15 ms
17 ms
14 ms
340 ms
1250 ms
2800 ms
3100 ms
...
```

El pico puede durar entre varios segundos y varios minutos.

---

# 2. OBJETIVOS FUNCIONALES

El sistema debe:

1. Recibir eventos de telemetría.
2. Persistir los datos necesarios para análisis histórico.
3. Detectar anomalías por `node_id`.
4. Detectar degradaciones por `region`.
5. Mantener una ventana estadística móvil de aproximadamente 60 segundos.
6. Detectar incrementos anómalos de latencia.
7. Emitir una alerta cuando se detecte una anomalía significativa.
8. Evitar generar miles de alertas idénticas durante un mismo incidente.
9. Recuperarse automáticamente cuando desaparezca el pico.
10. Permitir consultar el estado actual de las anomalías.
11. Exponer métricas suficientes para demostrar que el sistema cumple los objetivos de rendimiento.

---

# 3. REQUISITOS ESTADÍSTICOS

Debes diseñar una estrategia para detectar anomalías utilizando estadísticas móviles.

Como mínimo debe contemplar:

* media;
* desviación estándar;
* percentiles;
* puntuación z o una alternativa estadística robusta;
* ventana temporal;
* baseline;
* detección de cambios bruscos.

No estás obligado a utilizar exactamente z-score.

Si consideras que z-score es vulnerable a outliers contaminando la propia ventana, debes explicarlo y, si procede, utilizar una estrategia más robusta como:

* mediana;
* MAD;
* percentiles;
* baseline histórica;
* EWMA;
* combinación de varias señales.

La solución debe explicar:

1. cómo se calcula el baseline;
2. cómo se detecta un pico;
3. cómo se evita que el propio pico destruya el baseline;
4. cómo se determina el inicio del incidente;
5. cómo se determina su recuperación;
6. cómo se evita el alert storm.

---

# 4. ARQUITECTURA

Diseña la arquitectura completa antes de escribir el código.

Debes decidir y justificar:

* dónde ocurre la ingesta;
* dónde ocurre el análisis;
* qué datos permanecen en memoria;
* qué datos van a PostgreSQL;
* cuándo se utiliza Redis;
* si existe batching;
* cómo se controla el backpressure;
* cómo se limita la memoria;
* cómo se distribuye el trabajo;
* cómo se recupera el sistema después de una caída.

Puedes utilizar:

* GenServer;
* DynamicSupervisor;
* Task.Supervisor;
* Broadway;
* GenStage;
* ETS;
* PostgreSQL;
* Redis;
* PubSub;
* otros mecanismos de OTP;

pero **no estás obligado a utilizar ninguno de ellos**.

Selecciona la arquitectura que consideres adecuada y explica por qué.

---

# 5. REQUISITO CRÍTICO DE RENDIMIENTO

El detector debe poder procesar:

```text
10.000 eventos/s por nodo
100 nodos
= hasta 1.000.000 eventos/s globales
```

No es necesario demostrar que PostgreSQL puede almacenar indefinidamente 1M eventos/s.

Debes diferenciar claramente entre:

* ingestion path;
* hot path de detección;
* persistencia;
* almacenamiento histórico.

El hot path no debe depender de ejecutar una consulta costosa sobre toda la ventana histórica para cada evento.

Explica explícitamente la complejidad aproximada de tu estrategia:

```text
ingesta: O(...)
actualización del estado: O(...)
detección: O(...)
persistencia: O(...)
```

Identifica los posibles cuellos de botella.

---

# 6. BACKPRESSURE Y DEGRADACIÓN

El sistema debe tener un límite explícito de memoria y trabajo pendiente.

Cuando el detector no pueda mantener el ritmo de entrada:

1. la ingesta debe continuar;
2. debe activarse backpressure;
3. debe existir una política explícita para decidir qué trabajo se conserva;
4. pueden descartarse muestras de análisis si es necesario;
5. nunca deben acumularse colas infinitas;
6. deben registrarse métricas de pérdida;
7. el sistema debe recuperarse cuando disminuya la carga.

Debes diferenciar:

* pérdida de datos de análisis;
* pérdida de datos persistidos;
* pérdida de alertas.

Las alertas críticas no deben perderse simplemente porque se hayan descartado muestras estadísticas.

Define métricas para:

```text
queue_depth
events_received_total
events_processed_total
events_dropped_total
alerts_emitted_total
detector_lag_ms
detection_latency_ms
db_latency_ms
db_pool_queue_ms
memory_usage_bytes
scheduler_utilization
```

Añade cualquier otra métrica que consideres necesaria.

---

# 7. POSTGRESQL

PostgreSQL será el almacenamiento principal.

Diseña:

* esquema;
* índices;
* particionado si lo consideras necesario;
* retención;
* estrategia de inserción;
* consultas;
* pool de conexiones.

Debes explicar por qué cada índice existe y qué consulta acelera.

No hagas que una consulta SQL procese innecesariamente cientos de miles o millones de filas para cada evento.

Puedes utilizar:

* window functions;
* CTE;
* materialized views;
* tablas agregadas;
* hypertables si introduces una extensión;
* otras técnicas;

pero debes justificar la elección.

Si consideras que una CTE recursiva, una window function o cualquier otra técnica solicitada explícitamente sería contraproducente, dilo.

---

# 8. ELIXIR / OTP

Implementa un backend Phoenix completo.

Debe incluir como mínimo:

```text
lib/
  telemetry/
    anomaly_detector.ex
    anomaly_state.ex
    ingestion.ex
    metrics.ex

  telemetry_web/
    channels/
      telemetry_channel.ex
    controllers/
    router.ex
```

Puedes modificar esta estructura si tienes una razón técnica.

El sistema debe utilizar supervisión OTP correctamente.

Los procesos deben tener:

* límites claros;
* estrategia de reinicio;
* aislamiento de fallos;
* timeouts;
* protección frente a memory leaks.

No utilices procesos GenServer como simples contenedores de estado gigante si existe una alternativa mejor.

---

# 9. TELEMETRY

Implementa instrumentación real utilizando `:telemetry`.

Como mínimo:

```text
[:telemetry, :ingestion]
[:telemetry, :detector]
[:telemetry, :detector, :anomaly]
[:telemetry, :persistence]
[:telemetry, :pubsub]
```

Las métricas deben permitir responder a:

* ¿estamos procesando 10k eventos/s?
* ¿cuánto tarda la detección?
* ¿cuánto trabajo está pendiente?
* ¿estamos descartando muestras?
* ¿PostgreSQL está introduciendo latencia?
* ¿el scheduler está saturado?
* ¿la memoria está creciendo?
* ¿las alertas están llegando tarde?

No utilices mecanismos de introspección extremadamente costosos en el hot path.

Si necesitas mecanismos como `:sys.get_state/1` para debugging, utilízalos únicamente donde sea apropiado y explica sus costes.

---

# 10. PHOENIX / PUBSUB

Expón las anomalías mediante Phoenix Channels o WebSocket.

El cliente debe recibir eventos similares a:

```json
{
  "type": "latency_anomaly",
  "node_id": "worker-2",
  "region": "eu-west",
  "timestamp": "2026-08-13T10:05:01Z",
  "latency_ms": 2800,
  "baseline_ms": 15,
  "severity": "critical",
  "score": 12.4
}
```

El sistema debe soportar múltiples clientes sin que un cliente lento bloquee a los demás.

Explica cómo manejas:

* clientes lentos;
* desconexiones;
* reconexiones;
* duplicados;
* orden de eventos;
* burst de alertas.

---

# 11. FRONTEND

Implementa React + TypeScript funcional.

Debe incluir:

```text
src/
  hooks/
    useAnomalyStream.ts
  components/
    LatencyChart.tsx
    AnomalyList.tsx
  App.tsx
```

El hook debe soportar:

* WebSocket;
* reconexión;
* exponential backoff;
* jitter;
* límite de reintentos;
* buffer circular;
* deduplicación;
* eventos fuera de orden.

Mantén como máximo los últimos 100 eventos para visualización.

El frontend no debe intentar renderizar miles de eventos por segundo.

Si recibes más eventos de los que el navegador puede representar:

* conserva los datos relevantes;
* descarta datos antiguos o de menor prioridad;
* muestra que existe pérdida;
* evita bloquear el main thread.

Utiliza `React.memo`, `useMemo` y `useCallback` únicamente cuando tengan sentido.

No los añadas como decoración.

---

# 12. VISUALIZACIÓN

Implementa `LatencyChart`.

Debe mostrar:

* latencia;
* baseline;
* anomalías;
* severidad;
* timestamps;
* pérdida de eventos.

No es necesario utilizar una librería de gráficos concreta.

Si introduces una dependencia, explica por qué.

---

# 13. INFRAESTRUCTURA

Proporciona Docker Compose para:

```text
PostgreSQL
Redis
Backend
Frontend/Nginx
```

Elasticsearch puede utilizarse opcionalmente para almacenamiento histórico, pero **no puede formar parte del hot path de detección**.

Configura PostgreSQL de forma razonable.

No asumas que:

```text
shared_buffers=1GB
max_connections=200
```

son automáticamente configuraciones óptimas.

Justifica cualquier tuning.

El backend debe poder ejecutarse mediante release de Elixir.

Incluye:

```text
.env.example
docker-compose.yml
Dockerfiles necesarios
```

---

# 14. BENCHMARK

Implementa:

```text
benchmark.sh
```

El benchmark debe generar durante al menos 10 minutos:

1. carga estable;
2. ruido estadístico;
3. picos repentinos;
4. picos prolongados;
5. recuperación;
6. múltiples nodos;
7. diferentes regiones;
8. carga superior a la capacidad del detector.

Debe producir métricas para:

```text
input events/s
processed events/s
dropped events/s
CPU
RAM
database latency
detector latency
alert latency
queue depth
false positives
false negatives
precision
recall
```

El benchmark debe conocer de antemano dónde se produjeron los incidentes para poder calcular:

```text
true positive
false positive
true negative
false negative
```

Calcula:

```text
precision
recall
F1
p50 detection latency
p95 detection latency
p99 detection latency
```

---

# 15. TESTS

Implementa tests completos.

Como mínimo:

### Unitarios

* cálculo estadístico;
* detección de anomalías;
* recuperación;
* hysteresis;
* deduplicación;
* backpressure.

### Integración

* PostgreSQL;
* ingestion → detector;
* detector → PubSub;
* WebSocket.

### Property-based / generativos

Si consideras que aportan valor, utilízalos para validar invariantes como:

* ningún evento negativo;
* timestamps monotónicos donde corresponda;
* bounded memory;
* no duplicación de alertas;
* recuperación del detector.

---

# 16. FAILURE MODES

La solución debe explicar y, cuando sea razonable, probar:

### PostgreSQL caído

¿Qué ocurre?

### Redis caído

¿Qué ocurre?

### Detector saturado

¿Qué ocurre?

### Cliente WebSocket lento

¿Qué ocurre?

### Pico de 100x

¿Qué ocurre?

### Reinicio del proceso detector

¿Qué información se pierde?

### Reinicio del nodo

¿Cómo se recupera?

### Clock skew

¿Cómo afecta a la ventana temporal?

---

# 17. REVISIÓN CRÍTICA

Después de implementar todo el sistema, realiza una auditoría de tu propia solución.

Busca activamente:

* race conditions;
* deadlocks;
* memory leaks;
* mailbox growth;
* problemas de scheduler;
* N+1 queries;
* índices incorrectos;
* consultas demasiado caras;
* conexiones PostgreSQL insuficientes;
* PubSub bottlenecks;
* WebSocket backpressure;
* renders excesivos;
* dependencias innecesarias;
* APIs inexistentes;
* configuraciones incompatibles;
* supuestos incorrectos;
* contradicciones entre componentes;
* cualquier requisito que no hayas cumplido.

No ocultes problemas encontrados.

Si descubres un defecto durante la auditoría, **corrígelo en el código final**.

---

# 18. FORMATO DE RESPUESTA

La respuesta debe tener exactamente estas secciones y en este orden:

```text
ARCHITECTURE

ASSUMPTIONS

PERFORMANCE ANALYSIS

BACKEND CODE

FRONTEND CODE

INFRA CODE

TESTS

BENCHMARK

SELF-AUDIT

FINAL TRADE-OFFS
```

En `BACKEND CODE`, `FRONTEND CODE`, `INFRA CODE` y `TESTS` proporciona **archivos completos**.

No omitas archivos necesarios para que el proyecto funcione.

No escribas:

```text
...
```

para representar código omitido.

No escribas:

```text
// resto del código
```

ni:

```text
# etc.
```

ni:

```text
TODO
```

Si la implementación completa supera razonablemente el límite de salida, prioriza una arquitectura ejecutable y coherente y declara explícitamente qué parte no has podido completar. **Nunca presentes código incompleto como si fuera completo.**

---

# 19. CRITERIO PRINCIPAL

No se evalúa únicamente cuánto código produces.

Se evaluará, en este orden:

1. **Correctitud**
2. **Arquitectura**
3. **Concurrencia y tolerancia a fallos**
4. **Rendimiento**
5. **Calidad del código**
6. **Coherencia entre backend, frontend e infraestructura**
7. **Tests**
8. **Capacidad de detectar y corregir tus propios errores**

Una solución más pequeña y técnicamente sólida es preferible a una implementación enorme llena de abstracciones innecesarias.

Si debes elegir entre obedecer literalmente una instrucción y producir una solución técnicamente correcta, **explica el conflicto y elige la solución técnicamente correcta**. 