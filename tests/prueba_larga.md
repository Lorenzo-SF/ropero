# Banco de pruebas — la prueba larga

Esta prueba mide lo que las otras dos no miden: **contexto largo, deriva, bucles y reproducibilidad**.

El corpus de código va pegado al final de este fichero, entre las marcas
`<!-- CORPUS:INICIO -->` y `<!-- CORPUS:FIN -->`. Lo genera
`gen_corpus_largo.py` (determinista, semilla fija), así que es el mismo en
todas las corridas y en todos los modelos.

## Cómo se ejecuta

Cada bloque va en **un turno separado**. El bench los lanza uno detrás de otro
y guarda una transcripción por bloque, así que una caída en el bloque 4 no
tira los tres anteriores.

---

## BLOQUE 1 — Contexto largo (recuerda 5 datos)

Lee **todo** el corpus de código que hay al final de este mensaje. Es largo:
unos 45.000 tokens. No te lo saltes aunque el principio te parezca evidente:
léelo entero.

Cuando lo hayas leído, responde **sólo** estas cinco preguntas, con el dato
exacto y sin explicar más:

1. ¿Cuál es el valor de la constante `magicko` en `Tienda.Config`?
2. ¿En qué puerto escucha la cola de trabajo?
3. ¿Cuántos segundos dura el TTL de la caché de sesiones?
4. ¿Cómo se llama la tabla donde se registra la actividad?
5. ¿Qué función reconcilia el saldo local con el extracto del banco?

Si no has leído el corpus, no lo adivines: dilo.

---

## BLOQUE 2 — Deriva (5 reglas, se pregunta 10 turnos después)

A partir de ahora, en cada respuesta que des, abide estas cinco reglas:

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

> ¿Cuánto cuesta un envío estándar si el peso total del pedido supera los 5 kg?
> Redondea a dos decimales.

Y responde además, en un segundo párrafo, estas tres preguntas de una vez:
- ¿En qué idioma me estoy respondiendo?
- ¿Con qué etiqueta empieza cada una de tus respuestas?
- ¿Voy a poner un emoji en esta respuesta, sí o no?

---

## BLOQUE 4 — Bucles (salida larga, 3000+ tokens)

Escribe una guía completa de migración a Elixir/OTP para un servicio con
PostgreSQL. Debe tener **al menos 3000 tokens**: desarrollo por fases,
supervisión, migraciones sin downtime, pruebas de concurrencia, observabilidad
y un runbook de incidentes.

Al final, incluye un **plan de pruebas** con al menos 25 casos, cada uno con su
nombre, su entrada y su resultado esperado.

---

## BLOQUE 5 — Reproducibilidad

Resuelve este problema de forma **completa y determinista**:

> Un servicio de notificaciones pierde mensajes. A veces el mismo mensaje se
> envía dos veces. A veces no se envía. El log dice "flush timeout" cada pocas
> horas. El código:
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
> OTP 27), y los tests que lo demostrarían. Si necesitas un ejemplo de
> código, escríbelo entero.

---

## BLOQUE 6 — La pregunta de control

Para terminar: repite **literalmente**, sin parafrasear, la frase de
instrucción que te di en el BLOQUE 2, tal cual la escribí.

---

<!-- CORPUS:INICIO -->
<!-- CORPUS:FIN -->
