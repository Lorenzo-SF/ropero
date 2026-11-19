# Integraciones con agentes

`ropero connect --list` muestra las opciones reconocidas. Después de instalar y
arrancar un modelo, se puede registrar con `ropero connect --model <modelo>
--agent <agente>`. Para retirarlo se añade `--remove`.

| Agente | Qué hace Ropero | Qué hace falta y límites |
|---|---|---|
| OpenCode (`opencode`) | Actualiza el provider OpenAI-compatible en `opencode.json` o `opencode.jsonc`; conserva comentarios y otras claves. | OpenCode instalado. Se registra el endpoint local por engine. |
| Codex (`codex`) | Escribe un perfil TOML por engine en `CODEX_HOME` o `~/.codex`; deja intacto el perfil global. | Sólo engines que Ropero declara compatibles con Responses API. Se usa con `codex --profile ropero-<engine> -m <modelo>`. |
| Claude Code (`claude`) | Imprime las variables `ANTHROPIC_*` para el endpoint local Anthropic Messages. | Es una integración guiada: el usuario exporta las variables al shell/proceso. No modifica el perfil de Claude. Si Ropero tiene una clave, hay que sustituir el marcador por esa clave; Ropero no la imprime. Los modelos deben compartir engine, puerto y API Messages. |
| Pi (`pi`) | Actualiza `~/.pi/agent/models.json` con providers OpenAI-compatible. | Pi instalado. La configuración no selecciona por sí sola un modelo por defecto. |
| Hermes (`hermes`) | Llama a `hermes config` para registrar el provider y guarda la clave en `HERMES_HOME/.env`; mantiene un registro de propiedad para la baja. | Hermes CLI instalado y en `PATH`. Se usa con `hermes chat --provider custom:ropero-<engine> --model <modelo>`. Si falla una actualización, Ropero intenta restaurar configuración, `.env` y registro anteriores. |
| OpenClaw (`openclaw`) | Actualiza `~/.openclaw/agents/main/agent/models.json`. El alias histórico `claw` llama al mismo adaptador. | OpenClaw instalado. El registro queda en el catálogo del agente `main`; no cambia el modelo por defecto ni otros agentes. |
| PicoClaw (`picoclaw`) | Añade modelos al esquema v2 de `~/.picoclaw/config.json`, guarda claves bajo `model_list` en `~/.picoclaw/.security.yml` y registra propiedad en `~/.config/ropero/connect/picoclaw.json`. | PicoClaw v2. Rechaza otro valor de `version` en vez de migrar configuración. No cambia el modelo por defecto. |
| VS Code (`vscode`) | Registra el endpoint en `chatLanguageModels.json` mediante el proveedor actual `customendpoint`. | El archivo no contiene claves. Si el engine exige autenticación, completa la credencial desde Manage Language Models en VS Code. BYOK cubre chat y utilidades, no autocompletado inline, búsqueda semántica ni embeddings. |

Las integraciones automáticas preservan los campos que no administran, rechazan
formatos inválidos antes de reescribirlos y escriben los archivos que contienen
claves con permisos privados en sistemas Unix. La conexión configura el cliente;
no instala el agente ni inicia el engine. Primero hay que arrancar el modelo
con Ropero. Los clientes y engines pueden tener límites propios de API,
herramientas, razonamiento y multimodalidad.

## Referencias de formato

- [OpenCode: custom providers](https://opencode.ai/docs/providers/)
- [Codex: configuración de providers](https://developers.openai.com/codex/config-advanced/)
- [Claude Code: gateways LLM](https://docs.anthropic.com/en/docs/claude-code/llm-gateway)
- [Pi: configuración de modelos](https://docs.pi.dev/latest/models-configuration)
- [Hermes: providers](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md)
- [OpenClaw: custom providers](https://docs.openclaw.ai/gateway/config-tools/custom-providers)
- [PicoClaw: modelo personalizado](https://docs.picoclaw.io/docs/configuration/model-list/)
- [VS Code: modelos de IA y Custom Endpoint](https://code.visualstudio.com/docs/agent-customization/language-models)
- [Zed: acceso a API y endpoint compatible](https://zed.dev/docs/ai/use-api-access)
- [Cursor: claves API y proveedores admitidos](https://cursor.com/docs/models-and-usage/api-keys)
- [MicroClaw: configuración de ejemplo](https://github.com/microclaw/microclaw/blob/main/microclaw.config.example.yaml)
- [Graphify: instalación y asistentes compatibles](https://github.com/Graphify-Labs/graphify)

Esta guía describe el comportamiento implementado en Ropero y los formatos
documentados upstream. No certifica todas las versiones de las aplicaciones;
para Hermes se requiere además que su CLI esté disponible en el equipo.

## Otros IDE y herramientas de análisis

Estos productos tienen configuración propia y no son destinos automáticos de
`ropero connect` en esta versión:

- **Zed:** compatible con endpoints OpenAI. Abre Agent Settings con
  `agent: open settings` → LLM Providers → Add Provider; introduce el nombre,
  URL `http://127.0.0.1:<puerto>/v1`, ID del modelo y ventana de contexto. La
  clave se guarda en el llavero del sistema, no en `settings.json`. También se
  puede declarar `language_models.openai_compatible` en ese archivo y añadir
  la clave desde Agent Settings.
- **Cursor:** su documentación oficial describe claves para sus proveedores
  admitidos, pero no confirma una conexión directa a endpoints locales de
  Ropero. Por eso no se anuncia una integración compatible. Cursor Tab sigue
  usando los modelos integrados de Cursor.
- **MicroClaw:** configura manualmente `llm_provider`, `model`, `api_key` y
  `llm_base_url` en `microclaw.config.yaml` (para un servidor OpenAI-compatible,
  elige el provider `openai` y la URL local del engine). Conserva los tokens de
  canales y el resto del archivo; protege el archivo porque contiene secretos.
  MicroClaw necesita además su propio canal/bot configurado.
- **Graphify:** herramienta opcional para crear un grafo del repositorio y
  enriquecer el análisis en asistentes compatibles. Instálala siguiendo su
  guía y úsala desde Cursor, Codex, OpenCode u otro host compatible; el LLM y
  sus credenciales los configura ese host. No es un engine ni un modelo de
  Ropero y no se conecta con `ropero connect`.

OpenClaw conserva su integración implementada. Se actualizará la referencia de
su configuración cuando se reciba la URL específica que el usuario quiere usar.
