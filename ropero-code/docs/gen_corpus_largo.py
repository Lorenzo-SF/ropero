#!/usr/bin/env python3
"""Genera el corpus de codigo para la prueba larga (contexto largo real).

El fichero prueba_larga.md NO puede llevar 40k tokens de codigo dentro: se
genera. Este script lo produce de forma DETERMINISTA (semilla fija), asi que
dos corridas dan el mismo corpus y los 5 datos plantados estan siempre en el
mismo sitio.

Uso:
    python3 gen_corpus_largo.py > /tmp/corpus.md

El corpus ronda los 24.500 tokens: cabe en el ctx mas pequeno del roster
(32768), de modo que todos los modelos pueden hacer el test largo.

Los 5 datos plantados (el modelo tiene que acordarse de ellos al final):
    1. La constante MAGICKO es 4823
    2. La funcion se llama reconciliar_cuentas
    3. El puerto de la cola es 7317
    4. La tabla de auditoria se llama eventos_audit
    5. El TTL de la cache es 900 segundos
"""
import random
import sys

SEED = 20251025
random.seed(SEED)

# ── Generadores de codigo verosimil ────────────────────────────────────────

MODULOS = [
    "Cuentas", "Pagos", "Inventario", "Pedidos", "Clientes", "Facturacion",
    "Envios", "Proveedores", "Catalogos", "Descuentos", "Ventas", "Devoluciones",
    "Tickets", "Suscripciones", "Metricas", "Auditoria", "Reportes", "Sincronizacion",
]

VERBS = ["procesar", "validar", "transformar", "serializar", "agregar", "normalizar",
         "persistir", "enriquecer", "filtrar", "proyectar", "reconciliar", "indexar"]


def modulo(nombre):
    """Un módulo Elixir plausible con funciones."""
    lineas = []
    lineas.append("defmodule %s do" % nombre)
    lineas.append("  @moduledoc \"\"\"")
    lineas.append("  Servicios de %s para el dominio de la tienda.\"\"\"" % nombre.lower())
    lineas.append("")
    lineas.append("  @doc \"Configuración por defecto del pipeline.\"")
    lineas.append("  def config, do: [")
    lineas.append("    timeout: %d," % random.randint(500, 9000))
    lineas.append("    reintentos: %d," % random.randint(1, 5))
    lineas.append("    lote: %d," % random.choice([10, 25, 50, 100, 250, 500]))
    lineas.append("    estricto: %s" % random.choice(["true", "false"]))
    lineas.append("  ]")
    lineas.append("")
    for i in range(random.randint(3, 6)):
        verbo = random.choice(VERBS)
        fn = "%s_%s" % (verbo, random.choice(["lote", "registro", "bloque", "cola", "peticion"]))
        lineas.append("  @doc \"#{verbo.capitalize()} los datos de entrada.\"")
        lineas.append("  def %s(datos, opts \\\\ []) do" % fn)
        lineas.append("    cfg = Keyword.merge(config(), opts)")
        lineas.append("")
        lineas.append("    with {:ok, entrada} <- preparar(datos, cfg),")
        lineas.append("         {:ok, salida} <- aplicar(entrada, cfg) do")
        lineas.append("      {:ok, salida}")
        lineas.append("    else")
        lineas.append("      {:error, razon} -> {:error, razon}")
        lineas.append("    end")
        lineas.append("  end")
        lineas.append("")
    lineas.append("  defp preparar(datos, cfg) do")
    lineas.append("    if is_list(datos) and length(datos) <= cfg[:lote] do")
    lineas.append("      {:ok, datos}")
    lineas.append("    else")
    lineas.append("      {:error, :lote_excedido}")
    lineas.append("    end")
    lineas.append("  end")
    lineas.append("")
    lineas.append("  defp aplicar(entrada, cfg) do")
    lineas.append("    resultado =")
    lineas.append("      entrada")
    lineas.append("      |> Enum.map(&normalizar/1)")
    lineas.append("      |> Enum.reject(&is_nil/1)")
    lineas.append("")
    lineas.append("    {:ok, resultado}")
    lineas.append("  end")
    lineas.append("")
    lineas.append("  defp normalizar(%{id: id} = m) when is_integer(id), do: m")
    lineas.append("  defp normalizar(m) when is_map(m), do: Map.put(m, :id, nil)")
    lineas.append("  defp normalizar(m), do: nil")
    lineas.append("end")
    return "\n".join(lineas)


def cuerpo_extra(n=40):
    """Módulos de relleno, para llegar a los ~40k tokens."""
    out = []
    for i in range(n):
        nombre = "%s%s" % (random.choice(MODULOS), random.choice(["Worker", "Repo", "Service", "Adapter", "Pipeline"]))
        out.append(modulo(nombre))
        out.append("")
    return "\n".join(out)


def main():
    partes = []
    partes.append("# Repo tienda — código de referencia")
    partes.append("")
    partes.append("Este es el código del servicio. Léelo entero antes de responder.")
    partes.append("")

    # ── Los 5 datos plantados, escondidos en sitios distintos ───────────────
    partes.append("defmodule Tienda.Config do")
    partes.append("  @moduledoc \"\"\"Configuración global de la tienda.\"\"\"")
    partes.append("")
    partes.append("  # La constante que usa el cálculo de cupones.")
    partes.append("  @magicko 4823")
    partes.append("  def magicko, do: @magicko")
    partes.append("")
    partes.append("  # El puerto donde escucha la cola de trabajo.")
    partes.append("  @puerto_cola 7317")
    partes.append("  def puerto_cola, do: @puerto_cola")
    partes.append("")
    partes.append("  # Tiempo de vida de la caché de sesiones, en segundos.")
    partes.append("  @ttl_cache 900")
    partes.append("  def ttl_cache, do: @ttl_cache")
    partes.append("")
    partes.append("  # La tabla donde se registra toda la actividad.")
    partes.append("  @tabla_auditoria \"eventos_audit\"")
    partes.append("  def tabla_auditoria, do: @tabla_auditoria")
    partes.append("end")
    partes.append("")

    # El segundo dato plantado, en otro módulo, con otro nombre de función.
    partes.append("defmodule Tienda.Cuentas do")
    partes.append("  @moduledoc \"\"\"Reconciliación de cuentas con el banco.\"\"\"")
    partes.append("")
    partes.append("  @doc \"Compara el saldo local con el extracto del banco.\"")
    partes.append("  def reconciliar_cuentas(cuentas, extracto) do")
    partes.append("    diferencias =")
    partes.append("      for c <- cuentas, e <- extracto, c.numero == e.numero, do: {c, e}")
    partes.append("")
    partes.append("    Enum.map(diferencias, fn {c, e} ->")
    partes.append("      %{numero: c.numero, local: c.saldo, remoto: e.saldo, delta: c.saldo - e.saldo}")
    partes.append("    end)")
    partes.append("  end")
    partes.append("end")
    partes.append("")

    # El corpus tiene que caber en el ctx MAS PEQUEÑO del roster (32768,
    # que es lo que tienen devstral, laguna y qwenvision), porque el banco
    # exige que TODOS los modelos puedan hacer TODOS los tests.
    #
    # 38 modulos x ~640 tokens = ~24.300 tokens de corpus, mas las preguntas
    # (~200) y la respuesta del modelo. Con eso el prompt completo ronda los
    # 24.500 tokens: en un ctx de 32768 quedan 8.000 para la respuesta, de
    # sobra.
    partes.append(cuerpo_extra(38))

    partes.append("defmodule Tienda.Repo do")
    partes.append("  use Ecto.Repo, otp_app: :tienda")
    partes.append("")
    partes.append("  @doc \"Escribe una entrada en la tabla de auditoría.\"")
    partes.append("  def auditar(evento) do")
    partes.append("    insertar(Tienda.Config.tabla_auditoria(), evento)")
    partes.append("  end")
    partes.append("")
    partes.append("  defp insertar(tabla, evento) do")
    partes.append("    {:ok, %{tabla: tabla, evento: evento, ts: System.system_time(:second)}}")
    partes.append("  end")
    partes.append("end")
    partes.append("")

    sys.stdout.write("\n".join(partes) + "\n")


if __name__ == "__main__":
    main()
