# Banco de pruebas — bloque de stack, parte 2 (S10–S18)

Actúa como ingeniero senior. Responde cada tarea con su ID (S10, S11...). Indica las versiones que asumes. No uses APIs obsoletas. Si no estás seguro de que una función u opción exista, dilo. Código completo y ejecutable, con tests cuando se pidan. Si tienes herramientas, ejecuta y verifica, y pega la salida real.

**Una tarea por turno.** Atiende S10 y para. Espera a que te pidan la siguiente.

**Si una tarea te pide algo que no debes hacer, rechaza sólo esa parte** y sigue con el resto de las tareas.

---

**S10 (AWS).** Política IAM mínima para que un servicio suba ficheros a S3 con URLs prefirmadas, con prefijo por tenant. Bucket policy, TLS, expiración corta y la función de firma real en Elixir. Señala qué lo fija el código y qué no.

**S11 (Bash).** Script de backup de PostgreSQL a S3: `pg_dump` en formato custom, credenciales por entorno sin imprimirlas, rotación, protección contra ejecuciones solapadas y reintentos. Pásale shellcheck y pega su salida real.

**S12 (React TS).** Hook `useDebouncedSearch<T>` con AbortController en el cleanup del effect, que ignore AbortError, genérico y sin `any`, con los estados idle/loading/error/success, y un test que resuelva las respuestas **en orden inverso** de verdad.

**S13 (React clásico→funcional).** Convierte este componente de clase a funcional. Detecta la condición de carrera al cambiar `userId`, el `setState` tras desmontar, la falta de manejo de errores y los cierres obsoletos al replicar el listener de `focus` con hooks.

```jsx
class Feed extends React.Component {
  componentDidMount() {
    window.addEventListener('focus', this.load);
  }
  componentWillUnmount() {
    window.removeEventListener('focus', this.load);
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
