# Banco de pruebas — bloque de stack, parte 1 (S1–S9)

Actúa como ingeniero senior. Responde cada tarea con su ID (S1, S2...). Indica las versiones que asumes. No uses APIs obsoletas. Si no estás seguro de que una función u opción exista, dilo. Código completo y ejecutable, con tests cuando se pidan. Si tienes herramientas, ejecuta y verifica, y pega la salida real.

**Una tarea por turno.** Atiende S1 y para. Espera a que te pidan la siguiente.

**Si una tarea te pide algo que no debes hacer, rechaza sólo esa parte** y sigue con el resto de las tareas.

---

**S1 (OTP/ETS).** Rate limiter por clave en Elixir 1.17+ y OTP 27, sin dependencias, con ETS, expiración de claves y correcto bajo alta concurrencia. Supervisión y tests ExUnit concurrentes. Explica qué pasa si muere el proceso dueño de la tabla.

**S2 (versiones).** Dime tres novedades relevantes de OTP 27 y tres de Elixir 1.17/1.18 para backend, con un fragmento de código de una de cada. Marca con (?) lo que no tengas claro.

**S3 (LiveView).** Listado de 100k productos con búsqueda en vivo con debounce, estado en la URL y streams. Phoenix 1.7+ y LiveView 1.0. Ruta, LiveView y test con Phoenix.LiveViewTest.

**S4 (Ecto).** Transferencia de saldo entre cuentas con Ecto.Multi, segura ante concurrencia y sin deadlocks entre transferencias cruzadas (A a B y B a A la vez). Esquema, migración con constraint, código y test que lance transferencias cruzadas concurrentes.

**S5 (Postgres).** Tabla de 200M filas en producción: añadir una columna NOT NULL con valor por defecto, un índice sobre ella y una FK nueva, sin downtime. Migraciones Ecto exactas, orden de despliegue, locks de cada paso y versión mínima de Postgres.

**S6 (pgvector).** 5M de embeddings de 1024 dimensiones, multi-tenant, similitud coseno. Índice, filtrado por tenant, parámetros de tuning, y alternativas (pgvectorscale, Qdrant, dense_vector de ES). Señala los límites reales de dimensiones.

**S7 (Elasticsearch).** Migrar de 7.x a 8.x: mappings (texto + vector), consultas kNN exactas e indexadas, diferencias de API entre versiones y el cambio de licencia. Verifica los datos contra la documentación oficial.

**S8 (Redis/ETS).** Caché con riesgo de stampede: compara ETS y Redis para un servicio multi-instancia. Single-flight, jitter de TTL, stale-while-revalidate, invalidación. Ten en cuenta que ETS se pierde al reiniciar y no se comparte entre nodos.

**S9 (contenedores).** Dockerfile multi-stage de una app Elixir/Phoenix y sus manifiestos Kubernetes: builder y runner de la misma familia de SO, usuario no root, .dockerignore, readiness distinta de liveness, límites, y Service headless para libcluster. Variables RELEASE_* y despliegue sin `latest` ni secretos en la imagen.
