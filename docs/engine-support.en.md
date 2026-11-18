# Engine compatibility

[English](engine-support.en.md) · [Español](engine-support.es.md)

This guide describes what Ropero implements today and separates local checks from vendor documentation. An engine manifest or a catalog format is not proof that launching the model works.

## Local evidence

The checkout was reviewed on a machine with `llama-server`, `turbo-llama-server`, `flm`, `ignis-server`, and `strata`; their version or help output was inspected. `vllm`, `mlx-serve`, `trtllm-serve`, and `airllm` were absent, so those four were not tested locally. These observations describe that machine and those versions, not every Ropero installation.

## Engines and known limits

| Engine | What Ropero does | Evidence and limits |
|---|---|---|
| `llama-server` | Launches the binary with TOML arguments, adds the GGUF when needed, and sets `--alias` to the public alias or Ropero name. | Binary and help checked locally. Official documentation describes `-m/--model`, `--alias`, `--host`, and `--port`. |
| `turbo-llama` | Uses the `turbo-llama-server` fork with the same GGUF argument and alias path as `llama-server`. | Fork binary and help checked locally. Upstream llama.cpp documentation does not guarantee that every option exists in the fork; verify against the installed version's help. |
| `fastflowllm` / FLM | Delegates `pull`, `remove`, `list`, and `serve` to `flm`. The registered tag is `[model].alias` when set; otherwise it comes from `[model].path`, or from `--decision` in `[args].base` when `path` is empty. Ropero does not download that artifact into its own model directory. | Local `flm` checked. The official CLI uses tags, with `flm pull <tag>`, `flm serve <tag>`, and `flm remove <tag>`. Format, NPU, and platform support depend on FastFlowLM/XRT. |
| `ignis` | For `ninfer`, runs `ignis-server model download <id> --out <ARTIFACTS_DIR>/<name>` and serves the artifact from that directory. At launch it passes `--model-id <alias>` when the TOML declares `alias`. | Local `ignis-server` checked. Ignis documents catalog downloads with `--out` and the id exposed by the API. Ropero recognizes only the declared ids/artifacts for included Ignis models; this does not imply arbitrary model compatibility. |
| `strata` | Finds an installed profile with `strata models list --installed --output json`, maps `[model].path` to its profile id (for example, `gyro/Gyro-S` → `gyro-gyro-s`), then launches `strata serve --profile …`. Model options and flags are appended. When `[model].alias` is set, Ropero creates a private run-config copy with that id in `aliases`, leaving the source unchanged. Ropero format `strata` represents a Strata-prepared profile. | Local `strata serve` help confirms the mapped options (`--parallel`, `--batch-groups`, `--max-queue`, `--queue-timeout`, `--batch-telemetry`, `--offline`). `gyro-s` is detected as installed; Strata lists `gyro-m` but reports missing components. Ropero does not download or create profiles. |
| `vllm` | The runtime can pass model arguments and adjusts `--log-level`; the expected binary is `vllm`. | Not installed locally; no real launch was tested. Official documentation describes `vllm serve` and its options, but the current catalog has no vLLM model example with an end-to-end verified recipe. Treat as integrated/documented, not proven. |
| `mlx-serve` | The runtime passes `--model` with the resolved local path, pins `--host 127.0.0.1`, adds the allocated port, and appends TOML arguments. For safetensors models with `[model].alias`, it writes the alias to `~/.mlx-serve/model-settings.json`, preserving other settings. | Not installed locally; no real launch was tested. Upstream documents aliases in release v26.10.1; use that version or newer. Project requires macOS and Apple Silicon. Maturity is `integrated`, not `proven`. |
| `tensorrt-llm` | The expected binary is `trtllm-serve`; model arguments come from the TOML. | Not installed locally; no real launch was tested. NVIDIA documentation describes `trtllm-serve`; compatibility depends on version, GPU, runtime, and format. `groot-trt` is kept under `models/tensorrt/fired/`: TensorRT-LLM 1.4.0rc2.post1 rejected its NVFP4 checkpoint. Maturity is `integrated`, not `proven`. |
| `airllm` | Ropero runtime has no server or launch adapter. Its manifest is marked `partial`; TOMLs under `models/airllm/` must not be read as an operational integration. | Not installed locally and serving was not tested. AirLLM documents a Python library for layer-wise inference; no server/API implementation that Ropero can launch was found in this repository. |

## TOML argument contract

`levels.list`, `flags.list`, and `options.list` advertise interface values. Each entry must have its matching `[args.level.<id>]`, `[args.flag.<id>]`, or `[args.option.<id>]` block; validation rejects entries without a mapping. Options must map to flags accepted by that specific engine CLI. Ropero does not automatically translate one engine's options into another engine's syntax.

Common launch controls (`--port`, API key, and log level) are handled by the runtime only where a specific engine adapter exists. Do not assume that the same flag or behavior exists across all engines.

## Model aliases

`[model].name` identifies a model inside Ropero and must match the TOML filename. `[model].alias` is the server-facing public id where the engine permits it. Ropero applies it to llama.cpp (`--alias`), Ignis (`--model-id`), Strata (`aliases` in its run config), FLM (the registered tag used by pull/serve/remove), and MLX-Serve (the per-model entry in `~/.mlx-serve/model-settings.json` for safetensors models). For vLLM and TensorRT-LLM, Ropero maps it to their served-model-name options. Engines without a documented public-id mechanism keep their native model id.

## Primary sources

- [llama.cpp: HTTP server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [Turbo-Llama: fork source](https://github.com/Lorenzo-SF/turbo-llama)
- [FastFlowLM: CLI instructions](https://github.com/ROCm/FastFlowLM/blob/main/docs/docs/instructions/cli.md)
- [Ignis: download and agent configuration](https://github.com/gpillon/ignis/blob/main/docs/user/paths/agent.md)
- [Strata: terminal commands](https://github.com/Niko1221/Strata/blob/main/docs/TERMINAL.md)
- [Strata: run-config aliases](https://github.com/Niko1221/Strata/blob/main/serve/runconfig.py)
- [vLLM: `vllm serve`](https://docs.vllm.ai/en/latest/cli/serve/)
- [MLX-Serve: CLI and server flags](https://github.com/ddalcu/mlx-serve/blob/main/docs/cli.md)
- [MLX-Serve: per-model alias support](https://github.com/ddalcu/mlx-serve/pull/612)
- [MLX-Serve: model settings keyed by absolute model path](https://github.com/ddalcu/mlx-serve/blob/main/src/host/registry/model_settings.zig)
- [MLX-Serve: v26.10.1 release](https://github.com/ddalcu/mlx-serve/discussions/672)
- [TensorRT-LLM: `trtllm-serve`](https://nvidia.github.io/TensorRT-LLM/commands/trtllm-serve/trtllm-serve.html)
- [AirLLM: official repository](https://github.com/lyogavin/airllm)
