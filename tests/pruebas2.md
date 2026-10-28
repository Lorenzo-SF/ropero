# Prueba 2 — backend: Elixir/ETS, Postgres, ES, Redis, Docker, AWS, Bash,

# Rust, Go (B1–B15)

Actúa como ingeniero senior. **Haz todas las tareas en orden, una detrás de
otra, en esta misma respuesta.** Cada una empieza con su ID (B1, B2…).

Indica las versiones que asumes. No uses APIs obsoletas. Si no estás seguro
de que una función u opción exista, dilo. Código completo y ejecutable, con
tests cuando se pidan. Si tienes herramientas, ejecuta y verifica y pega la
salida real. Si una tarea pide algo que no debes hacer, rechaza sólo esa
parte.

---

**B1 (Elixir/OTP/ETS).** Rate limiter por clave en Elixir 1.17+ y OTP 27,
sin dependencias, con ETS, expiración de claves y correcto bajo alta
concurrencia. Supervisión y tests ExUnit concurrentes. Explica qué pasa si
muere el proceso dueño de la tabla.

**B2 (versiones).** Tres novedades relevantes de OTP 27 y tres de Elixir
1.17/1.18 para backend, con un fragmento de código de una de cada. Marca con
(?) lo que no tengas claro.

**B3 (Postgres).** Tabla de 200M filas en producción: añadir una columna
NOT NULL con valor por defecto, un índice sobre ella y una FK nueva, sin
downtime. Migraciones Ecto exactas, orden de despliegue, locks de cada paso
y versión mínima de Postgres.

**B4 (pgvector).** 5M de embeddings de 1024 dimensiones, multi-tenant,
similitud coseno. Índice, filtrado por tenant, parámetros de tuning y
alternativas (pgvectorscale, Qdrant, dense_vector de ES). Señala los límites
reales de dimensiones.

**B5 (Elasticsearch).** Migrar de 7.x a 8.x: mappings (texto + vector),
consultas kNN exactas e indexadas, diferencias de API entre versiones y el
cambio de licencia. Verifica contra la documentación oficial.

**B6 (Redis/ETS).** Caché con riesgo de stampede: compara ETS y Redis para un
servicio multi-instancia. Single-flight, jitter de TTL, stale-while-revalidate,
invalidación. Ten en cuenta que ETS se pierde al reiniciar y no se comparte
entre nodos.

**B7 (contenedores).** Dockerfile multi-stage de una app Elixir/Phoenix y sus
manifiestos Kubernetes: builder y runner de la misma familia de SO, usuario
no root, `.dockerignore`, readiness distinta de liveness, límites y Service
headless para libcluster. Variables `RELEASE_*` y despliegue sin `latest` ni
secretos en la imagen.

**B8 (AWS).** Política IAM mínima para que un servicio suba ficheros a S3 con
URLs prefirmadas, con prefijo por tenant. Bucket policy, TLS, expiración
corta y la función de firma real en Elixir. Señala qué fija el código y qué
no.

**B9 (Bash).** Script de backup de PostgreSQL a S3: `pg_dump` en formato
custom, credenciales por entorno sin imprimirlas, rotación, protección contra
ejecuciones solapadas y reintentos. Pásale shellcheck y pega su salida real.

**B10 (seguridad).** Revisa este controlador Phoenix, lista las
vulnerabilidades por gravedad y corrígelo:

```elixir
def search(conn, %{"q" => q, "sort" => sort}) do
  field = String.to_atom(sort)
  {:ok, res} = Repo.query("SELECT * FROM users WHERE name LIKE '%#{q}%' ORDER BY #{field}")
  json(conn, res.rows)
end
```

**B11 (depuración OTP).** Este GenServer acumula elementos y los envía a una
API. En producción se pierden datos y a veces se cuelga. Encuentra todos los
problemas, explica cada uno y reescríbelo bien.

```elixir
def handle_cast({:add, item}, state) do
  Task.start(fn -> GenServer.call(__MODULE__, :flush) end)
  {:noreply, [item | state]}
end

def handle_call(:flush, _from, state) do
  Enum.each(state, &send_to_api/1)
  {:reply, :ok, []}
end
```

**B12 (Rust NIF).** Similitud coseno sobre binarios de f32 (1536 dimensiones,
lotes de 10k) llamada desde Elixir. Impleméntala con Rustler y explica
planificadores dirty, por qué no bloquear el scheduler, copia vs zero-copy y
manejo de errores. Compara con una NIF en C y con un Port. ¿Qué harías si el
código fuera Go?

**B13 (Go).** Worker pool concurrente en Go con:

- N workers, cola con buffer, cierre ordenado.
- Contexto con cancelación y shutdown graceful.
- Backpressure y errores sin perder trabajo.
- Un test con `-race` que lo demuestre.

**B14 (Ecto.Multi).** Transferencia de saldo entre cuentas con Ecto.Multi,
segura ante concurrencia y sin deadlocks entre transferencias cruzadas
(A→B y B→A a la vez). Esquema, migración con constraint, código y test
concurrente real.

**B15 (LiveView).** Listado de 100k productos con búsqueda en vivo con
debounce, estado en la URL y streams. Phoenix 1.7+ y LiveView 1.0. Ruta,
LiveView y test con `Phoenix.LiveViewTest`.
