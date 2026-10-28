# Prueba 3 — frontend y sistemas: JS/TS/React/RN, Kotlin, Dart/Flutter,

# C, C++, Rust, patrones (F1–F15)

Actúa como ingeniero senior. **Haz todas las tareas en orden, una detrás de
otra, en esta misma respuesta.** Cada una empieza con su ID (F1, F2…).

Indica versiones. No uses APIs obsoletas. Código completo y ejecutable, con
tests cuando se pidan. Si tienes herramientas, ejecuta y verifica. Si una
tarea pide algo que no debes hacer, rechaza sólo esa parte.

---

**F1 (TypeScript/React).** Hook `useDebouncedSearch<T>` con `AbortController`
en el cleanup del effect, que ignore `AbortError`, genérico y sin `any`, con
los estados `idle/loading/error/success`, y un test que resuelva las
respuestas **en orden inverso** de verdad.

**F2 (React clásico→funcional).** Convierte este componente de clase a
funcional. Detecta la condición de carrera al cambiar `userId`, el `setState`
tras desmontar, la falta de manejo de errores y los cierres obsoletos al
replicar el listener de `focus` con hooks.

```jsx
class Feed extends React.Component {
  componentDidMount() {
    window.addEventListener("focus", this.load);
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

**F3 (React performance).** Lista de 100.000 productos con búsqueda en vivo,
filtros y selección múltiple. Sin lag. Explica virtualización, memoización,
`useDeferredValue`/`useTransition` y qué NO optimizarías. Da el código
principal.

**F4 (React Native).** Pantalla de lista con paginación y modo offline-first
en TypeScript: caché local (MMKV o SQLite), `useInfiniteQuery`, FlashList o
FlatList, keyExtractor, cómo evitas renders innecesarios y ciclo de vida.

**F5 (Kotlin/Compose).** Pantalla de lista con paginación y modo offline-first:
Jetpack Compose, Room como fuente de verdad, Paging 3 con `RemoteMediator`,
`collectAsStateWithLifecycle`, `LazyColumn` con key. Explica qué evitas con
cada decisión.

**F6 (Flutter).** Misma pantalla en Flutter: paginación + offline-first,
`ListView.builder`, isolate para parseo pesado, persistencia (sqflite o
drift), y cómo evitas rebuilds.

**F7 (Dart).** Diferencias entre `Future`, `Stream`, `isolate` y el event
loop. Ejemplo: descargar 1000 ítems y procesarlos sin bloquear la UI.

**F8 (JavaScript).** Event loop, microtareas vs macrotareas. Orden de salida
exacto de este código y por qué:

```js
console.log("A");
setTimeout(() => console.log("B"), 0);
Promise.resolve().then(() => console.log("C"));
queueMicrotask(() => console.log("D"));
console.log("E");
```

**F9 (JavaScript).** Closures, hoisting, `this` y prototype. Corrige el bug
de este código y explica por qué falla:

```js
function Contador() {
  this.n = 0;
  setInterval(function () {
    this.n++;
  }, 1000);
}
const c = new Contador();
```

**F10 (C).** Implementa un ring buffer genérico en C99, con capacidad
configurable, sin allocations después de `init`, thread-safe con `pthread`
(SPSC y MPSC), y pásale valgrind/ASan. Da el código, el Makefile y la salida
real de ASan.

**F11 (C++).** Implementa una cola thread-safe en C++20 con `std::mutex` y
`std::condition_variable`, sin busy-wait, con shutdown limpio. Explica move
semantics, RAII y smart pointers en las partes que uses. Tests con
`std::thread` y `-fsanitize=thread`.

**F12 (Rust).** Ownership, lifetimes, `Result` vs `panic`. Escribe un parser
de CSV streaming que:

- No cargue todo el fichero en memoria.
- Devuelva `Result<Row, ParseError>` sin `unwrap()`.
- Uses `BufRead` y `Iterator`.
- Un test con un CSV de 100k líneas generado en runtime.

**F13 (testing JS/TS).** Configura Vitest + Testing Library para un
componente. Pruebas:

- Un test de integración con `msw`.
- Un test de propiedad (fast-check) para una función pura.
- Un test e2e mínimo con Playwright.

**F14 (patrones JS/TS).** Aplica tres patrones de diseño (factory, observer,
strategy o los que elijas) a un sistema de notificaciones multi-canal
(email/SMS/push). Justifica por qué cada patrón y qué problema resuelve.

**F15 (Flutter state management).** BLoC vs Riverpod vs Provider para una app
con auth, listas paginadas y websockets. Tradeoffs reales, cuándo elegirías
cada uno, y un esqueleto funcional del que elijas.

````

---

## `prueba_larga.md`

```markdown
# La prueba larga — contexto largo, deriva, bucles, reproducibilidad
# (L1–L6)

Cada bloque va en un turno separado. El banco los lanza uno detrás de otro.
Una caída en el bloque 4 no tira los tres anteriores.

---

## BLOQUE 1 — Contexto largo (recuerda 5 datos)

Lee **todo** el corpus de código que viene al final de este bloque. Es largo.
No te lo saltes aunque el principio te parezca evidente: léelo entero.

Cuando lo hayas leído, responde **sólo** estas cinco preguntas, con el dato
exacto y sin explicar más:

1. ¿Cuál es el valor de la constante `magicko` en `Tienda.Config`?
2. ¿En qué puerto escucha la cola de trabajo?
3. ¿Cuántos segundos dura el TTL de la caché de sesiones?
4. ¿Cómo se llama la tabla donde se registra la actividad?
5. ¿Qué función reconcilia el saldo local con el extracto del banco?

Si no has leído el corpus, no lo adivines: dilo.

<!-- CORPUS:INICIO -->
<!-- CORPUS:FIN -->

---

## BLOQUE 2 — Deriva (5 reglas, se pregunta 10 turnos después)

A partir de ahora, en cada respuesta que des, cumple estas cinco reglas:

1. Responde siempre en español, nunca en inglés.
2. Empieza cada respuesta con la etiqueta `RESPUESTA:`.
3. Nunca uses emojis.
4. Si te piden un número, redondéalo a dos decimales.
5. No menciones la palabra "regla" en tus respuestas.

Acepto estas reglas y las seguiré durante toda la conversación.

---

## BLOQUE 3 — La pregunta, 10 turnos después

Ya han pasado 10 turnos desde que aceptaste las reglas. Responde a esta
pregunta **sin mirar hacia atrás**, en un único mensaje:

> ¿Cuánto cuesta un envío estándar si el peso total del pedido supera los
> 5 kg? Redondea a dos decimales.

Y responde además, en un segundo párrafo, estas tres preguntas de una vez:
- ¿En qué idioma me estás respondiendo?
- ¿Con qué etiqueta empieza cada una de tus respuestas?
- ¿Vas a poner un emoji en esta respuesta, sí o no?

---

## BLOQUE 4 — Bucles (salida larga, 3000+ tokens)

Escribe una guía completa de migración a Elixir/OTP para un servicio con
PostgreSQL. Debe tener **al menos 3000 tokens**: desarrollo por fases,
supervisión, migraciones sin downtime, pruebas de concurrencia, observabilidad
y un runbook de incidentes.

Al final, incluye un **plan de pruebas** con al menos 25 casos, cada uno con
su nombre, su entrada y su resultado esperado.

---

## BLOQUE 5 — Reproducibilidad

Resuelve este problema de forma **completa y determinista**:

> Un servicio de notificaciones pierde mensajes. A veces el mismo mensaje se
> envía dos veces. A veces no se envía. El log dice "flush timeout" cada
> pocas horas. El código:
>
> ```elixir
> def handle_cast({:notify, user, body}, state) do
>   Task.start(fn -> deliver(user, body) end)
>   {:noreply, [notification(user, body) | state]}
> end
>
> def handle_call(:flush, _from, state) do
>   Enum.each(state, &deliver/1)
>   {:reply, :ok, []}
> end
> ```
>
> Explica la causa raíz, el diseño correcto (con las APIs exactas de Elixir y
> OTP 27) y los tests que lo demostrarían. Si necesitas un ejemplo de código,
> escríbelo entero.

---

## BLOQUE 6 — La pregunta de control

Para terminar: repite **literalmente**, sin parafrasear, la frase de
instrucción que te di en el BLOQUE 2, tal cual la escribí.
````
