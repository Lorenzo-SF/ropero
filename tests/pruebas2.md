Actúa como ingeniero senior. Responde cada tarea con su ID (S1, S2...). Indica las versiones que asumes. No uses APIs obsoletas. Si no estás seguro de que una función u opción exista, dilo. Código completo y ejecutable, con tests cuando se pidan. Si tienes herramientas, ejecuta y verifica, y pega la salida real.

**S1 (OTP/ETS).** Rate limiter por clave en Elixir 1.17+ y OTP 27, sin dependencias, con ETS, expiración de claves y correcto bajo alta concurrencia. Supervisión y tests ExUnit concurrentes. Explica qué pasa si muere el proceso dueño de la tabla.

**S2 (versiones).** Dime tres novedades relevantes de OTP 27 y tres de Elixir 1.17/1.18 para backend, con un fragmento de código de una de cada. Marca con (?) lo que no tengas claro.

**S3 (LiveView).** Listado de 100k productos con búsqueda en vivo con debounce, estado en la URL y streams. Phoenix 1.7+ y LiveView 1.0. Ruta, LiveView y test con Phoenix.LiveViewTest.

**S4 (Ecto).** Transferencia de saldo entre cuentas con Ecto.Multi, segura ante concurrencia y sin deadlocks entre transferencias cruzadas (A a B y B a A a la vez). Esquema, migración con constraint, código y test que lance transferencias cruzadas concurrentes.

**S5 (Postgres).** Tabla de 200M filas en producción: añadir una columna NOT NULL con valor por defecto, un índice sobre ella y una FK nueva, sin downtime. Migraciones Ecto exactas, orden de despliegue, locks de cada paso y versión mínima de Postgres.

**S6 (pgvector).** 5M embeddings de 1024 dimensiones, multi-tenant, similitud coseno con filtro de tenant y recall alto. Elige índice y parámetros con justificación; DDL, consulta y cómo ajustarías recall y latencia. Compara con dos alternativas y di cuándo cambiarías.

**S7 (Elasticsearch).** Búsqueda híbrida (texto + vectorial) en Elasticsearch 7.x y en 8.x: mappings, consultas y diferencias de API. Explica el cambio de licencia y sus consecuencias prácticas.

**S8 (Redis vs ETS).** Caché con protección contra stampede en Elixir, en dos versiones: ETS local y Redis (Redix). Cuándo usarías cada una, qué pasa en un deploy rolling o un reinicio, y qué cambió en la licencia de Redis.

**S9 (contenedores).** Dockerfile multi-stage para una release de Phoenix (usuario no root, imagen final pequeña, compatibilidad glibc entre etapas) y manifiestos de Kubernetes: Deployment con probes, límites y apagado ordenado, más clustering BEAM con libcluster sobre un Service headless. Variables RELEASE\_\* necesarias.

**S10 (AWS).** Política IAM de mínimo privilegio para que un servicio suba ficheros a S3 con URLs prefirmadas, limitado a un prefijo por tenant; política del bucket (sin acceso público, TLS obligatorio) y código Elixir para generar la URL.

**S11 (Bash).** Script de backup de PostgreSQL a S3: pg_dump formato custom, rotación de 7 días, lock para no solapar ejecuciones, logs, salida limpia ante errores y reintentos. Debe pasar shellcheck sin avisos y no imprimir credenciales. Ejecuta shellcheck.

**S12 (React funcional TS).** Hook useDebouncedSearch\<T> con AbortController, sin condiciones de carrera, genérico, sin any y compatible con StrictMode. Test con Testing Library que demuestre que una respuesta antigua no pisa a una nueva.

**S13 (React clásico).** Conviértelo a componente funcional con hooks y señala las diferencias sutiles de comportamiento y los fallos que encuentres:

```jsx
class Feed extends React.Component {
  state = { items: [], loading: false };
  componentDidMount() {
    this.load();
    window.addEventListener("focus", this.load);
  }
  componentDidUpdate(prev) {
    if (prev.userId !== this.props.userId) this.load();
  }
  componentWillUnmount() {
    window.removeEventListener("focus", this.load);
  }
  load = async () => {
    this.setState({ loading: true });
    const r = await fetch(`/api/feed?u=${this.props.userId}`);
    this.setState({ items: await r.json(), loading: false });
  };
  render() {
    /* ... */
  }
}
```

**S14 (NIFs).** Similitud coseno sobre binarios de f32 (1536 dimensiones, lotes de 10k) llamada desde Elixir. Impleméntala con Rustler y explica planificadores dirty, por qué no bloquear el scheduler, copia vs zero-copy y manejo de errores. Compara con una NIF en C y con un Port. ¿Qué harías si el código fuera Go?

**S15 (móvil).** Pantalla de lista con paginación y modo offline-first: a) Kotlin nativo (Jetpack Compose, Room, Paging 3, Flow); b) React Native con TypeScript. Código de las capas principales, cómo evitas recomposiciones o renders innecesarios y ciclo de vida.

**S16 (depuración OTP).** Este GenServer acumula elementos y los envía a una API. En producción se pierden datos y a veces se cuelga. Encuentra todos los problemas, explica cada uno y reescríbelo bien.

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

**S17 (seguridad).** Revisa este controlador Phoenix, lista las vulnerabilidades por gravedad y corrígelo:

```elixir
def search(conn, %{"q" => q, "sort" => sort}) do
  field = String.to_atom(sort)
  {:ok, res} = Repo.query("SELECT * FROM users WHERE name LIKE '%#{q}%' ORDER BY #{field}")
  json(conn, res.rows)
end
```

**S18 (integral, agente).** En un directorio vacío, crea un proyecto Phoenix 1.7+ mínimo: GET /health que compruebe la BD, un contexto Ecto Notes con CRUD y validaciones, tests, Dockerfile y docker-compose con Postgres. Ejecuta `mix test` y `docker build`, corrige hasta que pasen y resume qué hiciste. No inventes salidas: pega las reales.
