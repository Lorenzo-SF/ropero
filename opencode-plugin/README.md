# Plugin de opencode para ropero

Dos cosas, y las dos por el mismo motivo: que el modelo que pides sea el que
te responde, y que no se te vaya el tiempo ni la GPU en peticiones que no son
la tuya.

## Instalar

```bash
mkdir -p ~/.config/opencode/plugins/ropero-modelo
cp opencode-plugin/index.ts ~/.config/opencode/plugins/ropero-modelo/index.ts
```

Los plugins de `~/.config/opencode/plugins/` se cargan solos. En el config:

```jsonc
"plugins": [
  {
    "package": "./plugins/ropero-modelo",
    "options": {
      "provider": "marichocho",
      "port": 9999,
      "titleMaxTokens": 64,
      "titleReasoning": "low"
    }
  }
]
```

| opción | qué hace | por defecto |
|---|---|---|
| `provider` | el provider de opencode que atiende a tus modelos de ropero | `marichocho` |
| `port` | puerto donde debe estar el modelo (se pasa como `--port`, así que el modelo se sirve ahí aunque tenga un `MODEL_PORT` pineado) | sin `--port` |
| `titleMaxTokens` | tokens máximos de la petición de título (`0` = sin tope) | `64` |
| `titleReasoning` | effort del razonamiento en la petición de título | `low` |
| `titleLocal` | si es `true`, el título se pone en local y **no** se llama al modelo | `false` |

## Qué hace

### 1. Arranca el modelo que toca

`llama-server` **no valida el campo `"model"` del body**
(`tools/server/server-common.cpp:1204` lo lee, pero el slot usa el modelo
cargado). Así que pedir `marichocho/qwen` mientras hay un `gptoss` en el
puerto **no da error: te contesta el gptoss**.

Eso convierte en mentira cualquier catálogo de modelos del cliente: diez
modelos registrados son diez etiquetas sobre el mismo servidor.

`ropero use <modelo>` cierra el hueco:

- **idempotente**: si ese modelo ya se sirve, no toca nada y sale en
  milisegundos. Se puede llamar en cada petición.
- para lo que hubiera en el puerto **por puerto**, no por nombre:
  `ropero stop <modelo>` mata en el primer puerto gestionado que encaje, que
  puede no ser el suyo.
- espera a que el modelo esté **listo de verdad** (no a que el puerto
  escuche).

Si el modelo no se puede arrancar, el hook **falla la petición**. Es lo
correcto: seguir con un error del server sería justo el fallo silencioso que
se quiere evitar.

### 2. Acota la petición de título

Al crear una sesión, opencode lanza en background una **segunda** petición al
modelo para titular la sesión. Esa petición va **sin límite de tokens**: el
backend usa su máximo (con un ctx de 262144 son 261.424 tokens) y con un
modelo que razona (el qwen35, cuyo template por defecto es `xhigh`) se come
36.000 tokens de razonamiento y **8 minutos** de GPU, en paralelo a tu
petición principal.

Es el issue upstream [anomalyco/opencode#54401](https://github.com/anomalyco/opencode/issues/54401),
reproducido con un backend local.

Aquí se acota:

```ts
event.options.maxTokens = 64
event.options.reasoningEffort = "low"
```

O, si prefieres que la petición no exista:

```ts
event.result = "titulo local"
```

> `reasoningEffort` se manda tal cual en el body. Para el qwen35 el template
> solo admite `xhigh | medium | low`: mandar `"high"` hace que el propio jinja
> lance `Unexpected reasoning effort high`. Por eso el effort del título es
> `low`, nunca `high`.

## Los hooks que usa

| hook | cuándo | qué hace |
|---|---|---|
| `context` | justo antes de cada petición del agent loop | `ropero use` del modelo de la sesión |
| `model.request` | en cada petición, con `kind` = `primary` \| `compaction` \| `title` \| `generate` | `ropero use` también para el título |
| `title` | en la petición de título | acota tokens y effort, o pone el título en local |

`hook("prompt")` **no** vale para esto: no tiene `model` (la resolución del
modelo ocurre después de la admisión).
