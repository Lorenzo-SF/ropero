import { Plugin } from "@opencode/plugin"
import { execFile } from "node:child_process"

/**
 * Plugin para los modelos locales de ropero (llama-server).
 *
 * HACE DOS COSAS, y las dos son por el mismo motivo: que el modelo que
 * pides sea el que te responde, y que no se te vaya el tiempo ni la GPU en
 * peticiones que no son la tuya.
 *
 * 1) ARRANCA EL MODELO QUE TOCA
 *
 *    llama-server NO valida el campo "model" del body: atiende con el modelo
 *    que tenga CARGADO. Así que pedir "marichocho/qwen" mientras hay un gptoss
 *    en el puerto no da error: te contesta el gptoss. Sin error, sin aviso.
 *    Eso convierte en mentira cualquier catálogo de modelos del cliente.
 *
 *    `ropero use <modelo>` cierra el hueco: es idempotente (si ese modelo ya
 *    se sirve, no toca nada), para por PUERTO lo que hubiera (no por nombre:
 *    `ropero stop <modelo>` mata en el primer puerto gestionado que encaje,
 *    que puede no ser el suyo) y espera a que el modelo esté listo de verdad.
 *
 * 2) LIMITA LA PETICIÓN DE TÍTULO
 *
 *    Al crear una sesión, opencode lanza en background una SEGUNDA petición
 *    al modelo para poner título a la sesión. Esa petición va SIN límite de
 *    tokens: el backend usa su máximo (con un ctx de 262144 son 261.424
 *    tokens) y con un modelo que razona (el qwen35, cuyo template por defecto
 *    es xhigh) se come 36.000 tokens de razonamiento y 8 minutos de GPU.
 *
 *    Es el issue upstream anomalyco/opencode#54401, reproducido con un
 *    backend local. Aquí se acota: maxTokens pequeño y el effort más bajo que
 *    el modelo admita. O, si prefieres, se pone el título en local y la
 *    petición no llega a existir.
 *
 *    Ojo: `reasoningEffort` se manda tal cual en el body. Para el qwen35 el
 *    template solo admite xhigh | medium | low: mandar "high" hace que el
 *    propio jinja lance "Unexpected reasoning effort high". Por eso el effort
 *    del título es "low", nunca "high".
 */

type Opts = {
  /** Provider de opencode que atiende a los modelos de ropero. */
  provider?: string
  /**
   * Puerto donde debe estar el modelo. Se pasa como `--port`, así que el
   * modelo se sirve ahí aunque tenga un MODEL_PORT pineado: opencode habla
   * con un único endpoint.
   */
  port?: number
  /** Tokens máximos para la petición de título. 0 desactiva el tope. */
  titleMaxTokens?: number
  /** Effort del razonamiento en la petición de título. */
  titleReasoning?: string
  /** Si es true, el título se pone en local y no se llama al modelo. */
  titleLocal?: boolean
}

type Resultado = { stdout: string; stderr: string; code: number }

function ropero(args: string[], timeoutMs = 900_000): Promise<Resultado> {
  return new Promise((resolve) => {
    execFile(
      "ropero",
      args,
      { timeout: timeoutMs, maxBuffer: 4 * 1024 * 1024, encoding: "utf8" },
      (err, stdout, stderr) => {
        if (!err) return resolve({ stdout, stderr, code: 0 })
        const code = typeof (err as any).code === "number" ? (err as any).code : 1
        resolve({ stdout: stdout ?? "", stderr: stderr ?? "", code })
      },
    )
  })
}

/**
 * Los modelos que ropero conoce, según el ropero INSTALADO (que es el que
 * arranca). Se cachea: cambia muy rara vez y `ropero list` no es gratis.
 *
 * POR QUÉ ESTE FILTRO
 *   El modelID de opencode no siempre es un modelo de ropero. Por ejemplo,
 *   un config que declare "Puertourraco" como nombre del provider local no
 *   tiene por qué corresponderse con ningún modelo de ropero. En ese caso el
 *   plugin NO hace nada: el server ya está sirviendo lo que sea, y meter a
 *   ropero por medio solo conseguiría romper la petición.
 */
let modelosConocidos: Set<string> | null = null

async function modelosDeRopero(): Promise<Set<string>> {
  if (modelosConocidos) return modelosConocidos
  const r = await ropero(["list"], 30_000)
  const set = new Set<string>()
  if (r.code === 0) {
    for (const line of r.stdout.split("\n")) {
      const m = line.match(/^\s{2}([A-Za-z0-9._-]+)\s+/)
      if (m) set.add(m[1])
    }
  }
  modelosConocidos = set
  return set
}

/**
 * Los NIVELES que declara un modelo ("ropero levels <modelo>").
 *
 * Un nivel es un modo del MODELO, no una opción de la petición: el fable y
 * el nemotron razonan o no razonan según un `--chat-template-kwargs` que va
 * en el ARGV del server, y ese argv se fija al arrancar. Por eso el nivel
 * no puede viajar en el body de la petición (como hace el reasoning_effort)
 * y tiene que llegar al arranque.
 *
 * Se cachean: cambiarlos es cosa del repo, no de la sesion.
 */
const nivelesPorModelo = new Map<string, Set<string>>()

async function nivelesDe(modelo: string): Promise<Set<string>> {
  const cached = nivelesPorModelo.get(modelo)
  if (cached) return cached
  const r = await ropero(["levels", modelo], 30_000)
  const set = new Set<string>()
  if (r.code === 0) {
    for (const line of r.stdout.split("\n")) {
      const m = line.trim().match(/^([A-Za-z0-9._-]+)$/)
      if (m) set.add(m[1])
    }
  }
  nivelesPorModelo.set(modelo, set)
  return set
}

/** El texto del primer mensaje de usuario, para el título local. */
function primerTexto(messages: readonly any[]): string {
  for (const m of messages) {
    if (m?.role !== "user") continue
    const partes = Array.isArray(m.content) ? m.content : []
    const texto = partes
      .filter((p: any) => p?.type === "text" && typeof p.text === "string")
      .map((p: any) => p.text)
      .join(" ")
      .trim()
    if (texto) return texto
  }
  return ""
}

export default Plugin.define({
  id: "ropero-modelo",

  async setup(ctx) {
    const opts = (ctx.options ?? {}) as Opts
    const provider = opts.provider ?? "marichocho"

    // El modelo que sabemos servido, y la promesa en curso: así dos
    // peticiones seguidas no lanzan dos 'ropero use'.
    // La clave lleva el NIVEL: el mismo modelo con otro nivel son args
    // distintos, y el server se reinicia (por eso no se puede cachear solo
    // el nombre del modelo).
    let servido: string | null = null
    let enCurso: Promise<void> | null = null
    const avisados = new Set<string>()

    async function correr(modelo: string, nivel: string | null): Promise<void> {
      const args = ["use", modelo]
      if (opts.port) args.push("--port", String(opts.port))
      if (nivel) args.push("--nivel", nivel)
      const r = await ropero(args)
      if (r.code !== 0) {
        throw new Error(
          `ropero use ${modelo}${nivel ? " --nivel " + nivel : ""} falló (rc=${r.code})\n` +
            (r.stderr.trim() || r.stdout.trim() || "(sin salida)"),
        )
      }
      const puerto = r.stdout.match(/ROPERO_PORT=(\d+)/)?.[1]
      console.log(
        `[ropero] '${modelo}'${nivel ? ` (nivel ${nivel})` : ""} listo${puerto ? ` en :${puerto}` : ""}`,
      )
    }

    async function asegurar(modelo: string, nivel: string | null): Promise<void> {
      // Solo si el modelo es UNO de ropero. Si el modelID de opencode no
      // corresponde a ningún modelo de ropero (un alias propio del config,
      // por ejemplo), el plugin no toca nada: el server ya está sirviendo
      // lo que tenga y llamar a ropero solo rompería la petición.
      const conocidos = await modelosDeRopero()
      if (!conocidos.has(modelo)) {
        if (!avisados.has(modelo)) {
          avisados.add(modelo)
          console.log(`[ropero] '${modelo}' no es un modelo de ropero: no hago nada`)
        }
        return
      }
      const clave = `${modelo}@${nivel ?? ""}`
      if (servido === clave) return
      if (enCurso) {
        try {
          await enCurso
        } catch {
          /* el que espera reintenta con su propio modelo */
        }
      }
      if (servido === clave) return
      const p = correr(modelo, nivel)
      enCurso = p
      try {
        await p
      } finally {
        if (enCurso === p) enCurso = null
      }
      servido = clave
    }

    // "context" corre justo antes de cada peticion al modelo del agent loop,
    // y trae el modelo con el que se va a llamar. Es async, asi que puede
    // esperar a que el modelo este listo sin que la peticion salga antes.
    await ctx.session.hook("context", async (event) => {
      const m = event.model
      if (!m || m.providerID !== provider) return
      const modelo = m.id
      if (!modelo) return

      // El VARIANT de opencode puede ser el nombre de un NIVEL del modelo.
      // Si lo es, se le pasa al arranque: el nivel va en el argv del server
      // (no en el body), asi que sin esto no habria forma de pedirlo desde
      // opencode. Por ejemplo, el fable declara el nivel "think".
      //
      // OJO: el nombre del variant y el del nivel no tienen por que ser lo
      // mismo, y aqui solo se acepta una coincidencia EXACTA con un nivel
      // que el modelo declare. Si no coincide, no se toca nada: el body lo
      // lleva (reasoning_effort y companhia) y el server con su default.
      let nivel: string | null = null
      const variant = (m as any).variant
      if (variant && (await nivelesDe(modelo)).has(variant)) nivel = variant

      if (nivel) {
        // El nivel ya va en el argv del server. Si ademas el body lleva un
        // reasoningEffort, el template lo recibe con un valor que el modelo
        // puede no admitir (el qwen35 solo admite xhigh | medium | low), y
        // el jinja revienta con "Unexpected reasoning effort". Se quita.
        const opts2 = (event as any).options
        if (opts2 && "reasoningEffort" in opts2) {
          try {
            delete opts2.reasoningEffort
          } catch {
            opts2.reasoningEffort = undefined
          }
        }
      }

      await asegurar(modelo, nivel)
    })

    // La peticion de TITULO no pasa por "context": es otro tipo de peticion
    // ("primary" | "compaction" | "title" | "generate"). Si el modelo que
    // esta sirviendo no es el de la sesion, el titulo se lo come otro.
    await ctx.session.hook("model.request", async (event) => {
      const m = event.model
      if (!m || m.providerID !== provider) return
      if (event.kind === "title") await asegurar(m.id, null)
    })

    // Y aqui se acota la peticion de titulo.
    await ctx.session.hook("title", (event) => {
      if (opts.titleLocal) {
        const texto = primerTexto(event.messages as any)
        const titulo = texto.length > 60 ? texto.slice(0, 57).trim() + "..." : texto
        // Poner result salta la llamada al modelo: no hay peticiones fantasma.
        event.result = titulo || "sesion"
        return
      }

      const max = opts.titleMaxTokens ?? 64
      if (max > 0) event.options.maxTokens = max

      // El effort solo si el modelo declara que razona. Y "low", nunca
      // "high": el template del qwen35 no admite "high" y revienta.
      const info = ctx.model.get(event.model.providerID, event.model.id)
      const razon = info?.capabilities?.reasoning
      if (razon && razon.supported) {
        event.options.reasoningEffort = opts.titleReasoning ?? "low"
      }
    })
  },
})
