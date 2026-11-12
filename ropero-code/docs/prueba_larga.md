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
# Repo tienda — código de referencia

Este es el código del servicio. Léelo entero antes de responder.

defmodule Tienda.Config do
  @moduledoc """Configuración global de la tienda."""

  # La constante que usa el cálculo de cupones.
  @magicko 4823
  def magicko, do: @magicko

  # El puerto donde escucha la cola de trabajo.
  @puerto_cola 7317
  def puerto_cola, do: @puerto_cola

  # Tiempo de vida de la caché de sesiones, en segundos.
  @ttl_cache 900
  def ttl_cache, do: @ttl_cache

  # La tabla donde se registra toda la actividad.
  @tabla_auditoria "eventos_audit"
  def tabla_auditoria, do: @tabla_auditoria
end

defmodule Tienda.Cuentas do
  @moduledoc """Reconciliación de cuentas con el banco."""

  @doc "Compara el saldo local con el extracto del banco."
  def reconciliar_cuentas(cuentas, extracto) do
    diferencias =
      for c <- cuentas, e <- extracto, c.numero == e.numero, do: {c, e}

    Enum.map(diferencias, fn {c, e} ->
      %{numero: c.numero, local: c.saldo, remoto: e.saldo, delta: c.saldo - e.saldo}
    end)
  end
end

defmodule DescuentosPipeline do
  @moduledoc """
  Servicios de descuentospipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6778,
    reintentos: 3,
    lote: 25,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule InventarioPipeline do
  @moduledoc """
  Servicios de inventariopipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 5216,
    reintentos: 2,
    lote: 100,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule CuentasRepo do
  @moduledoc """
  Servicios de cuentasrepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3984,
    reintentos: 1,
    lote: 25,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule FacturacionService do
  @moduledoc """
  Servicios de facturacionservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6662,
    reintentos: 3,
    lote: 25,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule AuditoriaWorker do
  @moduledoc """
  Servicios de auditoriaworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 892,
    reintentos: 5,
    lote: 25,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule ProveedoresAdapter do
  @moduledoc """
  Servicios de proveedoresadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 4506,
    reintentos: 5,
    lote: 100,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule FacturacionRepo do
  @moduledoc """
  Servicios de facturacionrepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 4903,
    reintentos: 1,
    lote: 25,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosWorker do
  @moduledoc """
  Servicios de enviosworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6105,
    reintentos: 4,
    lote: 100,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule MetricasPipeline do
  @moduledoc """
  Servicios de metricaspipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 1028,
    reintentos: 4,
    lote: 500,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DevolucionesWorker do
  @moduledoc """
  Servicios de devolucionesworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 4825,
    reintentos: 3,
    lote: 10,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule ProveedoresPipeline do
  @moduledoc """
  Servicios de proveedorespipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 2169,
    reintentos: 5,
    lote: 50,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule CatalogosAdapter do
  @moduledoc """
  Servicios de catalogosadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6252,
    reintentos: 4,
    lote: 250,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule InventarioAdapter do
  @moduledoc """
  Servicios de inventarioadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 965,
    reintentos: 5,
    lote: 10,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule InventarioService do
  @moduledoc """
  Servicios de inventarioservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3562,
    reintentos: 2,
    lote: 100,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule AuditoriaRepo do
  @moduledoc """
  Servicios de auditoriarepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3768,
    reintentos: 5,
    lote: 25,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosAdapter do
  @moduledoc """
  Servicios de enviosadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8990,
    reintentos: 5,
    lote: 50,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DescuentosWorker do
  @moduledoc """
  Servicios de descuentosworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 4965,
    reintentos: 5,
    lote: 50,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule MetricasPipeline do
  @moduledoc """
  Servicios de metricaspipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3437,
    reintentos: 3,
    lote: 500,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule ReportesRepo do
  @moduledoc """
  Servicios de reportesrepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 1899,
    reintentos: 1,
    lote: 50,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule PagosWorker do
  @moduledoc """
  Servicios de pagosworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8684,
    reintentos: 1,
    lote: 10,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule SuscripcionesService do
  @moduledoc """
  Servicios de suscripcionesservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 7158,
    reintentos: 2,
    lote: 250,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule PagosWorker do
  @moduledoc """
  Servicios de pagosworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 2494,
    reintentos: 1,
    lote: 250,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DevolucionesWorker do
  @moduledoc """
  Servicios de devolucionesworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3164,
    reintentos: 4,
    lote: 50,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule TicketsService do
  @moduledoc """
  Servicios de ticketsservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 822,
    reintentos: 1,
    lote: 25,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosAdapter do
  @moduledoc """
  Servicios de enviosadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6700,
    reintentos: 3,
    lote: 500,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule SuscripcionesRepo do
  @moduledoc """
  Servicios de suscripcionesrepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8548,
    reintentos: 1,
    lote: 500,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosPipeline do
  @moduledoc """
  Servicios de enviospipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8065,
    reintentos: 1,
    lote: 50,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule ProveedoresService do
  @moduledoc """
  Servicios de proveedoresservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 2828,
    reintentos: 2,
    lote: 500,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosAdapter do
  @moduledoc """
  Servicios de enviosadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6195,
    reintentos: 1,
    lote: 100,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def proyectar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule PagosRepo do
  @moduledoc """
  Servicios de pagosrepo para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8114,
    reintentos: 5,
    lote: 250,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DescuentosWorker do
  @moduledoc """
  Servicios de descuentosworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 3510,
    reintentos: 1,
    lote: 50,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule EnviosPipeline do
  @moduledoc """
  Servicios de enviospipeline para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 5618,
    reintentos: 5,
    lote: 500,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule CuentasAdapter do
  @moduledoc """
  Servicios de cuentasadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 7686,
    reintentos: 5,
    lote: 250,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DevolucionesService do
  @moduledoc """
  Servicios de devolucionesservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 7613,
    reintentos: 1,
    lote: 500,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule DevolucionesService do
  @moduledoc """
  Servicios de devolucionesservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 5114,
    reintentos: 5,
    lote: 50,
    estricto: true
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule ReportesWorker do
  @moduledoc """
  Servicios de reportesworker para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6443,
    reintentos: 2,
    lote: 500,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def serializar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_peticion(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def transformar_bloque(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule VentasService do
  @moduledoc """
  Servicios de ventasservice para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 6645,
    reintentos: 5,
    lote: 250,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def reconciliar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def persistir_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def validar_registro(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def procesar_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule CatalogosAdapter do
  @moduledoc """
  Servicios de catalogosadapter para el dominio de la tienda."""

  @doc "Configuración por defecto del pipeline."
  def config, do: [
    timeout: 8484,
    reintentos: 5,
    lote: 100,
    estricto: false
  ]

  @doc "#{verbo.capitalize()} los datos de entrada."
  def filtrar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def enriquecer_lote(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def indexar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def normalizar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  @doc "#{verbo.capitalize()} los datos de entrada."
  def agregar_cola(datos, opts \\ []) do
    cfg = Keyword.merge(config(), opts)

    with {:ok, entrada} <- preparar(datos, cfg),
         {:ok, salida} <- aplicar(entrada, cfg) do
      {:ok, salida}
    else
      {:error, razon} -> {:error, razon}
    end
  end

  defp preparar(datos, cfg) do
    if is_list(datos) and length(datos) <= cfg[:lote] do
      {:ok, datos}
    else
      {:error, :lote_excedido}
    end
  end

  defp aplicar(entrada, cfg) do
    resultado =
      entrada
      |> Enum.map(&normalizar/1)
      |> Enum.reject(&is_nil/1)

    {:ok, resultado}
  end

  defp normalizar(%{id: id} = m) when is_integer(id), do: m
  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)
  defp normalizar(m), do: nil
end

defmodule Tienda.Repo do
  use Ecto.Repo, otp_app: :tienda

  @doc "Escribe una entrada en la tabla de auditoría."
  def auditar(evento) do
    insertar(Tienda.Config.tabla_auditoria(), evento)
  end

  defp insertar(tabla, evento) do
    {:ok, %{tabla: tabla, evento: evento, ts: System.system_time(:second)}}
  end
end


<!-- CORPUS:FIN -->
