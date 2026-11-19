# Ropero CLI v2 — current contract

**Status:** English companion to the [Spanish specification](ropero-cli.md). This document records the public CLI and distinguishes implemented behavior from known gaps. The executable and tests are authoritative when this document and the code differ.

## Scope and conventions

The public interface is `ropero`. Help text and configuration support Spanish (`ES`) and English (`EN`); English is the default when no language is configured. `ropero config --lang ES|EN` persists the selection in `config.toml`.

Exit codes used by the CLI are `0` for success, `1` for execution/configuration failure, `2` for invalid usage, and `130` for an interrupt.

## Commands

| Command | Purpose |
|---|---|
| `ropero` | Short machine, engine, and model summary. |
| `ropero --help` | Command list; `ropero <command> --help` shows command help. |
| `ropero install [engines\|models] [--force] [--prio download\|build] [items…]` | Installs engines and/or models. Interactive selection requires a TTY; explicit item names support automation. |
| `ropero stop [model]` | Stops Ropero-managed engines, all or for one model. |
| `ropero <model> [run options] [model options]` | Starts a catalog model. |
| `ropero gguffer [source options]` | Builds or quantizes GGUF artifacts. |
| `ropero status [--live [seconds]]` | Shows machine status; live refresh defaults to one second and requires a terminal. |
| `ropero doctor` | Reports configuration and engine dependencies. |
| `ropero config [--lang ES\|EN]` | Shows resolved paths and sources, and optionally saves the interface language. |
| `ropero connect --list` | Lists supported coding-agent integrations. |
| `ropero connect --model models --agent agent [--remove]` | Adds or removes model connections for an agent. Model names may be comma- or semicolon-separated. |
| `ropero models [--all | --fired]` | Lists available models from installed engines by default; an installed engine with no available models is shown with a note. `--all` includes every engine and active catalog model, including unavailable entries; `--fired` lists deactivated models. These options are mutually exclusive. |
| `ropero model <model> --fired` / `--hired` | Deactivates or reactivates a model by moving its TOML into or out of `fired/`; it does not delete the TOML or artifacts. Repeating a transition or selecting a nonexistent model is rejected. If the name is already active when hiring, Ropero asks for a new name that is unique among active models; cancelling leaves the model fired. |
| `ropero models add [creation options]` | Registers a normalized, validated model TOML. |
| `ropero engines [--all]` | Lists engines and compatibility; `--all` includes incompatible engines. |
| `ropero validate` | Validates the active model and engine catalog. |
| `ropero command <model>` | Prints the complete launch command, with shell continuations, for copying. |
| `ropero info <model>` | Prints model identity, engine-facing alias, artifact paths, download/build metadata, levels, flags, options, and mapped arguments. |
| `ropero test [--model ID] [--group A..G] [--test ID] [--dimension NAME] [--all] [--json]` | Runs selected evaluation tests against an available model. |

The old public commands `list`, `args`, `levels`, and `path` are not part of this interface. Use `models`, `command`, and `info` instead.

## Model registration

`ropero models add` has four input modes:

1. Interactive wizard: `ropero models add` in a terminal.
2. Field-based mode: `--name`, `--engine`, `--format`, and `--artifact` are required; `--repo`, `--context`, `--alias`, and repeatable `--set FIELD=VALUE` are optional.
3. File mode: `ropero models add --path model.toml`.
4. Standard input: `cat model.toml | ropero models add --stdin`.

File and stdin modes parse and validate the model structure. All modes produce a normalized TOML in the model catalog. Unknown fields are discarded with a warning. The wizard and field mode do not read the editable template file. Existing model names cannot be overwritten.

The model's internal `name` must match its TOML filename. `alias` is the public engine-facing id where that engine supports a separate id. Every declared item in `levels.list`, `flags.list`, or `options.list` must have a corresponding `[args.level.*]`, `[args.flag.*]`, or `[args.option.*]` mapping; validation rejects advertised entries without mappings.

## Model launch and inspection

Common launch options are `--force`, `--cpu`, `--port N`, `--api-key KEY`, `--no-api-key`, `--ctx N`, `--log-level LEVEL`, and `--background`. The CLI also accepts the levels, flags, and options declared by that model's TOML and maps them to engine arguments. A model-specific setting only works when the TOML maps it to a flag supported by that engine.

`info` is the source for model metadata and its effective argument mapping. `command` prints the generated default engine command; it may require engine-specific prerequisites. For Strata, an installed profile must already exist. Ropero does not create or download Strata profiles.

For `status --live`, zero/negative intervals are rejected, and output is redrawn in place. Status includes swap and zram information and a table of Ropero-managed live engines with port, PID, and model when any are running.

## Agent connections

OpenCode removal uses `--remove`:

```text
ropero connect --model MODEL --agent opencode --remove
```

Claude Code uses the Anthropic Messages-compatible endpoint exposed by the selected engine. The command prints the required environment setup: `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `API_TIMEOUT_MS`, `DISABLE_TELEMETRY`, `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, and `ANTHROPIC_DEFAULT_SONNET_MODEL`. The base URL must point to the selected local gateway, for example `http://127.0.0.1:9984`.

## Evaluation output

The runner asks for a model when `--model` is omitted; test/group/dimension selectors are provided as flags and are not offered through an additional selector menu. It creates a Markdown report in the configured language under the Ropero state directory with three sections:

1. Statistics calculated by Ropero, including memory samples.
2. The engine log, when it can be read.
3. The model transcript, returned reasoning, and tool calls/results. Calls to delegated subagents link to their prompt, response, reasoning, and tool trace sections. Delegated subagents cannot delegate recursively.

`--json` writes the structured evaluation report to stdout and human-readable text to stderr.

## Engine and alias boundaries

Engine launch, installation, artifact ownership, and accepted flags are engine-specific. See the [bilingual engine compatibility guide](../docs/engine-support.en.md) for evidence and primary documentation links. Ropero applies a TOML alias to Strata's public `aliases` list in a private run-config copy; FLM uses the alias as its registered tag. The Ropero runtime currently marks AirLLM as partial rather than a verified serving integration.

## Remaining limitations

- Local execution has not verified vLLM, MLX-Serve, or TensorRT-LLM in this environment; their manifest entries and vendor documentation are not equivalent to an end-to-end tested model.
- AirLLM has no Ropero server/launch adapter; its catalog entries must not be treated as working models.
- The full workspace test suite has had environment-dependent socket-bind failures in the restricted test sandbox; focused CLI and catalog tests provide narrower evidence.

## Related specifications

- [Spanish CLI specification](ropero-cli.md)
- [System requirements](ropero-especificacion.md)
- [Build and installation specification](ropero-construccion.md)
- [Evaluation test index](../docs/tests/indice.md)
- [Engine compatibility guide (English)](../docs/engine-support.en.md)
