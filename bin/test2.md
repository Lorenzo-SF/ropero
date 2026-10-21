Verificado PR C. **El diseño es bueno — el overhead de 370 MiB como titular `:system` resuelve mi objeción de la capacidad—, pero hay un bug que impide arrancar cualquier modelo de GPU.** Y hay una segunda cosa, más de fondo, que es la que más me importa.

**Lo que pasa:**

```
$ Candil.Engine.start(engine, :coder, 0)
{:error, :unknown_cost}
```

Cero modelos de GPU arrancan. Y no es un caso raro: es **todos**.

---

## El bug · `:infinity` no es un número

`Arrea.Resource.acquire/4` valida los dos importes:

```elixir
defp cost_error(cost, policy_cost) do
  cond do
    not is_number(cost) or not is_number(policy_cost) -> :unknown_cost
```

Y `Candil.Engine.Resource.acquire/4` pasa `:infinity` como cuota:

```elixir
def acquire(device_index, holder, cost, quota \\ :infinity)
```

Verificado:

```
ACQUIRE: {:error, :unknown_cost}
ACQUIRE con quota 0: {:ok, %{capacity: 13000, holder: {:coder, 9999}, quota: 0}}
```

**El arreglo es una palabra**: la cuota de una GPU local es **0** — «este modelo no consume cuota» es exactamente el caso que el `@doc` de Arrea dice que es una afirmación legítima («`policy_cost` es un número `>= 0`, porque…»). Y el `:infinity` no existe en esta API: no hay un «sin límite» en el eje blando, sino que se dice 0.

Y el modo de fallo es el peor: el `acquire` falla, `Engine.start/3` entra en `{:error, reason} -> {:error, reason}` y **el arranque se aborta**. No es que la reserva falle y siga: **el modelo no arranca nunca**.

Como la suite está en verde (1310 tests, 0 fallos, 4 skipped), esto no lo caza ningún test: los que pasan por el arranque real son los de máquina, y están **skipped** en CI.

---

## La segunda cosa, que es la de fondo · la reserva reserva 512 MiB de 14707

La reserva inicial usa **la estimación**, porque el acquire va antes de `fit-params` (que es lo correcto: serializa los arranques concurrentes).

Pero la estimación es:

```elixir
defp estimate_model_size(%Candil.Model{context_size: ctx}), do: div(ctx, 1024) * 4
```

Y medido:

```
estimate_model_size(ctx=131072) = 512
estimate_model_size(ctx=4096)   = 16
```

**512 MiB** para un modelo que ocupa **14707 MiB** (medido con `llama-fit-params`: `mem = 14707 MiB`). Y `coder` **no tiene `vram_mb`** en el catálogo, así que usa la estimación: `vram_mb: nil`.

O sea que la reserva inicial es **28 veces menor que la realidad**, y por tanto **nunca rechaza nada**: con 15933 MiB libres, 512 siempre cabe. La reserva no previene el OOM; solo cuenta.

Luego el código corrige: `release` + `acquire(weight)`. Y ahí está el segundo agujero:

```elixir
Candil.Engine.Resource.release(device, holder)
Candil.Engine.Resource.acquire(device, holder, weight, :infinity)   # ← el resultado se descarta
```

**El resultado del segundo `acquire` se tira.** Si falla con `:insufficient_capacity` —que es justo lo que pasa cuando hay dos arranques concurrentes, porque los dos parten de la misma memoria libre—, el código **sigue arrancando el motor sin ninguna reserva**: se liberó la primera y la segunda no se concedió. El motor arranca, hace OOM, y la cuenta del device dice que hay 14707 MiB libres.

**Ese es el agujero que D-27 quiere cerrar y que el PR C abre.**

---

## Lo que propongo, y son tres cosas

**1 · La cuota es 0, no `:infinity`.** Una palabra, y desbloquea todos los arranques.

**2 · El resultado del segundo `acquire` no se descarta.** Si falla, **no se arranca**: se devuelve el error, o se reintenta en otro device (que es la opción C de D-27, y que ya está decidida). Con el resultado ignorado, el PR C no cierra el problema que dice cerrar.

**3 · La reserva inicial no puede ser 512 MiB cuando el modelo ocupa 14707.** Dos opciones, y las dos son válidas:

- **(a) `llama-fit-params` antes del acquire.** Pero entonces se pierde la serialización, que es justo lo que arregla el acquire. Salvo que se haga una **reserva previa** con un margen generoso —por ejemplo el `vram_mb` declarado, o un 20 % por encima de la estimación— y se ajuste después con el peso medido. Es una reserva **optimista con margen**, y el ajuste posterior es el que importa.

- **(b) Que la estimación sea menos absurda.** `div(ctx, 1024) * 4` da 512 para 131072 de contexto. No es una estimación de VRAM: es un cuarto de MiB por cada 1024 tokens de contexto, que no corresponde a nada. Una estimación **del KV cache** (que sí depende del contexto) sería `contexto × bytes_por_token`, y los bytes por token salen de la arquitectura… que es la opción (a) del plan, el parser de GGUF.

**Mi preferencia: (a)**, con `vram_mb` como reserva inicial **obligatorio** para los modelos de GPU, y `llama-fit-params` como el que da el peso real. Y si `vram_mb` no está, **fallar con un mensaje que lo diga** —no reservar 512 MiB en silencio, que es el mismo patrón de `typical_latency_s` y `vram_mb` que ya arreglasteis dos veces.

---

## Y el punto que más me importa, sobre el test

El test de máquina `resource_machine_test.exs` verifica que «un arranque fallido libera la reserva». **Eso es justo el camino que hoy no se puede ejecutar**, porque el arranque falla antes con `:unknown_cost`.

Es decir: el test que se ha escrito para cerrar la puerta está **al otro lado de la puerta cerrada**. Por eso los 4 skipped no son un detalle menor: **son el PR C sin verificar**, y el código que verifican tiene un bug que solo aparece cuando se ejecutan.

**La secuencia que propongo, y es la que da la puerta de verdad:**

1. Arreglar la cuota (0 en vez de `:infinity`).
2. Arreglar el resultado del segundo `acquire`.
3. **Ejecutar los tests de máquina** y que el de la reserva liberada pase de verdad.
4. Solo entonces, PR.

Y hay una puerta más antes, que es la que te propuse el otro día y que sigue sin hacerse: **arrancar un modelo del catálogo con los args que Candil genera hoy, y comprobar que carga**. Con la GPU llena por el `gemma` de otra sesión no lo he podido hacer, y por eso mi comprobación de `:no_room` fue inválida. **Necesito la GPU libre para verificar el PR C**, o un modelo pequeño que quepa en los 85 MiB que quedan (no hay ninguno).

---

## Una nota sobre `terminate/2`

El `release` de `Server.terminate/2` y el del camino de error de `Engine.start/3` **pueden llegar a liberar dos veces** con el mismo holder. Hoy es inocuo (el segundo `release` de un titular que no está devuelve ceros), pero si un mismo `{alias, port}` reservara dos veces —un reinicio, por ejemplo— el segundo `release` se llevaría **la reserva de la otra instancia**. Con el holder `{alias, port}` es difícil que pase, pero por eso el estado debería ser **uno solo**, con el `release` en un sitio.

---

## Resumen

|                 |                                                                                                                                                                 |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Diseño          | **Bueno.** El overhead como titular `:system` resuelve los 370 MiB; el holder `{alias, port}` resuelve la identidad; el acquire antes de `fit-params` serializa |
| Bug que bloquea | **La cuota `:infinity`** → `:unknown_cost` → ningún modelo de GPU arranca                                                                                       |
| Segundo bug     | **El resultado del segundo `acquire` se descarta** → se puede arrancar sin reserva                                                                              |
| tercero         | **La reserva inicial de 512 MiB** contra 14707 reales: o margen, o error explícito                                                                              |
| Tests           | En verde, pero los que lo verificarían están **skipped**, y fallan antes de llegar                                                                              |

Con esos tres arreglos, el PR C es lo que D-27 pidió. Sin ellos, es un PR que **no arranca modelos** y que la CI no puede ver.
