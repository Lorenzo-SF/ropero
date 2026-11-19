# Agent integrations

`ropero connect --list` shows the recognized options. After installing and
starting a model, register it with `ropero connect --model <model>
--agent <agent>`. Add `--remove` to remove it.

| Agent | What Ropero does | Requirements and limits |
|---|---|---|
| OpenCode (`opencode`) | Updates the OpenAI-compatible provider in `opencode.json` or `opencode.jsonc`, preserving comments and unrelated keys. | OpenCode installed. Registers the local endpoint by engine. |
| Codex (`codex`) | Writes one TOML profile per engine under `CODEX_HOME` or `~/.codex`, leaving the global profile unchanged. | Only engines declared by Ropero to support the Responses API. Run with `codex --profile ropero-<engine> -m <model>`. |
| Claude Code (`claude`) | Prints `ANTHROPIC_*` variables for a local Anthropic Messages endpoint. | Guided setup: the user exports the variables for the shell/process. Ropero does not edit Claude's profile. If Ropero has a key configured, replace the printed placeholder with that key; Ropero does not print it. Selected models must share an engine, port, and Messages API. |
| Pi (`pi`) | Updates `~/.pi/agent/models.json` with OpenAI-compatible providers. | Pi installed. This does not select a default model. |
| Hermes (`hermes`) | Uses `hermes config` to register the provider and stores its key in `HERMES_HOME/.env`; tracks ownership for removal. | Hermes CLI installed and available on `PATH`. Run it with `hermes chat --provider custom:ropero-<engine> --model <model>`. On update failure Ropero attempts to restore the previous config, `.env`, and ownership record. |
| OpenClaw (`openclaw`) | Updates `~/.openclaw/agents/main/agent/models.json`. The legacy alias `claw` uses the same adapter. | OpenClaw installed. Registers under the `main` agent; does not change its default model or other agents. |
| PicoClaw (`picoclaw`) | Adds models to the v2 schema in `~/.picoclaw/config.json`, stores keys under `model_list` in `~/.picoclaw/.security.yml`, and tracks ownership in `~/.config/ropero/connect/picoclaw.json`. | PicoClaw v2. Other `version` values are rejected rather than migrated. Does not change the default model. |
| VS Code (`vscode`) | Registers the endpoint in `chatLanguageModels.json` using the current `customendpoint` provider. | The file contains no API keys. If the engine requires authentication, add the credential through Manage Language Models in VS Code. BYOK covers chat and utility tasks, not inline completions, semantic search, or embeddings. |

Automatic integrations preserve fields they do not manage, reject invalid
formats before rewriting, and write files containing keys with private
permissions on Unix. Connecting configures the client; it does not install the
agent or start the engine. Start the model with Ropero first. Clients and
engines may have their own API, tools, reasoning, and multimodal limitations.

## Format references

- [OpenCode: custom providers](https://opencode.ai/docs/providers/)
- [Codex: provider configuration](https://developers.openai.com/codex/config-advanced/)
- [Claude Code: LLM gateways](https://docs.anthropic.com/en/docs/claude-code/llm-gateway)
- [Pi: models configuration](https://docs.pi.dev/latest/models-configuration)
- [Hermes: providers](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md)
- [OpenClaw: custom providers](https://docs.openclaw.ai/gateway/config-tools/custom-providers)
- [PicoClaw: custom models](https://docs.picoclaw.io/docs/configuration/model-list/)
- [VS Code: AI models and Custom Endpoint](https://code.visualstudio.com/docs/agent-customization/language-models)
- [Zed: API access and compatible endpoints](https://zed.dev/docs/ai/use-api-access)
- [Cursor: API keys and supported providers](https://cursor.com/docs/models-and-usage/api-keys)
- [MicroClaw: example configuration](https://github.com/microclaw/microclaw/blob/main/microclaw.config.example.yaml)
- [Graphify: installation and supported assistants](https://github.com/Graphify-Labs/graphify)

This guide describes Ropero's implemented behavior and the upstream documented
formats. It does not certify every application version; Hermes also requires
its CLI to be available on the machine.

## Other IDEs and analysis tools

These products have their own configuration and are not automatic
`ropero connect` destinations in this version:

- **Zed:** supports OpenAI-compatible endpoints. Open Agent Settings with
  `agent: open settings` → LLM Providers → Add Provider; enter a name, URL
  `http://127.0.0.1:<port>/v1`, model ID, and context window. Zed stores the
  key in the system keychain, not `settings.json`. You can also define
  `language_models.openai_compatible` in that file and add the key in Agent
  Settings.
- **Cursor:** its official documentation lists API keys for supported
  providers but does not confirm direct connections to Ropero's local
  endpoints. For that reason, Ropero does not claim a compatible integration.
  Cursor Tab continues to use Cursor's built-in models.
- **MicroClaw:** manually configure `llm_provider`, `model`, `api_key`, and
  `llm_base_url` in `microclaw.config.yaml` (choose `openai` for an
  OpenAI-compatible server and set the engine's local URL). Preserve channel
  tokens and other existing settings; protect this file because it contains
  secrets. MicroClaw also needs its own channel/bot configured.
- **Graphify:** optional tooling that builds a repository graph to enrich
  analysis in compatible assistants. Install it using its guide and use it
  from Cursor, Codex, OpenCode, or another supported host; configure the LLM
  and its credentials in that host. Graphify is not a Ropero engine or model
  and is not configured by `ropero connect`.

OpenClaw's integration remains implemented. Its configuration reference can
be updated when the user provides the specific documentation URL they want to
use.
