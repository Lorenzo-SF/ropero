---
title: "ropero — Specification and requirements"
subtitle: "The contract implemented by the code, written as if built from scratch"
author: "Extracted from the Lorenzo-SF/ropero implementation @ f21b442"
date: "2026-11-12"
lang: en
---

# ropero — Specification and requirements

> **Target functional contract for Ropero Rust.** This document incorporates
> the owner review received on 2026-11-16. If an earlier requirement
> conflicts with that review, this section takes precedence until it is
> formally integrated into the existing sections.

---

## 0.4 Subsequent CLI and configuration specification (2026-11-16)

This section records the later specification provided by the owner. It is
normative for the Rust implementation. Items marked as pending here are not
considered implemented merely because they appear in documents, manifests,
or tests.

### CLI

**R-CLI-18 · Summary with no arguments.** `ropero` displays its name, version,
build date and author; engine and model availability/installation; and a
prompt to consult `ropero --help`.

**R-CLI-19 · Navigable help.** `ropero --help` includes the summary, available
commands with short descriptions, and points to `ropero <command> --help` for
details. Each subcommand has its own help. The visual identity must be new,
clear, and consistent; it must not copy the v1 presentation literally.

**R-CLI-20 · Model launch.** `ropero <model>` requires an installed engine, an
installed model, and sufficient resources on the selected device. `--cpu`
requests CPU/RAM only for engines that support it; the expected contract
currently covers llama-server. `--force` may free resources by terminating
Ropero processes occupying the device, never unrelated processes. `--port`,
`--api-key`/`--no-api-key`, and `--ctx` override the port, authentication, and
context.

**R-CLI-21 · Interactive and unattended installation.** `ropero install`
installs engines and then models. `install engines [ids...]` and
`install models [ids...]` offer interactive selection when no list is given
and accept space- or comma-separated lists for automation. Ids and aliases
are validated before the system is modified.

**R-CLI-22 · Stop.** `ropero stop` stops every engine launched by Ropero;
`ropero stop <model>` stops only that model's instance and preserves other
models, even when they use the same engine.

**R-CLI-23 · GGUF.** `ropero gguffer` accepts a Hugging Face source
(`--repo`) or local source (`--path`), a list of quants, a local or remote
imatrix (remote repository and filename as a pair), output path, intermediate
precision, source/intermediate retention, exclusion of MTP/DFlash headers,
`--force`, quant listing, and diagnostics. It must accept safetensors or GGUF
inputs, including weights that are already quantized or F16/BF16 and need no
quantization. A compiled official llama-server engine is required. Strata-
specific preparation remains in the reference scripts/models declared for
that engine and is not assumed to be covered by the generic pipeline.

**R-CLI-24 · Status.** Displays CPU, RAM/swap, and available GPU telemetry:
dGPU and, on Linux, iGPU when present; on macOS the unified GPU is shown as
GPU. Unavailable values are identified as such.

**R-CLI-25 · Test.** Runs test suites against installed models and offers
selection by individual test, group/type, or all tests, according to
`docs/tests/indice.md`; it explains what each selection measures and retains
results.

**R-CLI-26 · Doctor.** Checks Ropero installation and dependencies, including
whether Ropero is on `PATH`, and checks dependencies for every engine,
installed or not, showing what is missing and how to resolve it.

**R-CLI-27 · List/template.** `list` shows only downloaded models whose
engine is installed, grouped by engine and using the visual legend.
`template` prints the complete TOML with usage comments.

### Configuration and artifact ownership

**R-CFG-1 · Per-engine configuration.** `~/.config/ropero/config.toml` (or
the equivalent XDG path) allows per-engine configuration of the model
directory, model TOML directory, and port range. Precedence, defaults, range
validation, overlaps, and migration from the current configuration must be
explicit. The existing global schema alone does not satisfy this requirement.

**R-CFG-2 · Declarative installation.** Manifests may describe how to
install/build each engine, its dependencies, and supported package managers.
Pre/post-install commands or scripts are included only with a security
contract, consent, reproducible output, and platform coverage. The engine
list and installation recipes are fixed and maintained by the project; they
are **not user-configurable**. User configuration may change presentation
(color), per-engine paths, and port ranges. Color does not determine
compatibility or state.

**R-MOD-32 · Shared paths.** Multiple engines may share a storage root. A
shared directory does not imply exclusive ownership: models, auxiliaries
(MTP, templates, mmproj, DFlash, imatrix), and derived files must be tracked
so installation and deletion do not interfere with one another.

**R-MOD-33 · Reuse of identical artifacts.** If two models need the same
GGUF, identified by source/repository, revision or content identity, final
file, and transformation/build, Ropero reuses the same bytes and does not
download/build them twice. Identity cannot rely on basename alone. If they
share only a root but not an artifact, they remain isolated, for example in
per-model subdirectories.

**R-MOD-34 · Referential deletion.** Removing a model removes its references
and exclusively owned artifacts; it does not remove shared bytes while
another valid reference needs them. Broken or orphaned references must be
recoverable without following symlinks outside the permitted root.

**R-MOD-35 · Confirmation for shared weights.** `remove` reports which other
registered models use shared weights/artifacts. If other users exist, it
preserves the bytes and removes only the requested model association. If the
model is the last user, it asks for confirmation before deleting the weights;
declining or lacking a TTY has no side effects.

**R-ENG-1 · Project-maintained integrations.** Supported engines form a
fixed list controlled by project code and manifests. Each integration
documents platforms, requirements, installation, download/store, serving
argv, readiness, shutdown, and formats/API. Users may adjust color, paths,
and ports, but not repositories, commands, dependencies, or installation
hooks.

**R-ENG-2 · Installation diagnostics.** `install engines` reports actionable
errors by default and offers a detailed mode containing context or a Rust
backtrace and the subprocess failure without leaking secrets. It must be
possible to enable this with `RUST_BACKTRACE=1` or an explicit flag.

**R-ENG-3 · Colibri.** Add Colibri as a supported engine. Implement its
installation and lifecycle according to documented upstream capabilities,
model selection, and OpenAI-compatible API. Do not duplicate its store or
downloads unnecessarily. CUDA, Vulkan, and Metal capabilities vary by model
and backend and must be stated precisely.

**R-ENG-4 · AirLLM (deferrable decision).** Before declaring the project
finished, decide and document the minimum AirLLM bridge: an isolated Python
process with a compatible HTTP API or another integration. Evaluate
packaging, CUDA dependencies, model management, and lifecycle. This decision
does not block closing the remaining engines and features.

**R-ENG-5 · Turbo-llama.** `turbo-llama` is a supported engine integrated as a
`llama-server` fork, with the same capabilities, platforms, and build
arguments on Linux and macOS. User availability does not depend on the
repository being publicly downloadable. Code must reference the source/
repository configured for the fork rather than replacing it with another
fallback. If cloning fails due to permissions or access, the error must
identify that cause; the engine recipe and support remain part of the
product.

### Initial inspection of the Rust tree (2026-11-16)

| Requirement | Observed evidence | Initial status |
|---|---|---|
| R-CLI-18/19 | Dispatcher/parser implemented; summary in transition | Partial |
| R-CLI-20/22 | Runtime modules present with active changes | Partial; confirm resource and ownership gates |
| R-CLI-21 | Menu/list present; manifest-based installation in progress | Partial; a CMake failure was found and fixed in runtime |
| R-CLI-23 | `ropero-forge` and gguffer CLI exist | Partial; quantized-GGUF/Strata cases need an acceptance matrix |
| R-CLI-24/25/26/27 | Battery, system info, doctor, list, and template crates present | Partial; validate semantics against the new contract |
| R-CFG-1 | Global config exists, but not per-engine or per-port | Pending |
| R-CFG-2/R-ENG-1 | Manifests and installation in progress; recipes for all engines not audited | Partial |
| R-MOD-32/33/34 | Per-model paths and safe deletion are active work; deduplication/references not found | Pending |
| R-MOD-35 | `remove` is active work; shared-weight warnings and last-user confirmation need validation | Pending |
| R-ENG-2 | Installation has no explicit detailed/backtrace mode | Pending |
| R-ENG-3 | No `EngineId` or Colibri manifest exists | Pending |
| R-ENG-4 | AirLLM uses a Python library API; integration deferred, analysis required for closure | Deferrable pending |
| R-ENG-5 | `EngineId` and manifest exist; verify the configured source is the right fork and build when access is available | Partial (repository inaccessible from this environment) |

This table is a snapshot of static inspection, not an acceptance-test
statement. Update it as each requirement closes with code and test evidence.

---

## 0. How this document was built

> **Status note (2026-11-16):** the historical text in this section describes
> the original extraction from v1. The later CLI specification and
> acceptance criteria are recorded in §4.14; requirements marked there are
> targets, not claims that the code already satisfies them.

This document is **not a plan**: it is the specification that the
implementation already satisfies. It was extracted by reading the code and
its comments, and each requirement includes the evidence (`file:line`) where
it is demonstrated.

### 0.1 The gap this document fills

Ropero's code cites a specification throughout—`spec §2.1`, `§5.5`, `§7.3`,
`§8.13`, `ADR-1`—and `README.md` points to `docs/specs/` as the “1.0 refactor
contract.”

**That document does not exist in the repository.** `docs/` contains the test
battery (`prueba*.md`), the judge, and two guides (`flm-memoria.md`,
`strata.md`), but not `docs/specs/`. The README also cites
`tests/suite/run.sh`, while the actual battery lives under `test/test-*.sh`.

In other words, the specification is real—the code implements and cites
it—but it was left out of the repository. This document reconstructs it.

### 0.2 Method

1. Complete read of the core: `bin/ropero`, `bin/lib/*.sh`,
   `bin/lib/engines/*.sh`, `bin/python/modelconf.py`, `install.sh`,
   `models/_template.toml`, `README.md`, `CHANGELOG.md`, `docs/strata.md`.
2. Parallel extraction of the perimeter: the GGUF factory (`cmd-gguffer.sh`,
   `bin/quant/`), endpoint and router (`bin/server/`), installation and model
   management (`cmd-install.sh`, `cmd-models.sh`, `install.sh`), and test
   battery (`test/*.sh`).
3. Read the companion `groot-strata.sh` script, integrated as a requirement
   for the “external GGUF served by Strata” path (§ 4.13). This script is
   **external and not versioned in this repository**; therefore its 13
   citations cannot be checked locally against a repository file.
4. **Owner interview** (2026-11-12): project origin (official llama.cpp →
   custom fork with TurboQuant/NextN/NVFP4 → MLX, FLM, TensorRT, AirLLM,
   vLLM, Strata), the two platforms (macOS and CachyOS), the purpose of each
   command (`install`, `stop`, `gguffer`, `stats`), and TOML vocabulary.
   Requirements marked 🔶 in this document come from that interview: they
   describe the **target state**, not today's code.

### 0.3 Conventions

- **MUST / MUST NOT**: normative requirement. Each has evidence.
- **R-XXX-n**: stable requirement identifier, by area.
- Requirements without local evidence are marked **[derived]**: inferred from
  observed behavior and reasons recorded in comments, but not covered by a
  test.
- **🔶** = requirement for the **target state**, taken from the owner's
  statement (2026-11-12) and **not yet implemented in code**. The divergence
  from code is in § 6.
- Requirements not guaranteed by the repository are listed in § 6 (gaps),
  not here.

### 0.4 Repository state analyzed

| Item | Value |
|---|---|
| Commit | `f21b442` (“chore: remove dead farm hop from endpoint.py and test-strata (F8)”) |
| Engines | 8 |
| Active models | 17 (across 7 engine folders) |
| Retired models (`fired/`) | 28 |
| Test files | 16 |
| Size | 11,022 lines of Bash + 4,188 of Python |

---

## 1. What ropero is

### 1.1 Thesis

**Ropero is a CLI that serves local language models with the engine that
performs best on each machine**, and **a model is a data file, not code**.

That leads to the two halves of the product:

- A model's **configuration** (which binary, flags, context, reasoning
  levels, required files) lives in TOML.
- **Code** does not change when the engine or model changes: add a file under
  `models/<engine>/` and, when needed, add an engine under
  `bin/lib/engines/`.

> “If the engine changes, the data changes; the code stays untouched.”
> — `ropero-code/README.md:9-10`

**Where it came from.** Ropero began with **official llama.cpp** (from
`ggml-org`), and engines were added from there: the custom fork with
**TurboQuant**, **NextN** changes (an MTP variant), and **NVFP4** fixes for
NVIDIA; **MLX** (Apple's Metal engine, used with the `mlx-serve` application);
**FastFlowLM**, for the AMD **XDNA NPU**; **TensorRT-LLM** (NVIDIA);
**AirLLM**; **vLLM**; and **Strata**, which is partially integrated.

The reference platforms are **macOS** and **CachyOS** (Arch-based). This is
not decorative: **each engine declares compatibility**, and `install
engines` does not offer Metal on Linux or NVIDIA/AMD NPU engines on macOS.

> “in the end, everything should be well implemented, correctly integrated,
> and working” — the project's stated goal

### 1.2 Scope

Ropero solves six things:

1. **Catalog:** which models exist, where they come from, and how to serve
   them.
2. **Lifecycle:** start a model, know when it is ready, stop it, stop it
   again, and never kill unrelated processes.
3. **Ports and state:** a model serves on a port; that port is the system's
   operational identity.
4. **Single gateway:** every engine is exposed behind the same
   OpenAI-compatible API and the same key.
5. **Build:** turn weights into a GGUF ready to serve, with imatrix and
   verification.
6. **Diagnostics:** inspect system and engine state and collect it in a file
   for review or sharing.

### 1.3 Explicit non-goals

| Ropero does NOT… | Why | Evidence |
|---|---|---|
| …download Strata models | Strata has no “download without starting” mode: it downloads a model when the engine starts. Ropero shows the **plan** and says so. | `ropero-code/bin/lib/engines/strata.sh:214-233` |
| …touch Strata files | Weights, n-gram table, engine, and directory belong to Strata. | `docs/strata.md`, “What ropero does NOT do” |
| …manage the user's Strata presets | Presets created by Ropero are named `ropero-<model>` and only that preset is stopped. | `docs/strata.md`, “The three pieces” |
| …tune the operating system | Swappiness, THP, DPM, kernel cmdline, and ananicy belong to `Lorenzo-SF/setup`. | `CHANGELOG.md`, “Outside ropero” |
| …act as a sourceable library | It is a pure CLI. Sourcing does nothing and does not alter shell options. | `ropero-code/bin/ropero:41-55` |
| …kill processes it does not own | `stop` kills **by port**, after verifying that the PID belongs to the engine. | `ropero-code/bin/lib/04-launch.sh:450-536` |

---

## 2. Design principles

These principles recur in the code and explain most concrete decisions. They
cannot be inferred from a function list alone.

### P-1 · Evidence takes precedence over assumptions

Code does not assume system state: it **measures** or **asks**. A successful
`command -v` is not enough; the interpreter is actually started.

> “a broken asdf shim passes `command -v` and then fails: test it, don't
> assume” — `ropero-code/bin/lib/02-modelconf.sh:60-63`

### P-2 · A silent failure is worse than a visible error

Whenever a check could silently degrade, code duplicates it with an
independent method. Canonical example: `lsof` may be missing, so `lsof
-iTCP` fails and always says “free” → Ropero starts a server over the
existing one, and `stop` kills nothing.

> “It is a SILENT failure that reads as ‘models won't start’”
> — `ropero-code/bin/lib/04-launch.sh:52-64`

For that reason there are **three** ways to ask who is listening on a port:
`lsof` → `ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd`.

### P-3 · Never kill unrelated processes

`stop` kills by port, never with a global `pkill -f` (which would kill
parallel slots). Before killing, it checks that the PID in state **is** the
engine: the OS recycles PIDs, and stale state could point to a browser.

> “Killing it would kill an innocent process”
> — `ropero-code/bin/lib/04-launch.sh:474-483`

### P-4 · Disk state is the source of truth

Listening on a port does not tell you who launched the process. Ropero
**knows** which model it launched on each port and writes it to
`$STATE_DIR/ports/<port>`. Without that file, `status`, `stop`, and `use`
idempotency would not work across processes.

> “what listens on a port does not tell you who launched it (and sometimes it
> does not even publish an id that maps to the catalog)”
> — `ropero-code/bin/lib/04-launch.sh:16-20`

### P-5 · A failed start is cleaned up

If the model does not become ready in time, Ropero **kills the process it
started** and cleans up state. Previously it warned and left it alive: the
server could finish loading and start serving requests, leaving a zombie on
the port.

> “Ropero ate the 300s timeout and then killed a perfectly healthy server”
> — `ropero-code/bin/lib/04-launch.sh:725-736`

### P-6 · Idempotent by default

`use` is idempotent: if the model is already serving with **the same
configuration**, it changes nothing and exits in milliseconds. If the
configuration changed (the *stamp* is the args' `cksum`), it restarts;
continuing with the old config would be a silent lie.

### P-7 · Do not claim what cannot be checked

The validator distinguishes **problems** (fatal) from **warnings**
(informational). An empty `[args].base` in an engine with its own server is an
omission, not an error: warn and allow it.

> “Problems fail `validate` (exit 1). Warnings are printed but do not fail.”
> — `ropero-code/bin/python/modelconf.py:1417-1421`

### P-8 · Ropero supplies one key

All engines share `LLAMA_API_KEY`. Engines that do not enforce it (Strata,
FLM) start without it and Ropero's proxy requires it: one key, no
contradictions.

### P-9 · The user decides; the script executes

Choosing a model from the menu **is** consent to download it; do not ask
again. VRAM-occupied warnings do not kill anything: they inform the user, who
decides.

### P-10 · Portability is a design constraint

Bash 3.2 (macOS `/bin/bash`): no `nameref`, `declare -A`, `mapfile`,
`readlink -f`, or `${var^^}`. Without namerefs, the catalog uses **parallel
arrays**, with the index as the position.

### P-11 · Sourcing executes nothing

All modules are safe to source: definitions and defaults only, no execution.
`rp_env_setup` (which creates directories) is called only by the command that
needs it: `help` and `--version` do not touch disk.

### P-12 · Comments explain why

Comments do not describe the line (the code already does that); they explain
why it is written that way and which concrete failure it prevents. That is the
difference between a system one can defend aloud and one that cannot.

---

## 3. Context and constraints

### 3.1 Platforms

| | macOS (Apple Silicon) | CachyOS / Arch (Linux) |
|---|---|---|
| Bash | **3.2** (system version) | ≥ 3 |
| GPU | Metal (`mlx-serve`) | CUDA (llama-server, TensorRT-LLM) |
| NPU | — | AMD XDNA (FastFlowLM) |
| CPU slot | **does not exist**: `--cpu` is a usage error | `:9998` |
| Dependency installer | `brew` | `pacman` |

Detection: `rp_detect_os()` → `macos | cachyos | arch | linux | other`
(`ropero-code/bin/lib/01-env.sh:80-99`).

### 3.2 Supported engines

| id | What it is | OS / hardware | Base port | Native auth |
|---|---|---|---:|---|
| `llama-server` | TurboQuant fork of llama.cpp (main engine) | macOS (Metal) / Linux (CUDA) | `:9999` | yes |
| `llama-server-official` | Upstream llama.cpp (`ggml-org`) | macOS / Linux (CUDA) | `:9989` | yes |
| `fastflowlm` | FastFlowLM on AMD NPU (XDNA) | **Linux only**, AMD NPU | `:9979` | no (proxy provides it) |
| `mlx-serve` | mlx-lm on Apple Silicon (through `mlx-serve` app) | **macOS only** (arm64) | `:9969` | yes |
| `tensorrt-llm` | TensorRT-LLM | **Linux only** + CUDA | `:9959` | no |
| `vllm` | vLLM | **Linux only** + `nvidia-smi` | `:9949` | no |
| `airllm` | AirLLM (library only, no server) | macOS / Linux | `:9939` | no |
| `strata` | Strata (MoE across VRAM/RAM/SSD) | Linux / macOS | `:9929` | no |

*(table from `ropero-code/bin/lib/04-launch.sh:167-180` and
`ropero-code/bin/lib/03-engines.sh:38`; canonical ids and legacy aliases are
ordered in `ropero-code/bin/lib/03-engines.sh:54-64`; platform column is
compatibility declared by each engine in `bin/lib/engines/<id>.sh`, hook
`compatible`)*

**R-CTX-1** · `install engines` offers **only engines compatible with the
operating system and hardware**: Metal is not offered on Linux; FLM,
TensorRT-LLM, and vLLM are not offered on macOS. Compatibility is declared by
the engine itself (hook `compatible`), not by a central table. Evidence:
`ropero-code/bin/lib/03-engines.sh:5-17`, `ropero-code/bin/lib/cmd-install.sh:495-499`.

**R-CTX-2** · `install models` offers **only models whose engines are
installed**: if the catalog has 80 models but only one engine is installed,
only that engine's 15 models are offered. Evidence:
`ropero-code/bin/lib/cmd-models.sh:136-145`, `:602-767`.

**R-CTX-3** · 🔶 **Engines and models do not all have the same maturity**, and
the specification must not pretend otherwise. At the time of writing:

| Status | Coverage |
|---|---|
| **End-to-end tested** | `llama-server` (TurboQuant fork), `mlx-serve`, `fastflowlm` (verified on the HX 370) |
| **Integrated but not tested by the owner** | `airllm`, `tensorrt-llm`, `vllm`, `llama-server-official` |
| **Partially integrated** | `strata` (target: first-class engine, R-STRATA-36) |
| **Broken guarantee mechanism** | endpoint (`endpoint.py`): untested and its auth test falsely passes (R-TST-18) |

The principle supporting this table is **P-1** (evidence over assumption): an
installed engine is not a tested engine, and documentation must say which it
is.

### 3.3 Reference hardware

Repository figures were measured on the **MSI Stealth AI+ A16**: RTX 5080
Laptop (15.93 GB VRAM), 30.45 GiB RAM, CachyOS. This is the source for
Strata's `--vram-reserve-mib 1344`, the 228 t/s for `strata-qwen36-q4`, and
the context limits.

The **NPU** has its own reference: **AMD Strix Point HX 370**, where
FastFlowLM is tested and “works perfectly.” It is the only hardware/engine
combination verified end-to-end on the NPU.

### 3.4 Non-functional constraints

| id | Constraint | Evidence |
|---|---|---|
| R-NF-1 | **Bash 3.2**: no `nameref`, `declare -A`, `mapfile`, `readlink -f`, `${var^^}` | `ropero-code/README.md:199-200`, `ropero-code/bin/ropero:20-22` |
| R-NF-2 | **Comments and messages in Spanish** | `ropero-code/bin/ropero:23-24` |
| R-NF-3 | Dispatcher does not alter shell options: neither `errexit` nor `nounset` | `ropero-code/bin/ropero:39-55` |
| R-NF-4 | Every module is safe to source (definitions only) | `ropero-code/bin/lib/01-env.sh:5-8`, `ropero-code/bin/lib/02-modelconf.sh:26` |
| R-NF-5 | `shellcheck -S warning` is clean on `bin/ropero bin/lib/*.sh bin/lib/engines/*.sh` | `ropero-code/README.md:195-197` |
| R-NF-6 | Model data is **data**: nothing in a TOML is executed | `ropero-code/models/_template.toml:9` |

---

## 4. Requirements

### 4.1 Architecture and modules

**R-ARQ-1** · The system consists of a **thin dispatcher** (`bin/ropero`,
about 200 lines) and lazily loaded modules. The dispatcher contains no
business logic: it resolves the root, sources `00-utils.sh` and `01-env.sh`,
then delegates.

> “This file ONLY routes” — `ropero-code/bin/ropero:14-19`

**R-ARQ-2** · The routing table is **complete from day one** and is not
changed: each later phase only adds `cmd-*.sh` and its
`rp_cmd_<command>` function.

> “NOBODY touches this dispatcher again” — `ropero-code/bin/ropero:25-28`

**R-ARQ-3** · An unported command responds `not yet ported (phase N)` and
exits with **2**; a command whose module does not define its function exits
with **1**. Evidence: `ropero-code/bin/ropero:252-263`.

**R-ARQ-4 (v2, working tree)** · Catalog and configuration resolution does
**not** use a `cwd`/`exe` directory chain as the repository's
self-discovery root by default. In normal use, the catalog root is derived
from external configuration: `<config-dir>/models/<engine>/`, with
manifests in `<config-dir>/engines/`. The `ROPERO_MODELS_DIR` and
`ROPERO_ROOT` overrides are preserved; when they point to the `models/`
directory of a catalog/repository root, manifest lookup retains the sibling
`<root>/engines/` semantics. Repository TOMLs are for development and tests
and are **not auto-discovered** in normal use. See
[ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md).
(Historical v1 evidence: `ropero-code/bin/ropero:66-99`.)

**R-ARQ-5** · Modules are: `00-utils` (utilities and presentation), `01-env`
(environment), `02-modelconf` (catalog wrapper), `03-engines` (engine
loader), `04-launch` (runtime), `05-download` (downloads), `06-styles` (live
output color), and one `cmd-<command>.sh` per command.

**R-ARQ-6 (v2, working tree)** · The catalog and engine manifests are TOML
data parsed and validated by the v2 Rust domain/runtime. In normal use, the
operational source of truth is the user's external catalog
(`<config>/models/<engine>/` and `<config>/engines/`), subject to environment
overrides. (Historical v1 evidence: Python parser in
`ropero-code/bin/python/modelconf.py`.)

**R-ARQ-7** · The parser is **self-contained**: it includes its own TOML
reader (`parse_toml`, `ropero-code/bin/python/modelconf.py:478`) and only
uses `tomllib` when available (`_tomllib()`, line 519). This allows it to run
with the system Python on macOS.

**R-ARQ-8** · The **Python interpreter is tested, not assumed**: candidates
are tried by running `python -c 'import sys'`; a broken asdf shim passes
`command -v` and then fails. `$ROPERO_PYTHON` takes precedence when set.
Evidence: `ropero-code/bin/lib/02-modelconf.sh:60-90`.

**R-ARQ-9** · Runtime state lives in `$STATE_DIR/ports/<port>`, a line-based
`KEY=value` file with `MODEL=`, `ENGINE=`, `PID=`, `PORT=`, `LEVEL=`, `TS=`,
`LOG=`, and `STAMP=`. Writes are **atomic** (tmp + `mv`) and **preserve**
keys not supplied in the call. Evidence:
`ropero-code/bin/lib/04-launch.sh:249-302`.

**R-ARQ-10** · The model directory has two levels: `models/<engine>/`
(active) and `models/<engine>/fired/` (retired). **The distinction is the
folder, not the schema**: retired models use exactly the same format.
Evidence: `ropero-code/models/_template.toml:5-7`.

**R-ARQ-11** · Tool state can be reconstructed from disk: managed ports are
derived from `$PORT`, `$CPU_PORT`, `[model].port` in active TOMLs, and the
files under `$STATE_DIR/ports/`. Evidence:
`ropero-code/bin/lib/04-launch.sh:304-334`.

---

### 4.2 The data model (TOML)

> The source of truth for the schema is the **validator**:
> `python3 bin/python/modelconf.py validate --root models`
> — `ropero-code/models/_template.toml:10-11`

#### 4.2.1 Tables and rules

**R-DATA-1** · `[model]` is required. `name` (kebab-case, globally unique
across active and retired models), `engine` (one of the 8), and `context`
(integer) are required; `alias`, `port`, and `path` are optional; `format`
may be `gguf` (default), `safetensors`, `q4nx`, or `strata`.

- `[model].path` is **required** with `format = safetensors` and **forbidden**
  with `format = gguf`. Evidence:
  `ropero-code/bin/python/modelconf.py:1474-1477`.
- Legacy id `mlx_lm` is accepted and canonicalized to `mlx-serve` with a
  warning. Evidence: `ropero-code/models/_template.toml:22-24`.

**R-DATA-2** · `[files]` is required. Values are **file names**, never
absolute paths (the validator reports an explicit error for those). `gguf`
is required with `format = gguf`. Keys: `gguf`, `mtp`, `mmproj`,
`chat_template`. Evidence:
`ropero-code/bin/python/modelconf.py:1479-1494`.

**R-DATA-3** · `[download]` is required unless the model is built with
`[build]`. `repo` is required; `file` is required **only** with
`format = gguf` (with safetensors the entire checkpoint is downloaded with
`hf download` and the name is meaningless; if present, it warns but does not
fail). `name`, `size`, `excludes`, and `manual` are optional. `manual = true`
removes the model from `ropero install models` offers (large checkpoints the
installer must not download unexpectedly). Evidence:
`ropero-code/bin/python/modelconf.py:1496-1529`,
`ropero-code/models/_template.toml:47-58`.

**R-DATA-4** · `[download.aux.<type>]` describes auxiliary files; type ∈
`mtp`, `mmproj`, `chat_template`, `imatrix`, `tokenizer`. `file` is required;
`repo` may point to another repository; `use` ∈ `run` | `build` (default:
`run` for runtime files and `build` for build-only files). Evidence:
`ropero-code/models/_template.toml:60-68`.

**R-DATA-5** · `[build]` declares **how to build** the GGUF: `repo`, `gguf`,
`gguf_file`, `quant`, `imatrix`, `imatrix_repo`, `imatrix_file`, `out`,
`out_principal`, `from_subdir`, `include`, `intermedio` (`f16`|`bf16`),
`keep_source`, `keep_intermedio`, `no_mtp`, `force`, `size`. An **unknown**
key produces a warning (not an error). Evidence:
`ropero-code/models/_template.toml:71-93`.

**R-DATA-6** · If a TOML has both `[download]` **and** `[build]`,
`ropero install models <model>` requires `--download` or `--build` to choose
a path; the interactive menu asks. Evidence:
`ropero-code/README.md:74-75`, `ropero-code/models/_template.toml:72-73`.

**R-DATA-7** · `[levels]`, `[flags]`, and `[options]` declare **what the model
accepts on the command line**. An **undeclared** level, flag, or option is a
usage error (exit 2): it is not silently ignored. Evidence:
`ropero-code/models/_template.toml:96-98`,
`ropero-code/bin/lib/cmd-runtime.sh:143`,
`ropero-code/bin/lib/cmd-catalog.sh:272`.

**R-DATA-8** · `[levels].default` must be in `[levels].list`;
`[options.default].<k>` must be in `[options].list`; `[aliases].<x>` must
point to a level in `[levels].list`. Evidence:
`ropero-code/bin/python/modelconf.py:1531-1555`.

**R-DATA-9** · `[limits].min_ram_gb` is a number > 0. It is **advisory**:
at launch, if RAM is insufficient, Ropero **warns and continues** (macOS
pages instead of failing; better to let the user try). Evidence:
`ropero-code/bin/python/modelconf.py:1557-1565`,
`ropero-code/bin/lib/04-launch.sh:598-617`.

**R-DATA-10** · `[args]` is **required for `engine = llama-server`** and must
contain `base` as a list. For other engines, an empty `[args].base` is a
**warning**, not an error (a model with its own server may need no flags).
Evidence: `ropero-code/bin/python/modelconf.py:1567-1582`.

**R-DATA-11** · A `${file}` used in `[args]` but not declared in `[files]` is
an error. Evidence: `ropero-code/bin/python/modelconf.py:1625-1644`.

**R-DATA-12** · The TOML's folder must be a known `engine_dir` (`llama`,
`llama-official`, `mlx`, `airllm`, `flm`, `tensorrt`, `vllm`, `strata`).
Evidence: `ropero-code/bin/python/modelconf.py:1646-1649`.

#### 4.2.2 Assembling argv

**R-DATA-13** · A model is **not executed**: its `[args]` are a list of
*tokens*, and the final argv is assembled in this fixed order:

```
[args].base → [args.level.<level>] → [args.flag.<f>] (in requested order) → [args.option.<o>]
```

Evidence: `ropero-code/models/_template.toml:144-146`.

**R-DATA-14** · One list item = **one** argument. An argument containing
spaces is a TOML usage error, not an argument with quotes.

**R-DATA-15** · `[args.level.<n>]` supports three equivalent ways to express
a level: `base` (literal args), `reasoning_effort` (shortcut translated to
`--chat-template-kwargs <json>`), and `chat_template_kwargs` (the complete
JSON). Evidence: `ropero-code/models/_template.toml:118-122`.

**R-DATA-16** · `[args.flag.<f>]` may declare `requires = [<keys from
[files]>]`: if a file is not on disk, using the flag is an error. And
`[args.option.<o>]` supports `replace = "--flag"`, which **replaces** the
base pair instead of appending another one (to avoid duplicate `--temp`).
Evidence: `ropero-code/models/_template.toml:124-129`,
`ropero-code/bin/python/modelconf.py:1608-1623`.

> **Pending fix · argv order with `replace` (R-DATA-16).** v1 removes the
> base pair and **appends the new one at the end**, although the correct
> alternative is to **replace it in place**, preserving argv order. v2 freezes
> the latter behavior because R-DATA-16 specifies it and it prevents the
> replacement value from depending on its position. This behavior change is
> documented as a `fix` and **pending its own PR**; it is not part of the v1
> extraction.

> **Pending fix · width of the replaced pair (R-DATA-16).** If the token to
> replace is the last base item, v2 removes only one token, while v1 removes
> two or one according to `_drop_flag_pair`. The difference is covered by
> synthetic tests and awaits reconciliation in its own PR.

#### 4.2.3 Placeholders

**R-DATA-17** · Valid placeholders in any `base[]` are: `${gguf}`, `${mtp}`,
`${mmproj}`, `${chat_template}`, `${ctx}`, `${value}` (only in
`[args.option]`), `${gguf_dir}`, `${safetensors_dir}`, `${model_path}`,
`${env:VAR}`, and `${env:VAR:-def}`. Evidence:
`ropero-code/models/_template.toml:131-142`.

**R-DATA-18** · Values can be **overridden without changing TOML** with
environment variables: `ROPERO_<NAME>_CTX` (effective context),
`ROPERO_<NAME>_GGUF` (points `${gguf}` to another quantization from the same
repo), and `ROPERO_MODEL_OPT_<K>` (option default). The name is normalized
to uppercase with hyphens and dots changed to `_`. Evidence:
`ropero-code/models/_template.toml:148-151`,
`ropero-code/bin/lib/04-launch.sh:895`.

#### 4.2.4 Formats

**R-DATA-19** · The four artifact formats are: `gguf` (local file),
`safetensors` (checkpoint under `$SAFETENSORS_DIR`), `q4nx` (FLM store:
`[model].path` is a **TAG**), and `strata` (Strata catalog:
`[model].path` is `<family>/<package>` and Strata downloads the files, not
Ropero). Evidence: `ropero-code/models/_template.toml:29-32`.

**R-DATA-20** · A model with `format = gguf` and **neither** `[download]`
**nor** `[build]` is a **local-only** model: its GGUF must already be on
disk. Ropero warns, but does not fail (one TOML with commented-out
`[download]` must not take down an entire catalog). Evidence:
`ropero-code/bin/python/modelconf.py:1521-1529`.

**R-DATA-21** · 🔶 **`[model].name` MUST equal the TOML filename without its
extension**, and that value is the **alias** used to start and stop the
model.

> “That same name, the `name` field, must have the same value as the TOML
> filename. So if we have a model named nemotron, the TOML is called
> nemotron.toml and its `name` attribute is nemotron too.” — owner,
> 2026-11-12

Consequence: **there cannot be two TOMLs with the same name**, because the
alias is the model's operational identity (start, stop, status). A model with
`[model].name = "nemotron"` in `other.toml` is a conflict, not a second
model. ⚠ **The validator does NOT check this rule** (H-23): it validates that
`[model].name` exists, but not that it matches the file.

**R-DATA-22** · The vocabulary for what a model accepts from the CLI has
**three classes**, each with its own TOML section:

| Class | What it is | TOML section | Example |
|---|---|---|---|
| **level** (mode) | a named configuration profile: `think`, `instruct`, `low`, `medium`, `high` | `[levels]` + `[args.level.<n>]` | `ropero nemotron` ≡ `ropero nemotron --think` if `think` is the default |
| **flag** | a switch that adds arguments | `[flags]` + `[args.flag.<f>]` | `--flash` → `--flash-attn` |
| **option** | a `key=value` with a default | `[options]` + `[options.default]` + `[args.option.<o>]` | `--temp 0.7` → `--temp 0.7` |

The **default level** makes `ropero <model>` without arguments equivalent to
`ropero <model> --<default-level>`. An **undeclared** level, flag, or option
is a usage error (R-CLI-9). Level aliases (`[aliases]`) are shortcuts:
`ropero model --x` ≡ `ropero model --high`. Evidence:
`ropero-code/bin/python/modelconf.py:1487-1529`, `:1549-1554`;
`models/_template.toml` (commented example, lines 230-289).

**R-DATA-23** · A model may use **different files for download and build**,
and **each may come from a different repository**: chat template, MTP,
mmproj (vision), and imatrix are declared as auxiliary
`[download.aux.<type>]` entries, each with its own `file`, optional `repo`,
and `use` (`run` = loaded at launch; `build` = used only by gguffer). The
template supports the complete case: `[download.aux.imatrix]` with
`repo = "another-user/imatrices"`. Evidence:
`ropero-code/models/_template.toml:205-224`;
`ropero-code/bin/lib/cmd-models.sh:321-336`.

---

### 4.3 The engine contract

**R-ENG-1** · An engine is **one file** `bin/lib/engines/<id>.sh` defining
**eight required hooks**:

| hook | contract |
|---|---|
| `rp_eng_<id>_label` | human-readable label on stdout |
| `rp_eng_<id>_compatible` | 0 if OS/hardware supports it (reason on stderr) |
| `rp_eng_<id>_deps` | external commands separated by spaces |
| `rp_eng_<id>_install [--force]` | installs **its** dependencies |
| `rp_eng_<id>_resolve` | absolute binary path |
| `rp_eng_<id>_probe <port>` | `ok` \| `down` \| `missing` (0 only for `ok`) |
| `rp_eng_<id>_native_auth` | `yes` \| `no` |
| `rp_eng_<id>_launch <model> <port> <logfile> <argv...>` | **PID** on stdout |

Evidence: `ropero-code/bin/lib/03-engines.sh:6-25`.

**R-ENG-2** · The loader **checks all eight** and, if one is missing, names
it in the error: `engine 'x': missing hook rp_eng_x_label`. Evidence:
`ropero-code/bin/lib/03-engines.sh:93-105`.

**R-ENG-3** · Engine ids contain hyphens and are **not translated**: they
are legal Bash identifiers (`rp_eng_llama-server_label`). Legacy ids are
accepted and canonicalized (`llama`→`llama-server`, `mlx_lm`→`mlx-serve`,
`flm`→`fastflowllm`, `tensorrt`→`tensorrt-llm`,
`strata_engine`→`strata`). Evidence:
`ropero-code/bin/lib/03-engines.sh:26-33`, `54-64`.

**R-ENG-4** · **Adding an engine = adding one file** (and, if it has models,
`models/<dir>/`). Nothing else. Evidence: `ropero-code/README.md:181-182`.

**R-ENG-5** · A broken engine does **not take down installation**:
`_rp_engine_deps_ok` returns 1 with the missing-item list and lets its caller
decide. Evidence: `ropero-code/bin/lib/03-engines.sh:186-195`.

**R-ENG-6** · The **probe checks Ropero's port**, not the engine's internal
port: Ropero's endpoint and `/health` must respond. This matters for engines
behind `endpoint.py` (Strata, FLM, TRT). Evidence:
`ropero-code/bin/lib/engines/strata.sh:143-156`.

**R-ENG-7** · An engine without native auth (`native_auth = no`) starts
**without a key**; Ropero's proxy provides it in front. Evidence:
`ropero-code/bin/lib/engines/strata.sh:158-167`.

---

### 4.4 Command line

**R-CLI-1** · The public surface is:

```
ropero <model> [--port N] [--cpu] [--background] [--force]
ropero <command> [options]
```

Evidence: `ropero-code/bin/ropero:8-11`.

**R-CLI-2** · Commands are: `use`, `stop`, `status`, `list`, `template`,
`args`, `levels`, `path`, `models`, `install` (with `setup`/`engines`/
`link`/`opencode-plugin`), `engines`, `doctor`/`check`, `config`, `gguffer`,
`test`. `engines` and `models` alias `install engines` / `install models`.
Evidence: `ropero-code/bin/ropero:150-168`, routing table `160-179`.

**R-CLI-14** · 🔶 **The public `install` surface has two commands**:
`ropero install engines` and `ropero install models`. `setup`, `link`, and
`opencode-plugin` are **internal steps** in the installation process, not
commands the user should need to know: they exist in the dispatcher because
they are stages to execute, not because they are part of the public contract.

> “Install … should only have two, which are `ropero install engines` and
> `ropero install models`.” — owner, 2026-11-12

The current code exposes all five. The divergence is in § 6 (H-22).

**R-CLI-15** · Bare `ropero install` **runs the complete process**:
environment preflight → engines → models, in that order. Without a TTY it is
a **usage error** (exit 2): no menus that consume EOF and choose on their own.
Evidence: `ropero-code/bin/lib/cmd-install.sh:117-130` (bare: `setup` →
`engines` → `models`), `:463-467`.

**R-CLI-16** · `ropero install engines <id>...` and
`ropero install models <alias>...` accept an **explicit list** and are the
**unattended mode**: without a TTY there is no menu; a list is required. This
allows an engine/model set to be installed automatically. Evidence:
`ropero-code/bin/lib/cmd-install.sh:91-96`, `:464-467`;
`ropero-code/bin/lib/cmd-models.sh:641-645`.

**R-CLI-17** · 🔶 **`ropero stats [--watch]`** is a **system status** command:
CPU, RAM, and GPU, with automatic refresh under `--watch`. It does not exist
in the repository (H-23); the closest current commands are `ropero doctor`
(engine, port, and model state) and `bin/diagnostico.sh` (a collector that
combines five sources in one file). The requested live-metrics command is
**distinct** from `doctor`, which remains engine status.

**R-CLI-3** · **Exit codes**:
`0` success · `1` execution error · `2` usage error (including “command not
yet ported”) · `130` Ctrl-C in foreground. Evidence:
`ropero-code/bin/lib/00-utils.sh:170-182`, `ropero-code/bin/ropero:252-263`,
`ropero-code/bin/lib/04-launch.sh:1159-1161`.

**R-CLI-4** · A usage error **prints the error and a help pointer**
(`ropero <cmd> --help → usage for <cmd>`), not a twenty-line dump. Evidence:
`ropero-code/bin/lib/00-utils.sh:170-182`,
`ropero-code/bin/ropero:236-241`.

**R-CLI-5** · General help is **inline** in the dispatcher: it must work
when only `00-utils.sh`/`01-env.sh` exist, even without `alaja` and ANSI.
Evidence: `ropero-code/bin/ropero:20-22`, `199-213`.

**R-CLI-6** · An unknown command that **does not start with `-`** is
interpreted as a **direct model launch**, not an error. Evidence:
`ropero-code/bin/ropero:265-277`.

**R-CLI-7** · `ropero list` shows active models; `--all` includes retired
models. Retired models are **informational**: they are not installed or
started (`ropero <retired-model>` fails with “is retired”). Evidence:
`ropero-code/README.md:39`, `ropero-code/bin/lib/04-launch.sh:848-852`.

**R-CLI-8** · `ropero args <model>` prints final **argv**, one argument per
line. `ropero levels <model>` prints declared levels. `ropero path <model>`
prints resolved paths. All three reject arguments other than the model.
Evidence: `ropero-code/bin/lib/cmd-catalog.sh:221`, `299`, `346`.

**R-CLI-9** · Levels, flags, and options are validated **against TOML**: an
undeclared level, flag, or option is a usage error, with a message listing
valid values. Evidence: `ropero-code/bin/lib/cmd-runtime.sh:143`,
`ropero-code/bin/lib/cmd-catalog.sh:258`, `272`.

**R-CLI-10** · Argument grammar after the model name: the first argument
that is not `k=v` is the **level**; `k=v` items are **options**; the rest are
**flags** (with or without `--`). Evidence:
`ropero-code/bin/lib/02-modelconf.sh:269-307`.

**R-CLI-11** · `ropero status <model>` returns **0 if serving · 1 if not ·
2 if unknown**. Evidence: `ropero-code/README.md:45`.

**R-CLI-12** · `--cpu` exists only on Linux (on macOS it is a usage error)
and only works with `engine = llama-server` (it is `--n-gpu-layers 0`, one
of its flags). Evidence: `ropero-code/bin/lib/cmd-runtime.sh:85-92`,
`ropero-code/bin/lib/04-launch.sh:865-870`.

**R-CLI-13** · Repeated `--port`: the last wins, with a warning.
`--port=N` also works. An invalid port (outside 1–65535) is an error.
Evidence: `ropero-code/bin/lib/cmd-runtime.sh:94-113`,
`ropero-code/bin/lib/04-launch.sh:142-145`.

---

### 4.5 Runtime lifecycle

#### 4.5.1 Preflight

**R-RUN-1** · Preflight happens **before touching the port**: a doomed
launch (missing artifact, invalid argv, engine not installed) **does not kill
what is already running**.

> “preflight BEFORE touching the port — do not kill a running process for a
> doomed launch” — `ropero-code/bin/lib/04-launch.sh:833-835`

Preflight order: (1) engine installed, (2) `--cpu` compatible with the
engine, (3) valid model argv, (4) artifact on disk, (5) chat template exists,
(6) `[limits]` as a warning. Evidence:
`ropero-code/bin/lib/04-launch.sh:857-972`.

**R-RUN-2** · The **chat template is checked on disk** before launch:
llama-server aborts if it cannot find it, with its wall of built-in templates
and the actual error buried above in the log.

> “stop here, with one line that says what is wrong and where”
> — `ropero-code/bin/lib/04-launch.sh:770-775`

**R-RUN-3** · The model's **context must reach the engine**: inject `-c
<context>` (or `--ctx-size`) before TOML args, so a TOML containing its own
`-c` takes precedence. Without this, llama-server uses the model's native
context, KV cache consumes VRAM, experts remain on CPU, and the model runs at
half speed.

> “nemotron: 60 t/s instead of 100–120” —
> `ropero-code/bin/lib/04-launch.sh:934-950`

**R-RUN-4** · For `engine = llama-server`, Ropero adds `--model
<gguf-path>` to argv: TOML declares flags, not the path; without this the
server starts in *router mode* and loads nothing. `--cpu` is added **at the
end** (overriding TOML `--n-gpu-layers`/`--threads`) as
`--n-gpu-layers 0 --threads <nproc>`. Evidence:
`ropero-code/bin/lib/04-launch.sh:928-933`, `957-963`.

**R-RUN-5** · `ROPERO_<NAME>_GGUF` override is resolved **during preflight**,
using the same file that will be substituted for `${gguf}`: otherwise
preflight could approve a different quantization from the one served.
Evidence: `ropero-code/bin/lib/04-launch.sh:888-900`.

#### 4.5.2 Readiness

**R-RUN-6** · Wait for the **model, not merely the port**:
`rp_wait_ready` polls the engine *probe* and, when it responds, requires
`/v1/models` to publish the requested model (by name, GGUF, or path). Success
also accepts **state** with `MODEL=<expected>` and a live PID (some servers
do not publish an id that maps to the catalog).

> “Ropero ate the 300s timeout and then killed a perfectly healthy server”
> — `ropero-code/bin/lib/04-launch.sh:725-736`

**R-RUN-7** · If the process **dies** before listening, failure is
**immediate**: do not wait N seconds. Evidence:
`ropero-code/bin/lib/04-launch.sh:704-709`.

**R-RUN-8** · Default timeout is `ROPERO_START_TIMEOUT=300 s`, polling every
2 s. For a model downloaded at launch (Strata: up to 72 GB), the documented
recipe is to increase it to `7200`. Evidence:
`ropero-code/bin/lib/01-env.sh:63`,
`ropero-code/bin/lib/04-launch.sh:698`,
`ropero-code/docs/strata.md:67-72`.

**R-RUN-9** · `/v1/models` is queried **with the API key**: llama-server
requires the key even to list models. Evidence:
`ropero-code/bin/lib/04-launch.sh:338-351`.

**R-RUN-10** · A failed launch **kills the process it started** and cleans
state; the error includes the **last 15 log lines** (where the cause is,
not in the message itself). Evidence:
`ropero-code/bin/lib/04-launch.sh:650-682`.

**R-RUN-11** · The **PID is captured through a temporary file**, never
command substitution: with `$( )` the process is created by the *subshell*,
the engine becomes orphaned, and `wait` replies “not a child of this shell”
with rc 127—which Ropero read as “the model exited with code 127” while the
engine remained alive.

> “With the PID in a temporary file, THIS shell creates the process”
> — `ropero-code/bin/lib/04-launch.sh:1061-1070`

#### 4.5.3 The stamp (STAMP)

**R-RUN-12** · The launch configuration *stamp* is the args' **`cksum`**
(`sha256sum` is not available on macOS). It detects TOML changes while the
model is running.

> “the live server keeps the OLD config and ‘already running’ would be a
> silent lie” — `ropero-code/bin/lib/04-launch.sh:753-757`

**R-RUN-13** · If the model is already running on the destination port:

- **identical stamp** → do nothing (“already running”), exit 0;
- **different stamp** → restart, with a warning;
- **no stamp in state** (written by another version) → **restart**, because
  it cannot be *proved* that the configuration is the same.

Evidence: `ropero-code/bin/lib/04-launch.sh:986-1010`.

#### 4.5.4 Foreground and Ctrl-C

**R-RUN-14** · In foreground, the engine **always** starts in background
with output to the log file; the terminal shows the **live colored log** from
the current launch's offset (so history is not repeated). Evidence:
`ropero-code/bin/lib/04-launch.sh:1040-1048`, `1113-1123`.

**R-RUN-15** · Foreground waiting is a **`kill -0` loop**, not `wait`: Bash
defers the INT trap until the awaited command ends, and the awaited command
is the engine, which ignores SIGINT (nohup + shell without job control →
`SigIgn 0x7`). The trap runs as soon as the signal arrives and kills the
engine with SIGTERM.

> “Neither terminal Ctrl-C nor a later `kill -INT` kills it: SIG_IGN cannot
> be ‘unignored’ with kill” — `ropero-code/bin/lib/04-launch.sh:1134-1141`

**R-RUN-16** · Ctrl-C exits with **130**, recognized **by the process exit
code**, not by the trap (which may not run). The trap kills the engine, *tail*,
and colorizer (otherwise tail remains blocked in inotify without an owner).
Evidence: `ropero-code/bin/lib/04-launch.sh:1127-1128`, `1159-1161`,
`ropero-code/bin/lib/06-styles.sh:12-20`.

**R-RUN-17** · Launch is **locked by port** for its duration: a `.lock`
directory contains the PID (`mkdir` is atomic). The lock **expires** if its
Ropero process dies (`kill -9` must not block the port forever). Evidence:
`ropero-code/bin/lib/04-launch.sh:216-247`.

---

### 4.6 Ports, state, and stopping

**R-EST-1** · Destination port precedence:
`--port N` > pinned `[model].port` > CPU slot (`:9998`, Linux only) > engine
base slot. Evidence: `ropero-code/bin/lib/04-launch.sh:147-182`.

**R-EST-2** · Managed ports are the canonical list for `status`, `stop all`,
and `use` idempotency: `$PORT` + `$CPU_PORT` (Linux only) + `[model].port`
from **active** TOMLs + ports with state files (ad hoc `--port`). Evidence:
`ropero-code/bin/lib/04-launch.sh:304-334`.

**R-EST-3** · To ask “who is listening on this port?” use **four cascading
methods**: `lsof` → `ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd` (socket
inode). Without `lsof`, `lsof -iTCP` fails and always says “free.” Evidence:
`ropero-code/bin/lib/04-launch.sh:52-123`.

**R-EST-4** · Model identity on a port has a **four-level hierarchy**:
(1) `/v1/models` id mapped to catalog; (2) state file with `MODEL=` and live
PID; (3) raw id published by the server; (4) `"?"` if the port is up but
`/v1/models` does not respond. Evidence:
`ropero-code/bin/lib/04-launch.sh:391-426`.

**R-EST-5** · A published id matches a model if it is the name, GGUF (with
or without extension), or checkpoint path (basename or full path). Evidence:
`ropero-code/bin/lib/04-launch.sh:353-372`.

**R-EST-6** · `stop` kills **by port**, never with a global `pkill -f`
(which would kill parallel slots). Its target tree is: state PID + listening
PIDs + their direct children. Evidence:
`ropero-code/bin/lib/04-launch.sh:450-500`.

**R-EST-7** · **Kill the state PID even if the port is not listening yet**:
a server loading a model has not started listening and previously remained
alive, then began serving requests after loading. Evidence:
`ropero-code/bin/lib/04-launch.sh:458-465`.

**R-EST-8** · **Before killing, verify the PID has not been recycled**: the
OS reuses PIDs and stale state could point to a browser. Check that
`/proc/<pid>/cmdline` mentions the engine binary; if it does not, **do not
kill** and warn. Without `/proc` (not Linux), assume that it does. Evidence:
`ropero-code/bin/lib/04-launch.sh:430-448`, `474-484`.

**R-EST-9** · Shutdown is staged: `SIGTERM` to the whole tree → up to **10 s**
(checking both the port **and** PIDs) → safety-net `SIGKILL` → remove state.
Evidence: `ropero-code/bin/lib/04-launch.sh:508-536`.

**R-EST-10** · `stop` on a free port returns **1** (nothing to stop), not 0.
Evidence: `ropero-code/bin/lib/04-launch.sh:502-506`.

**R-EST-11** · The **API key** is shared across engines and resolved by
precedence: environment variable `LLAMA_API_KEY` > `$STATE_DIR/api_key` >
generate `sk-ropero-<hex>` (openssl, fallback `od` + `/dev/urandom`), written
with `chmod 600`. Evidence:
`ropero-code/bin/lib/04-launch.sh:184-214`.

**R-EST-12** · **State is the source of truth** and is written
**atomically** (tmp + `mv`), preserving keys not present in the call.
Evidence: `ropero-code/bin/lib/04-launch.sh:268-296`.

**R-EST-13** · `ropero use <model>` is **idempotent by (model, level)** and
prints `ROPERO_PORT=<n>` as the **last stdout line**. This is the stable
contract consumed by the OpenCode plugin. Evidence:
`ropero-code/bin/lib/cmd-runtime.sh:265-292`, `ropero-code/README.md:163-164`.

**R-EST-14** · `ropero stop` **without arguments** stops **all ports
managed** by Ropero; `ropero stop <alias>` stops one. “Managed” means
**started by Ropero**: an engine the user started manually in their own
terminal is **never touched**.

> “because perhaps you started a llama-server yourself; those aren't touched,
> only Ropero's” — owner, 2026-11-12

Evidence: `ropero-code/bin/lib/cmd-runtime.sh:314-324` (default target
`all`), `:325-360`; `ropero-code/bin/lib/04-launch.sh:304-334` (managed ports).

**R-EST-15** · 🔶 **Detecting unrelated engines is an open question.** The
ability to **report** (“these processes are using the GPU and are not mine”)
differs from the ability to **stop** them; today Ropero does neither: it does
not see, count, or mention them. On Linux, `nvidia-smi --query-compute-apps`
lists them; macOS is unresolved.

What **is** resolved is the safety rule: an unrelated engine is **never
killed** (P-3, R-EST-6, R-EST-8). Reporting and stopping are different, and
the former is much cheaper and more useful: “Ropero won't start because VRAM
is unavailable” is indistinguishable from “another llama-server is using the
GPU” until someone checks.

---

### 4.7 The single gateway: the OpenAI-compatible endpoint

**R-API-1** · `bin/server/endpoint.py` is a FastAPI/uvicorn server exposing
an OpenAI-compatible API **with its own authentication**, acting as a reverse
proxy (or generating in-process) for engines without an authenticated server.
Reasons: `airllm` is a pure library (no `serve`); `trtllm-serve` has no
`--api_key` (the OpenAI SDK's `api_key` is a *client* field, not a server
check); `flm serve` does not check the key. With `HOST=0.0.0.0`, these three
would be **exposed to the LAN without auth**. Evidence:
`ropero-code/bin/server/endpoint.py:13-20`.

**R-API-2** · There are exactly five routes:

| Route | Method | Auth |
|---|---|---|
| `/health` | GET | **no** (deliberate) |
| `/v1/models` | GET | yes |
| `/ropero/engine` | GET | yes |
| `/v1/chat/completions` | POST | yes |
| `/v1/completions` | POST | yes |

`/v1/chat/completions` and `/v1/completions` share one function; route
selects which applies. There is no `/docs` or `/openapi.json`. Evidence:
`ropero-code/bin/server/endpoint.py:1326-1429`.

**R-API-3** · `/health` **always returns 200**, even if the model is not
loaded or the backend is dead: actual state is in `backend.ready` and
`backend.error`. This is a liveness check, not status information.

> “Ropero opens it before the model has finished loading”
> — `ropero-code/bin/server/endpoint.py:1352-1354`

**R-API-4** · `/v1/models` returns **exactly one model**, with `id` equal to
Ropero's name (not the backend id). This is what makes
`ropero stop <model>` and the status `MODEL` column work. Evidence:
`ropero-code/bin/server/endpoint.py:1362-1378`,
`ropero-code/bin/server/README.md:17-18`.

**R-API-5** · The request body's `model` is **always** rewritten to the id
known by the backend: `model_id` for `tensorrt` and `strata`, and the **TAG**
for `flm` (FLM validates it; rewriting it to Ropero's id would break the
request). Evidence: `ropero-code/bin/server/endpoint.py:886-903`, `1532-1540`.

**R-API-6** · `/v1/completions` is **translated to
`/v1/chat/completions`** upstream (FLM does not implement the former), so
the four backends have the same semantics. Evidence:
`ropero-code/bin/server/endpoint.py:1408-1423`.

**R-API-7** · Errors use the **OpenAI format**:
`{"error":{"message":…, "type":…, "code":…}}`, with `type` ∈
`invalid_request_error` | `server_error` | `upstream_unavailable` and `code` ∈
`invalid_api_key` | `invalid_json` | `invalid_messages` | `model_loading` |
`backend_down` | `backend_error`. Evidence:
`ropero-code/bin/server/endpoint.py:198-204`, table in § 8.1 of the analysis
note.

**R-API-8** · A body that is not a JSON object, or invalid JSON, is **400
`invalid_json`**—never 500. Malformed `messages` is **400
`invalid_messages`**. Evidence:
`ropero-code/bin/server/endpoint.py:228-243`, `1419-1422`.

**R-API-9** · A message's `content` may be a **list of VLM parts**:
`{"type":"text"}` passes through its text and `{"type":"image_url"}` is
replaced with the literal `"[image]"`. Reason: a text endpoint that calls
`str()` puts `{"type": "text"...}` into the prompt. Evidence:
`ropero-code/bin/server/endpoint.py:158-175`.

**R-API-10** · `airllm` **does not implement tool calling**; the endpoint
returns an explicit 400 rather than pretending to support it (the client
would hang). Evidence: `ropero-code/bin/server/endpoint.py:556-563`.

**R-API-11** · Authentication accepts **two headers**:
`Authorization: Bearer <key>` and `X-Api-Key: <key>`. Comparison is
**constant-time** (`hmac.compare_digest`) over UTF-8 bytes. The `Bearer`
prefix is **case-sensitive** (lowercase `bearer` → 401); the value is
trimmed with `strip()`. Evidence:
`ropero-code/bin/server/endpoint.py:207-225`,
`ropero-code/test/test-endpoint-auth.sh:66-70`.

**R-API-12** · **Without a configured key the endpoint is open**, and says
so in its log. This is the system's only fail-open case, and it is explicit.
Evidence: `ropero-code/bin/server/endpoint.py:213-214`, `1542-1544`.

**R-API-13** · **The key is never forwarded to the backend**:
`authorization` is in `HOP_BY_HOP` and is removed from upstream headers.
The backend does not validate it (hence the proxy); forwarding credentials to
a component that does not need them only expands the attack surface. Evidence:
`ropero-code/bin/server/endpoint.py:127-129`, `141`, `864-884`.

**R-API-14** · The key **never appears in a log**, and endpoint logging is
**always to stderr** (Ropero consumes stdout). Evidence:
`ropero-code/bin/server/endpoint.py:145-152`.

**R-API-15** · The backend subprocess always listens on **`0.0.0.0.1`**:
the proxy is the only process exposed to the network. Evidence:
`ropero-code/bin/server/endpoint.py:44-45`, `1226`, `1251`, `1313`.

**R-API-16** · The proxy **can run without a backend**: its external port
opens before the model is ready and returns `503 model_loading`, not a hang.
Evidence: `ropero-code/bin/server/endpoint.py:332-337`, `549-554`.

**R-API-17** · One uvicorn worker: with a proxied backend the model lives in
the subprocess; with AirLLM it lives in the proxy process and cannot be
shared among workers. Evidence:
`ropero-code/bin/server/endpoint.py:1576-1583`.

**R-API-18** · AirLLM **serializes generations** with a global lock: it moves
the entire model layer by layer. Two simultaneous generations do not mean
“faster”; they split the same throughput and double swap. Evidence:
`ropero-code/bin/server/endpoint.py:326-329`, `531`.

**R-API-19** · If a backend binary **rejects a flag** (`No such option`),
the proxy removes it from argv, **caches** it, and **relaunches**. The cache
is tied to binary identity (path + `mtime_ns` + size): if the binary changes,
the cache is ignored. Evidence:
`ropero-code/bin/server/endpoint.py:678-717`, `773-813`.

**R-API-20** · Engine and proxy logs **share one file**
(`$LOG_DIR/<model>.log`); child stderr is read through a pipe and rewritten,
retaining the **last 40 lines** to detect `No such option`. Evidence:
`ropero-code/bin/server/endpoint.py:742-769`,
`ropero-code/bin/lib/04-launch.sh:1035`.

**R-API-21** · A backend 4xx/5xx error is **forwarded unchanged** (same
status and bytes); connection errors become 502/503 with a clear reason,
**never a hang**. Evidence:
`ropero-code/bin/server/endpoint.py:922-936`, `996-1006`.

**R-API-22** · In SSE streaming, the proxy emits `data: [DONE]` in `finally`
even if upstream disconnects mid-stream (AirLLM has no server to do so).
Evidence: `ropero-code/bin/server/endpoint.py:628-632`.

**R-API-23** · 🔶 **Proxy use is decided per engine, not globally.** Each
engine knows whether it needs the proxy, and its `launch` hook assembles the
command—with or without the proxy. The engine does not need Ropero to “know”
anything else: configuration only needs the **model**, with its `engine`.

> “when a model is added and it specifies an engine that uses Ropero's
> endpoint, Ropero already knows at launch to start that endpoint. The only
> thing that needs to be configured in Ropero is the model” — owner,
> 2026-11-12

Consequence: **integrating a new engine is development** (write its file in
`bin/lib/engines/`); **using a new model is not** (write its TOML). Evidence:
`ropero-code/bin/lib/engines/airllm.sh:121`,
`ropero-code/bin/lib/engines/strata.sh:278`,
`ropero-code/bin/lib/engines/tensorrt-llm.sh:169` (all three launch
`endpoint.py`); `ropero-code/bin/lib/engines/mlx-serve.sh:5` and
`ropero-code/bin/lib/engines/fastflowlm.sh:292` (direct).

**R-API-24** · 🔶 **Which engines use the proxy today** (measured):

| Engine | Proxy? | Why |
|---|---|---|
| `airllm` | **yes** | pure library; no server |
| `tensorrt-llm` | **yes** | `trtllm-serve` does not validate the API key |
| `strata` | **yes** | `strata launch` returns; proxy is the live process and supplies the key |
| `fastflowlm` | **no** (the code) | `flm serve` is already OpenAI-compatible |
| `mlx-serve` | **no** | own server with `--api-key-env` |
| `llama-server` / `-official` | **no** | own server with native auth |
| `vllm` | **no** | own server |

⚠ **Open contradiction**: the header in
`ropero-code/bin/lib/engines/fastflowlm.sh:6-8` says “runs behind endpoint.py”
and `ropero-code/CHANGELOG.md:23-25` groups it with TRT and Strata, but the
actual launch (`ropero-code/bin/lib/engines/fastflowlm.sh:292`) is direct
`flm serve` and the test **requires this**
(`ropero-code/test/test-endpoint-auth.sh:109-122`). Code and test prevail;
the headers are wrong (H-24).

**R-API-25** · 🔶 **The endpoint is untested.** The owner explicitly says:
“I don't know whether this is only AirLLM or TensorRT because I haven't
tested them.” There is also a measured reason for doubt: **the auth test
passes while auth is broken** (R-TST-18). Therefore the contract in § 4.7
**cannot be considered satisfied** until that test propagates its exit code.

---

### 4.8 Routing

**R-ROUTE-1** · Ropero's actual routing is **by port and disk state**, not by
request rules. A client chooses the model (and therefore the port); the
runtime determines what serves that port.

**R-ROUTE-2** · `bin/router.sh` does **not route HTTP requests**: it is a
semantic model selector. It queries FastFlowLM's decision endpoint
(`D3_URL`, default `http://127.0.0.1:9991/v1/systemone`), asks for a model for
the task, and **prints one name to stdout**. If the classifier does not
respond, it **never fails**: it returns a fallback id and exits 0. Evidence:
`ropero-code/bin/router.sh:9-38`.

**R-ROUTE-3** · `router.sh`'s fallback is `minimax/m3.1-flash`, which **does
not correspond to any model in Ropero's catalog**. It is leftover behavior.
Evidence: `ropero-code/bin/router.sh:31-38` versus `models/`.

**R-ROUTE-4** · `router.sh` has **no consumer in the repository**, and its
header promises to query `MODELS.md`, a file that **does not exist** here.
Evidence: `ropero-code/bin/router.sh:6`, no `grep` matches.

**R-ROUTE-5** · The classifier request body is built by **unescaped
interpolation**: a description containing double quotes breaks JSON and the
request falls back without a message. Evidence:
`ropero-code/bin/router.sh:27-35`.

---

### 4.9 Installation and engine management

#### 4.9.1 Scope and layout

**R-INST-1 (updated in v2)** · `install engines` builds an engine when its manifest declares an `[install]` recipe, then creates the symlink. Without a declared recipe, an already-installed binary is required. Existing source trees are not deleted.

> “R1 — symlinks only: no copy, no download, no build, no entry farms, no repository deletion” — `ropero-code/install.sh:23-24`

**R-INST-2** · Installation leaves **exactly two symlinks**:

```
~/.local/bin/ropero      → <repo>/bin/ropero
~/.local/share/ropero    → <repo>/bin
```

The share **is** the repository's `bin`. Evidence: `ropero-code/install.sh:5-20`, `:367-392`; same contract in `ropero-code/bin/lib/cmd-install.sh:530-537`.

**R-INST-3** · The repository is the **source of truth**: `git pull` immediately updates installed code, without reinstalling. Evidence: `ropero-code/install.sh:25-28` (rule R3).

**R-INST-4** · **Installer never writes inside `<repo>/`**. Since share is a symlink to repo `bin`, writing to share **is** writing to repository. Engine binaries therefore live in `$BIN_DIR` (`~/.local/bin`), like `uv` or `brew`. Evidence: `ropero-code/install.sh:45-47` (R11), `ropero-code/bin/lib/cmd-install.sh:538-549`.

**R-INST-5** · **Offline operation**: there is no network mode, `--tarball`, or stdin bootstrap. `--tarball` and `--version` are **rejected with exit 2**, with an explanatory message. Evidence: `ropero-code/install.sh:35-36` (R6), `:556-557`.

**R-INST-6** · The user's **PATH is never modified**: if `~/.local/bin` is not in PATH, Ropero warns and prints the missing `export` command, but does not edit shell files. Evidence: `ropero-code/install.sh:408-415` (R13).

**R-INST-7 (v2, working tree)** · The v1 installer **only creates** `$BIN_DIR` and `dirname "$DIR"`; the runtime/configuration creates the rest. In v2, configuration bootstrap idempotently creates the configuration directory and default `config.toml` **without overwriting existing files**. In normal use, the external catalog is in `<config_dir>/models/<engine>/`, engine manifests are in `<config_dir>/engines/`, and `logs_dir` defaults to `<config_dir>/logs`.

**Configuration variables and precedence:** `environment variable > config.toml > default` for `models_dir` (`ROPERO_MODELS_DIR`), `gguf_dir` (`GGUF_DIR`), `safetensors_dir` (`SAFETENSORS_DIR`), `bin_dir` (`ROPERO_BIN_DIR`), and `logs_dir` (`LOG_DIR`). `<config_dir>` is `$XDG_CONFIG_HOME/ropero` or `~/.config/ropero`.

**Engine manifests (user):** they are not embedded as fallbacks. They are read from the `engines/` directory associated with the catalog root (for example `<config_dir>/engines/` or `<root>/engines/` when scanning starts from `models/` due to an override).

**Migration (legacy TOML files):** copy/move model TOML files to `~/.config/ropero/models/<engine>/` (or the configured `models_dir`) and engine manifests to `~/.config/ropero/engines/` (or the catalog root's `engines/`). Repository TOML files can be used in development/tests through `ROPERO_MODELS_DIR`/`ROPERO_ROOT`; there is no autodiscovery in normal use. See [ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md).

#### 4.9.2 Flags and exit codes

**R-INST-8** · `install.sh` flags are a **closed set**:

| Flag | Semantics |
|---|---|
| (none), `--install`, `--local` | install (`--local` is a silent alias) |
| `--check` | verify **without modifying anything**; exit 0/1 |
| `--dir <path>`, `--dir=<path>` | redirect the share directory |
| `--bin-dir <path>`, `--bin-dir=<path>` | redirect the entry point |
| `--help`, `-h` | usage, exit 0 |
| `--tarball`, `--version` | **exit 2** (symlink-only) |
| anything else | **exit 2** |

Precedence: *flag > environment variable > default* (`ROPERO_DIR`, `ROPERO_BIN_DIR`). Evidence: `ropero-code/install.sh:69-75`, `:535-562`.

**R-INST-9** · `--local` together with `--check` → **`--check` wins**: the check is evaluated before migration or linking. Evidence: `ropero-code/install.sh:563`, `:567-570`.

**R-INST-10** · The installer uses **only `pipefail`**: no `errexit` or `nounset`, so an implicit failure does not abort installation halfway without a message. Aborts are **explicit** (`|| die`). Evidence: `ropero-code/install.sh:61-66`.

**R-INST-11** · Output goes to **stderr** with markers `[✓] [!] [✗] [-]`, in Spanish, with final count “N ok · M warnings · K failures”; color is used only when stderr is a TTY and `NO_COLOR` is unset. Codes: 0 ok · 1 failure · 2 usage. Evidence: `ropero-code/install.sh:41-42`, `:86-108`.

**R-INST-12** · The installer **verifies its own result**: after linking, it runs `bash <repo>/bin/ropero --help`; a non-zero result is a failure (installation **exit 1**), not a warning. Evidence: `ropero-code/install.sh:423-434` (R9).

#### 4.9.3 Migrating the old layout and recovery

**R-INST-13** · `fase_migrar` decides based on `$DIR` state:

| Situation | Action |
|---|---|
| does not exist | do nothing (created when linking) |
| canonical symlink to `$ORIGIN/bin` | do nothing (idempotent) |
| our symlink with old target | recreate canonically |
| **dangling symlink** | if raw target contains `/ropero/` as a component → recreate; otherwise warn and **leave untouched** |
| unresolvable symlink (permissions) | warn, leave untouched |
| foreign symlink | warn, leave untouched, `SHARE_SKIP=1` |
| **real file** at `$DIR` | `[✗]` + **exit 1** |
| **recognized real directory** | recover → `mv` to `$DIR.old.$$` → `rm -rf` |
| **unknown real directory** | `[✗]` + **exit 1** (“I won't touch it”) |

Evidence: `ropero-code/install.sh:275-356`.

**R-INST-14** · **Recovery hooks run before deleting anything**: `_rescatar_engines` (for each engine binary symlink inside the old farm) and `_rescatar_venv` (the gguffer venv to `$HOME/.local/state/ropero/venv`). Evidence: `ropero-code/install.sh:191-258`.

**R-INST-15** · Engine recovery is **skipped if `command -v <name>` already resolves**: it must not shadow a working brew or system binary. This is exactly why `resolve` gives PATH priority. Evidence: `ropero-code/install.sh:196-214` (F2).

**R-INST-16** · Recovery **does not overwrite** anything: if the destination exists (file or symlink), or the link is dangling or non-executable, recovery is skipped. Evidence: `ropero-code/install.sh:215-223`.

**R-INST-17** · A recovered link points to the **actual binary** (symlinks resolved), so it remains valid if `$BIN_DIR` is emptied and repopulated. Evidence: `ropero-code/install.sh:219-228`.

**R-INST-18** · If a recovered venv collides with an existing one, it is moved aside as `venv.farm.$$`; nothing is overwritten. Evidence: `ropero-code/install.sh:236-258`.

#### 4.9.4 What the installer never touches

**R-INST-19** · Untouchable items: the repository; a real file at `$BIN_DIR/ropero`; a real file at `$BIN_DIR/ropero.d`; an unrecognized real directory at `$DIR`; a foreign symlink at `$DIR`; the network; the user's PATH; and other engines' environments. Evidence: `ropero-code/install.sh:369-373`, `:404-406`, `:352`, `:324-326`, `ropero-code/bin/lib/cmd-install.sh:601-604`.

**R-INST-20** · **There is no `--uninstall`**: the only uninstall procedure is to remove the two symlinks manually. Migration of the old layout does exist. Evidence: no “uninstall” matches in `install.sh`.

#### 4.9.5 `--check`

**R-INST-21** · `--check` is **read-only** and performs five checks: (1) the entry point must be a symlink resolving to an existing executable; (2) the share must be a symlink whose resolved target is **inside the installed entry point's repository**; (3) `bash <entry> --help` must exit 0; (4) legacy remnants (`ropero.d`, `$DIR.old.*`) produce warnings, not failures. With **guard F4**: if the resolved entry point points **outside** this repository, warn and **run nothing**. Evidence: `ropero-code/install.sh:445-530`, `:501-512`.

#### 4.9.6 Engines: three easily confused states

**R-INST-22** · An engine has **three distinct states**, which doctor reports separately:

| State | Definition |
|---|---|
| **installed** | its `install` completed successfully **and** `resolve` returns a usable path |
| **linked** | a symlink exists in `$BIN_DIR` → actual binary |
| **on PATH** | `command -v <name>` resolves |

Evidence: `ropero-code/bin/lib/cmd-install.sh:506-515`, `:593-613`; `ropero-code/bin/lib/cmd-doctor.sh:286-301`.

**R-INST-23** · An engine whose installer reports OK but **is not usable afterward** is a **failure, not a disguised success** (`rc=1`). Evidence: `ropero-code/bin/lib/cmd-install.sh:506-515`.

**R-INST-24** · `resolve` precedence is **PATH → `$BIN_DIR` → long paths**, for all eight engines. ⇒ *The engine used is the one on PATH*, not the newest or the one just compiled. Evidence: `ropero-code/bin/lib/engines/llama-server.sh:314-327`, `ropero-code/bin/lib/engines/llama-server-official.sh:228-241`, `ropero-code/bin/lib/engines/vllm.sh:73-84`, `ropero-code/bin/lib/engines/mlx-serve.sh:76-138`, `ropero-code/bin/lib/engines/tensorrt-llm.sh:103-116`, `ropero-code/bin/lib/engines/fastflowlm.sh:155-167`, `ropero-code/bin/lib/engines/strata.sh:124-139`; `ropero-code/bin/lib/engines/airllm.sh:70-75` (does not search for a binary: it searches for an **interpreter**).

**R-INST-25** · Link names are the `basename` of the resolved binary: `llama-server`, `llama-server-official`, `vllm`, `mlx-serve`, `trtllm-serve`, `flm`, `strata`. **`airllm` has no link**: it is a library served by `bin/server/endpoint.py` through its interpreter. Evidence: `ropero-code/bin/lib/cmd-install.sh:564-565`, `:599-600`.

**R-INST-26** · `ropero install engines` **requires a TTY**: without one it is a usage error (exit 2), because a menu receiving EOF would otherwise select something on its own. An unsupported engine is **skipped** with a warning; an already installed one is not reinstalled unless `--force`. Evidence: `ropero-code/bin/lib/cmd-install.sh:375-448`, `:464-467`, `:495-505`.

**R-INST-27** · A **broken engine does not abort** installation: `_rp_engine_deps_ok` returns 1 with the missing dependency list, **without calling `exit`**. Evidence: `ropero-code/bin/lib/03-engines.sh:171-184`.

---

### 4.10 The catalog and model downloads

#### 4.10.1 TOML is the data

**R-MOD-1** · The catalog **is the TOML files**: the root is `<repo>/models`; each `*.toml` not beginning with `_` is an **active** model; files under `models/<engine>/fired/` are **retired** (informational only: they are neither installed nor launched). Evidence: `ropero-code/bin/python/modelconf.py:630-658`, `ropero-code/README.md:60-62`.

**R-MOD-2** · `bin/python/modelconf.py` is the **single source of truth for the schema** (shell does not duplicate it). Evidence: `ropero-code/bin/lib/02-modelconf.sh:4-6`.

**R-MOD-3** · The shell consumes the catalog through **one dump** per process (idempotent), using separator `0x1F` and **31 fields** per record, plus a **format-version guard**: if the header is not `ROPERO_MC_DUMP_VERSION=1`, it errors rather than reading blindly. Evidence: `ropero-code/bin/lib/02-modelconf.sh:6-7`, `:33-34`, `:159-185`.

**R-MOD-4** · **A broken TOML file does not break the catalog**: a warning is sent to stderr and that file is skipped; the others still load. Previously the whole dump aborted and `ropero list` printed an empty list **with exit 0** (the worst possible failure: a silent falsehood). Evidence: `ropero-code/bin/python/modelconf.py:1176-1186`.

**R-MOD-5** · Duplicate names between active and `fired` files are **not an error**: both rows remain and **the first wins** (active files come first). Evidence: `ropero-code/bin/python/modelconf.py:137-140`, `ropero-code/bin/lib/02-modelconf.sh:220-229`.

**R-MOD-6** · The **format** declares **where the artifact lives**:

| `format` | Location |
|---|---|
| `gguf` | `$GGUF_DIR/<model>/<[files].gguf>` |
| `safetensors` | `$SAFETENSORS_DIR/<[model].path>` |
| `q4nx` (FLM) | `[model].path` is a **TAG**, not a path |
| `strata` | `[model].path` is `<family>/<package>` |

Evidence: `ropero-code/bin/lib/cmd-catalog.sh:389-410`.

#### 4.10.2 Aliases

**R-MOD-7** · The alias is declared in `[model].alias`; level aliases in `[aliases]` **must point to a declared level** (otherwise validation fails). Evidence: `models/_template.toml` (`[aliases]` section), `ropero-code/bin/python/modelconf.py:1549-1554`.

**R-MOD-8** · In memory, an alias is materialized as **an extra row** in the parallel arrays, after the real rows, copying their fields with `MODEL_ISALIAS=1` and `MODEL_REAL=<real>`. Evidence: `ropero-code/bin/lib/02-modelconf.sh:36-38`, `:193-214`.

**R-MOD-9** · An **alias collision is an error** that names the conflict; an alias equal to its own name is ignored. Evidence: `ropero-code/bin/lib/02-modelconf.sh:202-206`.

**R-MOD-10** · An alias **behaves like the name**: `rp_model_exists` accepts both, `rp_model_argv` translates alias → real, and `ropero path` always uses the **real name** for the directory. Evidence: `ropero-code/bin/lib/02-modelconf.sh:231-235`, `:284-288`; `ropero-code/bin/lib/cmd-catalog.sh:364-372`.

#### 4.10.3 The stdout contract

**R-MOD-11** · **stdout = data; stderr = all human-readable output**. In `ropero list`, stdout uses the raw format `  %-16s engine=%-13s format=%-12s ctx=%-7s …`; the **two-space prefix** is an explicit contract with the plugin. Evidence: `ropero-code/bin/lib/cmd-catalog.sh:11-15`, `:24-27`, `:91-92`; `ropero-code/bin/lib/00-utils.sh:5-8`.

**R-MOD-12** · `ropero levels` prints **no headers**, because anything else on stdout breaks shell completion; levels are validated with `[A-Za-z0-9._-]+` (shell-safe tokens). Evidence: `ropero-code/bin/lib/cmd-catalog.sh:284-287`.

**R-MOD-13** · `ropero args` prints **one argument per line** and **validates against TOML**: an undeclared level, flag, or option is exit 2. Evidence: `ropero-code/bin/lib/cmd-catalog.sh:221-282`.

#### 4.10.4 Downloads: `hf` only

**R-MOD-14** · The only downloader is **`hf` (Hugging Face CLI)**: there is no `curl` fallback, because `ROPERO_DOWNLOADER` was “a way for tests to leak through”. Evidence: `ropero-code/bin/lib/05-download.sh:9-13`.

**R-MOD-15** · GGUF downloads use their own **staging directory**: `mktemp -d "$dest_dir/.rp-dl.XXXXXX"`—inside the destination and on the same filesystem, so `mv` is atomic; it also prevents two downloads with the same filename from different repos colliding. Evidence: `ropero-code/bin/lib/05-download.sh:95-99`, `:158-162`.

**R-MOD-16** · The exact GGUF download sequence is: (1) if the final name exists and is non-empty → “already downloaded”, return 0; (2) if the final file is missing but the repo-named file exists (a download interrupted before rename) → `mv` and accept it; (3) check for **`hf` BEFORE asking**—promising a download and then admitting it cannot be done is the worst sequence; (4) `rp_confirm` with size; (5) `hf download` to staging; (6) if `hf` says OK but the file is missing, search by `basename` (hf sometimes normalizes the name); (7) rename by comparing absolute paths; (8) `rm -rf` staging. Evidence: `ropero-code/bin/lib/05-download.sh:123-199`.

**R-MOD-17** · Safetensors checkpoint downloads deliberately **do NOT use staging**: `hf` can resume with a fixed `--local-dir`, and staging would destroy that resume behavior (a partially downloaded 12 GB checkpoint must **continue**, not start over). Evidence: `ropero-code/bin/lib/05-download.sh:203-209`.

**R-MOD-18** · A download is verified by **actual contents**, not markers:
- GGUF: final file exists **and is non-empty** (`[[ -f && -s ]]`).
- Safetensors: weights in any of three formats (`*.safetensors`, `pytorch_model*.bin`, `*.npz`) **and**, if there is more than one shard, the index (`model.safetensors.index.json` or `pytorch_model.bin.index.json`)—“one loose shard does NOT count”—**and** `config.json`.

Evidence: `ropero-code/bin/lib/05-download.sh:20-22`, `:38-59`, `:126`, `:228-235`.

**R-MOD-19** · Even if `hf` exits 0, the result is **verified again**; if anything is missing → error “hf finished but <dir> is incomplete”, remove the marker, and **return 1**. A downloader's exit 0 does not mean a model is installed. Evidence: `ropero-code/bin/lib/05-download.sh:285-296`.

**R-MOD-20** · There is **no hash or checksum verification** anywhere. `[download].size` is **informational only**: used only in consent messages and tables. Evidence: `ropero-code/bin/lib/05-download.sh:91`, `:151`; `ropero-code/bin/lib/cmd-models.sh:478`, `:487`.

**R-MOD-21** · Download state is **reconstructed from disk**: GGUF → final file exists; safetensors → weights + `config.json` + `.download_complete` marker (touched **only** if weights really exist). A `.download_complete` without weights means “broken directory, not an installed model” and is downloaded again. Evidence: `ropero-code/bin/lib/05-download.sh:21-22`, `:250-256`, `:286-298`.

**R-MOD-22** · `hf` output goes to **stderr**, so an install with model selection does not “look like it is spewing garbage instead of a model”. Evidence: `ropero-code/bin/lib/05-download.sh:277-279`.

**R-MOD-23** · Auxiliary `[download.aux.*]` files are downloaded into the same directory using `use=run` filtering (those referenced by startup: `${mtp}`, `${mmproj}`, `${chat_template}`). Evidence: `ropero-code/bin/lib/cmd-models.sh:321-336`, `:515-517`.

#### 4.10.5 Reuse between models

**R-MOD-24** · If two TOMLs request the **same repo + same filename**, the second model is **linked** to the first model's directory instead of downloading again. Comparison is **repo + destination filename**, not filename alone (`mmproj-BF16.gguf` can exist in different repos with different contents). Evidence: `ropero-code/bin/lib/cmd-models.sh:338-435`, `:346-350`.

**R-MOD-25** · A manually created symlink is **left alone** (not touched); a **broken** one is removed; and if the file has the repo's name rather than the expected name, no useless directory is linked—instead, the required `mv` is explained. `--force` does **not** link anything (“I want the file HERE NOW”). Evidence: `ropero-code/bin/lib/cmd-models.sh:396-426`.

#### 4.10.6 Deciding to download

**R-MOD-26** · `ropero models` **requires a TTY** unless selection is explicit; explicitly requesting a `fired` model is a **usage error (exit 2)**; a model without `[download]` or `[build]` is exit 2 (“its artifact is obtained manually”); and a flag contradicting the TOML (`--build` for a model with only `[download]`) is exit 2. Evidence: `ropero-code/bin/lib/cmd-models.sh:641-662`, `:688-690`, `:730-737`.

**R-MOD-27** · **Selecting a model is consent to download it**: `RP_YES=1` is set for the process, so the size question is not repeated. Evidence: `ropero-code/bin/lib/cmd-models.sh:19-20`, `:500`.

**R-MOD-29** · A model can declare **both paths** (`[download]` and `[build]`); the user must then choose, and `--prio download|build` selects **without prompting**, enabling unattended installation of a model list. If the requested priority is unavailable for that model, it is **ignored**: not a usage error and does not abort. It is also ignored when the model has only one path and that path is requested, since it is already the default. An ignored `--prio` does **not** prevent other models in the list from being installed. This changes the v1 behavior, which treated it as a usage error. Evidence: `ropero-code/bin/lib/cmd-models.sh:696-723`, `:730-737`; full contract in [doc B §4.10.7](ropero-construccion.md#4107-el-contrato-de-prio).

**R-MOD-30** · In the **build** path, if the safetensors repo is already quantized, the quantization phase is skipped and GGUF is generated directly. This is the same rule used by gguffer when the requested quant is the factory quant (R-GGUF-9). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:439-483`.

**R-MOD-31** · `ropero gguffer` is the manual equivalent of `ropero install models --prio build`: it downloads a safetensors repo, creates the GGUF, and quantizes it. It works directly if the safetensors are already quantized, or with an imatrix for quantization. Use it when the full `install models` cycle does not fit (one model, one quant, or a model not yet in the catalog). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1-15`, `:1271-1327`.

**R-MOD-28** · Engines with their own stores (FLM, Strata) **make the decision themselves**: if an engine defines the optional `install_model` hook, it takes precedence and skips download/build. FLM downloads its build with `flm pull <tag>` (the tag can come from `[model].path` **and** `[decision]` in `[args]`) and creates symlinks in `~/models/q4nx/<model>`. **Strata downloads nothing**: Strata has no “download without launch” mode; its hook requests the *plan* (`strata launch --plan --output json`), shows it, and explicitly says “NOTHING was downloaded”. Evidence: `ropero-code/bin/lib/cmd-models.sh:437-455`, `ropero-code/bin/lib/engines/fastflowlm.sh:188-259`, `ropero-code/bin/lib/engines/strata.sh:188-237`.

---

### 4.11 The test suite

#### 4.11.1 What it is

**R-TST-1** · The suite is an **executable specification at two levels**: (1) **behavior**—138 explicit shell + Python assertions covering launch, probe, symlinks, auth, GGUF, quantization, FLM, and Strata; (2) **evaluation**—50 opencode sessions (10 models × 5 prompts) defining quality, with an external judge reading the `.md` files manually. Evidence: call-site count in `test/test-*.sh`; per-file breakdown in §5.2.

**R-TST-2** · Wrapper exit codes distinguish only 0 (all five suites pass), 1 (a suite failed), and 2 (cannot even start). The actual evaluation output is not an exit code: it is a **0–100 score report per block** with an A/B/C verdict. Evidence: `ropero-code/test/lib/test-common.sh:499-565`, `docs/juez.md`.

**R-TST-3** · **No test automatically evaluates model response content**: there is no assertion about response quality, only about the artifact (non-empty JSON). Quality regression is manual. Evidence: `ropero-code/test/lib/test-common.sh:401-437`.

#### 4.11.2 Shared runner

**R-TST-4** · The runner resolves paths **from the script's own location**, not from `cwd` or `ROPERO_ROOT`, so tests can run from anywhere. Evidence: `ropero-code/test/lib/test-common.sh:37-38`.

**R-TST-5** · The runner creates its own **hermetic provider** (`ropero-local`, OpenAI-compatible) at `127.0.0.1:<port>/v1`; it **does not read or touch the user's global opencode config**. It isolates `HOME` via `XDG_CONFIG_HOME/DATA/STATE/CACHE` under the suite directory. Evidence: `ropero-code/test/lib/test-common.sh:20-27`, `:147-183`, `:288-297`.

**R-TST-6** · The API key is resolved **after** model startup, in this order: `LLAMA_API_KEY` > `$STATE_DIR/api_key` > `OPENAI_API_KEY` > `sk-local-dev-key`. Order matters: resolving earlier against a freshly started engine would return 401. The key is **never** printed or written into the report; the config file containing it is `chmod 600`. Evidence: `ropero-code/test/lib/test-common.sh:85-100`, `:83`, `:181`, `:459`.

**R-TST-7** · Test model context and `output` derive from TOML `context`: `output = context − 1024`, clamped to `[4096, 65536]`, with default context 32768. Evidence: `ropero-code/test/lib/test-common.sh:114-128` (measured: `groot` → `context 65536 / output 64512`).

**R-TST-8** · Before launching opencode, the runner probes the engine with `GET /v1/models` + `Authorization: Bearer <key>` and aborts clearly if there is no response; an **empty** response is a failed probe. Evidence: `ropero-code/test/lib/test-common.sh:189-203`, `:394-398`.

**R-TST-9** · The runner invokes exactly `opencode run <prompt> --agent build --model ropero-local/<model> --format json --standalone`, sending stdout to JSON and stderr to its own file, **never discarded**. Evidence: `ropero-code/test/lib/test-common.sh:401-403` (argv verified in execution).

**R-TST-10** · A suite failure **does not abort the others**: it is recorded and execution continues; aggregate exit is 1 only if any failed. `ropero stop all` tolerates failure (`|| true`), so an already-clean environment cannot abort the suite. Evidence: `ropero-code/test/lib/test-common.sh:541-565`, `:213`, `:216`.

**R-TST-11** · The runner produces `recopilacion-completa.md` containing the **contents** of every artifact (engine log, JSON, exported session, stderr), not just their paths. Evidence: `ropero-code/test/lib/test-common.sh:312-332`, `:445-492`.

**R-TST-12** · Refusal to start is **exit 2**, not 1: missing model, missing `jq`/`opencode`/`ropero`, inability to create output directory, or inability to prepare the isolated environment. Evidence: `ropero-code/test/lib/test-common.sh:501-528`, `:530-539`.

#### 4.11.3 Test doubles

**R-TST-13** · The `test/fakes/` doubles are how the contract is verified **without GPU, network, or touching user state**. Fake `pkill` kills nothing and records argv; fake `curl` returns 401 with rc=22 when the header is missing and rc=7 for any other URL; fake `ropero` emits `ROPERO_PORT=<n>` as its last line. Evidence: `test/fakes/{pkill,curl,ropero,opencode}`.

**R-TST-14** · Fake `llama-quantize` **enforces the engine's real restriction**: requantization fails with literal message `requantizing from type X is disabled`. Therefore gguffer must detect incompatible quantization *before* downloading weights. Evidence: `ropero-code/test/fakes/gguffer/llama-quantize:121-129`.

**R-TST-15** · Synthetic GGUF (`gguf_fake.py`) makes the fake quantizer's result verifiable by the **real `ggufcheck`**, not another double; this lends credibility to the `file_type` assertion. Evidence: `test/fakes/gguffer/gguf_fake.py`, `ropero-code/test/test-gguffer.sh:178`.

**R-TST-16** · The offline suite, with temporary `HOME` and fakes on PATH, runs **5/5 suites successfully in about 2 seconds, exit 0**, with no GPU, network, or process killing. *Measured in execution.* Evidence: `test/test-groot.sh` + `test/fakes/` on PATH.

**R-TST-17** · Process termination uses a **closed, explicit list**: `llama-server`, `llama-server-official`, `opencode`, `mcode`, `claude`. **Never `python`**—`pkill -9 python` could kill processes unrelated to the test. Evidence: `ropero-code/test/lib/test-common.sh:206-219`, `ropero-code/test/fakes/pkill:6-10`.

#### 4.11.4 What the suite does NOT guarantee: measured gaps

**R-TST-18** · 🔴 **The endpoint auth guard passes green while auth is broken.** Reproduced: in a copy where `authed()` always returned `True`, the Python block reported five failures, but the script exited **0**. Python has its own counters and `sys.exit`; bash captures it in `rc=$?` and **never checks it** (the only use of `$rc` is unreachable because an earlier `exit 0` occurs). Bash counters cover only wiring checks, and the printed total adds a hardcoded `+12`. The repo's most sensitive security requirement is therefore **not guaranteed by the suite**. Fix: `[[ "$rc" == "0" ]] || fallos=$((fallos+1))`. Evidence: `ropero-code/test/test-endpoint-auth.sh:31-106`, `:204-208`.

> This is the worst kind of test defect: a checker that reports green without checking. It is worse than having no checker because it consumes the trust it should provide.

**R-TST-19** · 🟠 **The gguffer test is not hermetic**: it depends on host `transformers` because a real pre-check runs **before** the fake converter. Measured: in a sandbox without `transformers`, **15 of 38 checks fail** and the script exits 1. The message blames the environment, not gguffer. A fake `python` (or `ROPERO_PYTHON` pointing to a stub) is missing. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1961-1980`, `ropero-code/test/test-gguffer.sh:16-17`.

**R-TST-20** · 🟠 **There is no CI runner or documented command** for the complete suite: 16 scripts must be run manually. The path given by README and CHANGELOG (`tests/suite/run.sh`) **does not exist** (moved to `test/` in `4b133bf`, docs were not updated). `ropero-code/README.md:176` correctly says `test/`, so docs contradict each other. There is no `.github/`, `Makefile`, or `justfile`. Evidence: `ropero-code/README.md:176`, `:194`, `ropero-code/CHANGELOG.md:205`.

**R-TST-21** · 🟠 **Fake logs are written but never asserted**. Five root fakes record argv in `$HOME/.fake-*.log`, and **no test reads them** (`FAKE_LOG`/`.fake-` grep has zero matches). Thus “never `pkill -9 python`” and “exact `opencode run` argv” are documented, not verified invariants. The scaffolding already exists; a `grep` of the log is enough. Evidence: `ropero-code/test/fakes/README.md:50-51`.

**R-TST-22** · 🟠 **`[levels]` resolution has no tests**. The catalog defines per-model levels and `rp_model_argv <model> [level]` resolves them, but no test verifies a requested level reaches argv, an undeclared level fails, or the default level is applied. This is half the catalog configuration surface. Evidence: `models/llama/groot.toml` (`[levels]`), `ropero-code/bin/lib/02-modelconf.sh:269-312`.

**R-TST-23** · 🟠 **21 code files are untouched by tests**, including `cmd-catalog.sh`, `cmd-doctor.sh`, `cmd-install.sh`, `cmd-runtime.sh`, `cmd-gguffer.sh`, `router.sh`, the six remaining engines (only `fastflowlm` and `strata` have tests), and all `bin/quant/`. Only **2 of 8 engines** are tested. Evidence: grep over `test/*.sh` gives zero matches.

**R-TST-24** · 🟡 **CHANGELOG describes a stale contract contradicted by the test**: it says FLM passes `--api-key` to the endpoint and “refuses to start without `LLAMA_API_KEY`”; the test says the **opposite** (direct `flm serve`, no `--api-key`, starts without `LLAMA_API_KEY`). It also gives the wrong count (17 vs. 19 checks) and omits the refactor. **The test is the source of truth; CHANGELOG is wrong.** Evidence: `ropero-code/CHANGELOG.md:98-102` vs `ropero-code/test/test-endpoint-auth.sh:116-122`, `:199-202`.

**R-TST-25** · 🟡 **Two scripts depend on the environment more than expected**: `test-flm-decision.sh` and `test-models-symlink.sh` abort with `LLAMA_API_KEY: unbound variable` if it is not exported (because of `set -u` + `: "${OPENAI_API_KEY:=$LLAMA_API_KEY}"`); the runner also requires `jq` on PATH. An environment failure is presented as a test failure. Evidence: `ropero-code/bin/lib/01-env.sh:60`, `ropero-code/test/lib/test-common.sh:522-528`.

**R-TST-26** · 🟡 **Bash 3.2 compatibility is a claim, not a test**: README and 11 script headers state it, but the suite never runs `bash 3.2` (sandbox is Linux with bash 5). Grep confirms prohibited constructs are absent, but actual macOS compatibility is **untested**. Evidence: `ropero-code/README.md:199-200`, `ropero-code/test/bench-motor.sh:411`.

**R-TST-27** · 🟡 **`shellcheck` is not part of the suite**, despite README documenting it as a development requirement. Evidence: `ropero-code/README.md:194`.

**R-TST-28** · 🟡 **Two scripts return non-standard codes**: `test-strata.sh` exits with the **number of failures** (3 failures → exit 3, not 1), and `test-endpoint-auth.sh` exits 0/1 without propagating the Python block's failure (see R-TST-18). Evidence: `ropero-code/test/test-strata.sh:218-223`, `ropero-code/test/test-endpoint-auth.sh:204-208`.

---

### 4.12 The opencode integration

**R-OC-1** · The plugin is `opencode-plugin/index.ts`, a TypeScript plugin (`@opencode/plugin`) with `id: "ropero-modelo"`. It does **two things** for the same reason: the requested model should be the one that answers, and GPU/time should not be spent on requests that are not yours. Evidence: `ropero-code/opencode-plugin/index.ts:4-40`, `:146-147`.

**R-OC-2** · The plugin is installed as a **copy**, not a symlink—unlike the rest of Ropero, for an explicit reason: opencode's watcher (inotify) does **not** follow symlinks, so changes would not reload. If destination differs, a **numbered backup** (`.bak`, `.bak.1`, `.bak.2`…) is saved before overwriting; if identical (`cmp -s`), nothing is touched. Evidence: `ropero-code/bin/lib/cmd-install.sh:688-691`, `:710-712`, `:722-732`.

**R-OC-3** · The installer **does not register the plugin**: at the end it checks whether opencode config declares `"ropero-modelo"`; if not, it warns and prints the snippet. Plugins in `~/.config/opencode/plugins/` load automatically. Evidence: `ropero-code/bin/lib/cmd-install.sh:741-753`, `ropero-code/opencode-plugin/README.md:14`.

**R-OC-4** · The plugin's **three options**:

| Option | Default | Effect |
|---|---|---|
| `provider` | `"marichocho"` | opencode provider serving Ropero models |
| `port` | — (no `--port`) | port where model should run; **overrides pinned `MODEL_PORT`** |
| `titleMaxTokens` | `64` | maximum tokens for title request; `0` disables |
| `titleReasoning` | `"low"` | reasoning effort for title request |
| `titleLocal` | `false` | title is generated locally and model is **not** called |

Evidence: `ropero-code/opencode-plugin/index.ts:42-57`, `:151`, `:164`, `:271-279`.

**R-OC-5** · The **three hooks** used and when:

| Hook | When | What it does |
|---|---|---|
| `context` | just before every agent-loop request | `ropero use <model>` (+ `--level` if variant matches a level), and removes `reasoningEffort` from body if a level was passed |
| `model.request` | every request (`kind` = `primary\|compaction\|title\|generate`) | also calls `ropero use` for title request |
| `title` | title request | limits `maxTokens` and `reasoningEffort`, or generates title locally |

`hook("prompt")` **does not work**: it has no `model`. Evidence: `ropero-code/opencode-plugin/index.ts:215-281`, `ropero-code/opencode-plugin/README.md:96-103`.

**R-OC-6** · The plugin **filters**: it queries cached `ropero list` and does nothing if `modelID` is not a Ropero model—“the server is already serving whatever it is; adding Ropero would only break the request”. Evidence: `ropero-code/opencode-plugin/index.ts:76-101`, `:179-191`.

**R-OC-7** · If `ropero use` fails, the hook **throws** and the opencode request **fails**. It does not silently degrade. Evidence: `ropero-code/opencode-plugin/index.ts:167-172`, `ropero-code/opencode-plugin/README.md:60-62`.

**R-OC-8** · The opencode `variant` is passed as `--level` **only if it exactly matches** a level declared by the model (`ropero levels <model>`). Evidence: `ropero-code/opencode-plugin/index.ts:230-232`.

**R-OC-9** · Deduplication uses `served` + `inFlight` to avoid two `ropero use` calls for consecutive requests; the key includes the **level**, because the same model with another level has different args and restarts the server. Evidence: `ropero-code/opencode-plugin/index.ts:153-160`, `:192-210`.

**R-OC-10** · The stable CLI contract has three parts: `list` (two spaces + name), `levels` (one per line), and `use` (last line `ROPERO_PORT=<n>`). Evidence: `ropero-code/opencode-plugin/index.ts:173`, `ropero-code/bin/lib/cmd-runtime.sh:273`, `:291`; `ropero-code/README.md:163-164`.

**R-OC-11** · The plugin's default `provider` is **`"marichocho"`**, a provider that does **not exist** in the owner's current opencode configuration. It is a live code residue. Evidence: `ropero-code/opencode-plugin/index.ts:151`.

---

### 4.13 Strata engine and `groot-strata.sh` integration

This section answers: “What would need to be built for `groot-strata.sh` to become part of Ropero?” First, the current state.

#### 4.13.1 What Strata is and why Ropero treats it separately

**R-STRATA-1** · Strata is **not a model server**: it is a *terminal* that downloads an engine, prepares artifacts, and leaves the model on disk. Therefore Ropero's engine has **three extra pieces** beyond the 8-hook contract: `install_model`, the `strata-run.sh` *bridge*, and *pack* resolution. Evidence: `ropero-code/test/test-strata.sh:5-16`, `:102-109`.

**R-STRATA-2** · A Strata model in the catalog is a **pointer** into Strata's catalog: `[model].path = "<family>/<package>"` (e.g. `gyro/Gyro-M`), **not a path**. `_rp_eng_strata_pack` says so explicitly and does **not** use `_rp_engine_model_path`, which would prepend `$SAFETENSORS_DIR`. Evidence: `ropero-code/bin/lib/engines/strata.sh:160-176`; `ropero-code/models/strata/gyro-m.toml:22-25`.

**R-STRATA-3** · Ropero does **not** download the files: Strata downloads them at launch. Strata has no “download without launching” mode; Ropero's `install_model` hook explicitly says “NOTHING was downloaded” and gives the command (`ROPERO_START_TIMEOUT=7200 ropero use <model>`). Evidence: `ropero-code/bin/lib/engines/strata.sh:188-237`; `docs/strata.md`.

**R-STRATA-4** · Strata installation is **official**: download the installer to a **temporary file** (never `curl | sh`) and run it with `env STRATA_BIN_DIR=$BIN_DIR sh $tmp`. Reason: `strata install` uses the same environment variable to **find** `setup.py` and decide **where to write**, so the two paths cannot be isolated through that command. Evidence: `ropero-code/bin/lib/engines/strata.sh:87-111`.

**R-STRATA-5** · `resolve` precedence is **PATH → `$BIN_DIR` → `~/.local/bin/strata` → `~/.local/share/strata/terminal/current/bin/strata`**—the *active launcher*, not the newest version on disk. Evidence: `ropero-code/bin/lib/engines/strata.sh:124-139`.

**R-STRATA-6** · The **probe** distinguishes three states: `missing` (Strata absent), `down` (Strata present, no service on port), and `ok`. It checks Ropero's **port** (the proxy), not Strata's internal port: the Ropero endpoint's `/health` must respond. Evidence: `ropero-code/bin/lib/engines/strata.sh:141-154`.

**R-STRATA-7** · Strata does **not provide auth** (`native_auth` → `no`): Ropero's proxy supplies the key. Strata starts with `--access local --no-api-key`, listening only on 127.0.0.1. Evidence: `ropero-code/bin/lib/engines/strata.sh:157-163`.

**R-STRATA-8** · Ropero's `launch` returns a PID, but the supervisor is another process: `strata launch --no-tui` **returns** when the server is ready. Hence the bridge. Evidence: `ropero-code/test/test-strata.sh:160-161`; `ropero-code/bin/server/strata-run.sh:14-30`.

**R-STRATA-9** · The `strata-run.sh` **bridge** (1) launches Strata and waits for it to finish—download may be 70 GB; (2) stays alive while the model serves; and (3) on SIGTERM/SIGINT stops only what it launched and exits. Evidence: `ropero-code/bin/server/strata-run.sh:14-30`, `:100-152`.

**R-STRATA-10** · To stop only its own launch, it is saved with `--save-preset <name>` and stopped with `strata launch --preset <name> --stop`: bare `strata launch --stop` stops **all** launches, and stopping the user's Strata would be rude. Evidence: `ropero-code/bin/server/strata-run.sh:36-48`, `:100-110`.

**R-STRATA-11** · Every **15 seconds**, the bridge checks that launch is still running (`strata launch --status --output json`). If Strata stopped it or the engine died, the wrapper exits with an error so Ropero **cleans state** instead of retaining a nonexistent “model”. Its `sleep` is 1 second, not 1 hour, because bash delays traps while a command sleeps (measured). Evidence: `ropero-code/bin/server/strata-run.sh:130-152`.

**R-STRATA-12** · Ropero launch passes `--backend strata`, `--backend-bin <binary>`, `--model <family/package>`, Ropero's **published ID** (`strata-qwen38-q2`), model **context**, and **internal port = external + 1000** (10998). Extra TOML flags travel as `--extra-arg` to the backend. Evidence: `ropero-code/test/test-strata.sh:169-177`.

**R-STRATA-13** · The `--set` **prefix** passes valued options to Strata: `ropero use strata-qwen36-q4 --vram-reserve 1344` becomes `--set vram_reserve_mib=1344`. The 1344 MiB default was **measured on this machine** (RTX 5080 Laptop): below 512 MiB free, NVIDIA's driver pages and the request never returns. Evidence: `ropero-code/docs/strata.md:132-137`; `ropero-code/models/strata/gyro-m.toml:56-60`.

#### 4.13.2 The gap: `strata launch` cannot serve these models

**R-STRATA-14** · 🔴 **Ropero's Strata engine launches with `strata launch`, which runs through `setup.py` again.** `setup.py` rewrites the run config with Flash-Next's expert profile (**48×512**), which the engine rejects for Groot (**48×256**): “read_expert_profile: … is 48x512 but this model is 48x256 — it is a profile for a different artifact”. Without a model-specific profile, engine verification kills startup: “verify: needs the profile-filled VRAM expert tier”. Evidence: `groot-strata.sh:15-18`, `:184-205`.

⇒ The `strata launch` path is **inherently insufficient** for any GGUF whose expert profile does not match the installed model. This is not hypothetical: it is **exactly the case for the two Groot Gyro-M models**.

**R-STRATA-15** · 🔴 **Each GGUF needs its own pack**, not the pack of the already installed model. The `gyro-m` pack (Flash-Next, 91 GB) contains **absolute tensor positions** within its GGUF; those positions are invalid for another GGUF (Groot, 54 GB). The confusing symptom is “this pack holds the tensor only in its GGUF form”, followed by “ggml_cuda_init: failed to initialize CUDA: no CUDA-capable device is detected”—which looks like a GPU problem but is caused by the pack. Evidence: `groot-strata.sh:260-271`.

**R-STRATA-16** · The model-specific **expert profile** is generated synthetically and neutrally (not calibrated with activations), using **round-robin ranking by layer**: layer 0 expert 0, layer 1 expert 0, … layer 47 expert 0, layer 0 expert 1, … This covers all 48 layers evenly; natural ordering would fill cache with early layers and leave later ones uncovered. Format: `"STRP"` + u32 version + u32 layers + u32 experts + u32 n1 + u32 n2, with n1 pairs (u16 layer, u16 expert) = ranking, and n2 u32 = inverse permutation. Evidence: `groot-strata.sh:184-205`, `:206-258`.

**R-STRATA-17** · The script does more than launch: its full cycle is `prepare` → `tune` → `launch`:
- `prepare`: directories + download + **verification** (`strata models import <gguf> --plan`) + **one profile per GGUF** (isolated `setup.py` per model, own pack, own profile, patched args).
- `tune`: **measures this PC** (`strata tune auto`) and saves its preset, which Strata applies at launch. `quick` mode (~20 minutes) or `thorough` (up to 2 hours).
- `launch`: `strata serve --profile <cfg> --port <n>`.

Evidence: `groot-strata.sh:5-11`, `:363-399`, `:402-413`, `:449-473`.

**R-STRATA-18** · Strata `setup.py` is called **directly** with `STRATA_CONTROL_INSTALL=1` and a **per-model private** `STRATA_INSTALL_DIR`, because `setup.py` uses `CONTROL` to direct **all** writes into `home()` (not shared `gyro-m`), while `strata install` uses the same variable both to **find** `setup.py` and decide where to write; it cannot isolate both paths. Evidence: `groot-strata.sh:160-182`.

**R-STRATA-19** · The **argument patch** rebuilds the list, preserving current tuning options and **replacing only** pack, profile, and reserve (including duplicates). It removes `--mmap-experts` (a valueless switch; the engine decides itself) and adds `--pack <own> --expert-profile <own> --vram-reserve-mib <reserve>`. It also rewrites `tokenizer` (the one from the own pack), `model_name`, and `log` (each profile writes **its own** log to avoid collisions). Evidence: `groot-strata.sh:301-354`.

**R-STRATA-20** · `tune` **requires a free GPU** and verifies tune output through JSON, not exit code: it requires `ok is True`, `outcome` ∈ {`improved`, `default_confirmed`}, and a present `preset.id`. A `strata tune` that exits 0 without measuring anything **is a failure**, not success. Evidence: `groot-strata.sh:363-399`.

**R-STRATA-21** · The free-GPU check is intentionally strict: it tolerates trivial use (≤500 MiB per process—the desktop may appear as a CUDA process using a few MiB), but **any significant process or uninterpretable data blocks the operation**, and **nothing is ever killed**. Evidence: `groot-strata.sh:100-125`.

**R-STRATA-22** · The script's `stop` **refuses to stop processes**: “this script does not track engine ownership; stop them with their operator”. This is correct for a script without ownership tracking, and **exactly what Ropero already handles better**: it has state per port and verifies PID identity before killing (R-EST-6, R-EST-8). Evidence: `groot-strata.sh:96-98`.

#### 4.13.3 Requirements imposed by the integration

**R-STRATA-23** · 🔴 **Strata's engine needs a second launch mode**: `strata serve --profile <run-config>` in addition to the current `strata launch --family … --model …`. The current mode **rewrites the run config** and is incompatible with GGUFs using their own profiles. Without this mode, Groot Gyro-M models **cannot be served by Ropero**.

**R-STRATA-24** · 🔴 **`[model].path` needs a second meaning for Strata**: it currently means `<family>/<package>` in Strata's catalog; for the script path it would need to mean **the prepared run-config path** (or an explicit discriminator, e.g. `[model].strata_mode = "profile"` + `[model].path = "<path-to-cfg>"`). `validate_model` and the engine must distinguish these cases unambiguously.

**R-STRATA-25** · 🔴 **Prepared artifacts need a place in Ropero's catalog**. A model is currently one artifact (`$GGUF_DIR/<model>/<gguf>`); the script path needs four per model:

| Artifact | Path (per script) | Description |
|---|---|---|
| GGUF | `~/models/gguf/<id>/Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` | weights |
| pack | `~/models/gguf/<id>/packs/gyro-gyro-m/` | absolute tensor positions **for this** GGUF |
| profile | `~/models/gguf/<id>/expert-profile.bin` | 48×256 expert ranking |
| run config | `~/.local/share/strata/install/strata-<id>.json` | patched args |

The current catalog has no place for derived artifacts. FLM's convention (`~/models/q4nx/<model>` with symlinks) suggests `~/models/<format>/<model>/` per model, containing derived artifacts.

**R-STRATA-26** · The GGUF's **public filename** has consequences: the script renames **both** models to `Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` because “setup.py only finds that name”. In Ropero, `[files].gguf` can differ per model, and directories (`$GGUF_DIR/<model>/`) separate them. ⇒ Each model TOML must declare `[files].gguf = "Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf"`; Ropero should **not** enforce a per-model filename when the engine requires a per-artifact name. Side effect: R-MOD-24 compares repo + filename, so the two models **will not** link because their repos differ. Correct.

**R-STRATA-27** · `tune` must become a **first-class Ropero operation**: `ropero tune <model> [--mode quick|thorough]` measures the machine and saves the preset, with the script's strict JSON checks (`ok is True` + `outcome` + `preset.id`). Today tune exists only as a TOML option (`[options] list = ["tune", …]`) and is **commented out** in all Strata TOMLs.

**R-STRATA-28** · The `vram-reserve` option is **declared but dead**: it appears in `[options].list`, but `[args.option.vram-reserve]` is **commented out** in both Strata TOMLs. The commented mechanism (`--set vram_reserve_mib=${value}`) also differs from the script (`--vram-reserve-mib <n>` as a direct flag). ⇒ Choose **one** form and make it active and tested; without reserve, below 512 MiB free the NVIDIA driver pages and the request hangs. Evidence: `ropero-code/models/strata/gyro-m.toml:29`, `:56-60`; `ropero-code/docs/strata.md:132-137`; `groot-strata.sh:343-344`.

**R-STRATA-29** · VRAM reserve (1344 MiB) is a value **measured on one specific machine** (MSI Stealth AI+ A16, RTX 5080 Laptop). If adopted as Ropero's default, it must be an **overridable environment variable**, and the default must be documented as *measured on this machine*, not universal. The script already does this (`RESERVA="${RESERVA:-1344}"`).

**R-STRATA-30** · Preparing the pack requires `llama.cpp` with **`gguf-py` already installed** in Strata's tree: the script does **not** download it automatically (“Install it first, then run `preparar_pack <id>` again”) and refuses to continue if missing. Ropero must be able to **check and name** this prerequisite, not discover it halfway through preparation.

**R-STRATA-31** · Isolating `setup.py` requires the private installation directory **not to be a symlink** and **not to equal** shared `INSTALL_DIR`. These explicit checks prevent preparation of one model from overwriting the shared installation. Evidence: `groot-strata.sh:168-170`.

**R-STRATA-32** · The script's state is on its own disk, not Ropero's: `$RAIZ/<id>/tune.json`, `$RAIZ/<id>/tune.log`, `$RAIZ/<id>/prepare-pack.log`, and run config at `$INSTALL_DIR/strata-<id>.json`. For Ropero to stop the model and clean state, it needs to **reference these** in its per-port state file (`$STATE_DIR/ports/<port>`), whose `KEY=` line format can be extended.

**R-STRATA-33** · The script's free-GPU check is stricter than Ropero's, and both are reasonable in context: Ropero is **idempotent** (if model already runs on port, it does not touch it—R-RUN-13); the script has **no** ownership tracking and cannot touch anything (R-STRATA-21, 22). In Ropero the right check is: *“Is the PID recorded for this port the engine I started, and is it still alive?”*—R-EST-8. **Do not port the script's check as-is**: Ropero would refuse to start if the desktop uses 300 MiB of VRAM.

**R-STRATA-34** · The script is **not portable to macOS** for three specific reasons, and Ropero has an equivalent hard constraint: `readlink -f` (Ropero has its own `real_dir`/`real_path`/`_rp_link_real`, R-NF-4), GNU `ln -sT`, and GNU `stat -c%s`. It also uses `set -euo pipefail`, while Ropero's installer avoids `errexit` **deliberately** so implicit failures do not abort halfway without a message. Evidence: `groot-strata.sh:48`, `:140`, `:288`; `ropero-code/install.sh:61-66`; `bin/lib/00-utils.sh`.

**R-STRATA-35** · The script's status command uses `mktemp` + a temporary file for `strata models list --installed --output json`, for a concrete measured reason: in a pipeline, if the Python reader starts early and closes, `strata models list` gets SIGPIPE and **crashes with a core dump**. This is the same pattern as R-EST-3 and the owner's documented `grep -q` + `pipefail` bug. Evidence: `groot-strata.sh:422-428`.

#### 4.13.5 Target state

**R-STRATA-36** · 🔶 **Strata is a first-class engine.** The stated goal is for `ropero <model>` to serve it like the others, without the user needing to know Strata is special. It is **partially integrated** today: Ropero treats it as a pointer to Strata's catalog, while Groot/Gyro-M need preparation Ropero does not yet perform.

**R-STRATA-37** · 🔶 **Preparation is what is missing**, namely three things the script currently does manually: a **GGUF-specific pack** (R-STRATA-15), **GGUF-specific expert profile** (R-STRATA-16), and **machine-measured tune** (R-STRATA-20). Current state:
- **What Strata does best**: download and prepare the engine. Strata has no “download without launching” mode; it downloads at launch. Ropero cannot offer the **build** path (download weights, convert, quantize), because Strata does not provide it.
- **Manual equivalent of build** for Strata is **download the model manually and configure it manually**: exactly what `groot-strata.sh` does (prepare → tune → launch), filling the gap until Ropero takes it over.
- **Golden rule for the manual path**: Strata `setup.py` rewrites the run config with the installed model's expert profile; therefore prepare the profile **once** and launch with `strata serve --profile` instead of `strata launch` (R-STRATA-23).

**R-STRATA-38** · 🔶 **The script is the manual path, not the integrated path.** Its purpose in this specification is to define the contract Ropero must implement: per-GGUF profile, per-GGUF pack, VRAM reserve, verified tune, and launch through `--profile`. When Ropero takes ownership, the script is no longer needed and its logic belongs in the engine.

#### 4.13.4 Comparison table: script ↔ Ropero

| Capability | `groot-strata.sh` | Ropero today | Gap |
|---|---|---|---|
| Download GGUF | `hf download` into model directory | `hf` to staging + rename (§4.10.4) | 🟢 compatible (same `hf`, destination) |
| Verify GGUF | `strata models import --plan` | ✖ absent | 🟡 **new** |
| Per-model run config | `setup.py` with private `INSTALL_DIR` | ✖ one shared install | 🔴 **new** |
| Pack per GGUF | `setup.py --prepare-pack` | ✖ | 🔴 **new** |
| Expert profile | round-robin 48×256 | ✖ (engine uses factory profile) | 🔴 **new** |
| VRAM reserve | `--vram-reserve-mib 1344` in args | declared and **commented out** | 🟠 **reactivate** |
| Tune | `strata tune auto` + JSON verification | commented option only | 🔴 **new** |
| Launch | `strata serve --profile` | `strata launch --family/--model` | 🔴 **new mode** |
| Stop | refuses (no ownership) | stops by port with verified PID | 🟢 **Ropero is better** |
| State | script-owned disk files | `$STATE_DIR/ports/<n>` | 🟠 **bridge** |
| Idempotency | `prepare` idempotent by size | same | 🟢 |
| Free GPU | strict (≤500 MiB) | idempotent by port | 🟡 **do not port as-is** |
| Portability | GNU-only (`readlink -f`, `ln -sT`, `stat -c`) | bash 3.2 | 🔴 **adapt** |

**Verdict**: the script is a **single-use Strata client**, not a launcher. It supplies three things Ropero lacks (own pack, own profile, verified tune) and one thing Ropero already does better (stop). Integration is **not** “import a script”: it requires a second Strata launch mode, a model discriminator to distinguish a catalog pointer from a prepared run config, a catalog location for derived artifacts, and a first-class `tune` operation.

One check must **not** be made: `strata launch` **cannot** serve these models; this is a design incompatibility documented by the script, not a configuration bug. Any proposal to “fix Groot's TOML” is dead before it starts.

---

### 4.14 GGUF creation (gguffer)

#### 4.14.1 What it is

**R-GGUF-1** · `ropero gguffer` is the **only** GGUF creation implementation (download → conversion → imatrix → quantization → verification → model card). The CLI and TOML `[build]` path share it through `rp_model_build_argv`: there are **not two GGUF makers**. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1-15`.

**R-GGUF-2** · gguffer does **not execute TOML**: it reads fields and assembles argv, like the rest of the system. A model is DATA (P-6).

**R-GGUF-59** · 🔶 gguffer has a unique prerequisite: an available **llama.cpp engine** (official or fork), which converts weights, quantizes, and merges shards. Other Ropero commands use an installed engine; gguffer uses it **as a tool**. This is the difference between launching a model and creating a GGUF. See R-GGUF-54…R-GGUF-58.

#### 4.14.2 CLI contract

**R-GGUF-3** · Input comes from **one of three mutually exclusive sources**: `--path <dir>` (already cloned repo), `--repo <owner/name>` (clone it), or `--gguf <file|owner/repo>` (existing GGUF). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1418`.

**R-GGUF-4** · Two distinct output options: `--out NAME|PATH` (name/directory/path), and `--out-principal PATH` (the **exact** path for the first quant in the run). Each quant needs its own output; `--out-principal` is for the one placed in the model folder expected by TOML; others remain in the normal directory named `<repo>-<quant>.gguf`. `--out-principal` **requires** a `.gguf` filename. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:27-32`, `:90-121`, `:1466-1485`.

**R-GGUF-5** · With **multiple** quants, `--out` cannot be an exact file path; it must be a name or directory. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1466-1479`.

**R-GGUF-6** · Imatrix rules are **strict and checked first**: local `--imatrix` is exclusive with `--imatrix-repo`/`--imatrix-file`; those latter two must always be together; imatrix without `--quant` is meaningless (nothing to quantize), except `--diagnostico`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:45-48`, `:1350-1371`, `ropero-code/test/test-gguffer.sh:191-214`, `:242-246`.

**R-GGUF-7** · `--quant` is repeatable and accepts comma-separated values. Already having the requested quant is **not an error**: the GGUF is placed and command exits 0 **without deleting anything**. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:27`, `:54-58`, `:1288-1295`; `ropero-code/test/test-gguffer.sh:152-156`.

#### 4.14.3 The saving that shapes the design

**R-GGUF-8** · 🔴 **Viability is checked BEFORE downloading weights**. If requested quant differs from the quant already present, gguffer fails (`rc≠0`) without downloading and suggests the unquantized sibling repo that would work (e.g. `acme/model-BF16`).

> “saving 60 pointless GB” — `ropero-code/test/test-gguffer.sh:162-169`

The engine **cannot requantize** (`requantizing from type X is disabled`). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1619-1685`; `ropero-code/test/fakes/gguffer/llama-quantize:121-129`.

**R-GGUF-9** · 🔴 An already quantized repo (NVFP4) is preserved, not quantized: output has `general.file_type = 39` (NVFP4), and the message says so. Requesting a **different** quant is an error. A quant already matching the request is left untouched. Evidence: `ropero-code/test/test-gguffer.sh:144-161`, `:219-223`.

**R-GGUF-10** · An **FP8** repo can be quantized from its dequantized F16; gguffer warns that dequantization loses the factory quant's advantage. Evidence: `ropero-code/test/test-gguffer.sh:235-240`.

#### 4.14.4 MTP head: the block imatrix does not cover

**R-GGUF-11** · Imatrix is calculated over the **base model** (64 layers), but GGUF contains the MTP head as an **additional block** (the 65th). Imatrix does not cover it; with *very-low-bit* quants (IQ2_M, IQ3_XXS…) quantizer aborts with “Missing importance matrix for tensor blk.64…”. bartowski's Qwen3.8 IQ2_M leaves the MTP block in Q4_0, which requires no imatrix. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:123-140`.

**R-GGUF-12** · Imatrix coverage is checked **per whole block**, not tensor-by-tensor: uncovered tensors are normally f32 norms, which are not quantized and require no imatrix; quantizer aborts when an entire block is uncovered. Evidence: same file, lines 123-140.

**R-GGUF-13** · MTP block is found through metadata, not tensor name: MTP tensors have ordinary block names (`blk.64.attn_k.weight`), with no `nextn` in the name. Formula: `mtp_block = block_count - nextn_predict_layers`. Dense Qwen3.8: 65, 1 → block 64; fable: 41 → 40; nemotron: `nextn=0` → no MTP. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:151-176`.

**R-GGUF-14** · For imatrix errors, gguffer translates the quantizer's opaque message (“Missing importance matrix for tensor blk.64.attn_k.weight”) into its cause (block 64 is the MTP head; imatrix covers the base model, not the head), and offers two solutions: leave that block Q4_0 via `--tensor-type`, or convert without MTP using `--no-mtp` and rename GGUF. Evidence: `bin/lib/cmd-gguffer.sh` (phase 3 error tail).

#### 4.14.5 Six phases

**R-GGUF-15** · gguffer has **six fixed phases**: (1) download/clone repo or use supplied GGUF; (2) convert to GGUF (or quantize existing GGUF directly); (3) quantize (imatrix → quantizer), with a separate log to expose quantizer's final error; (4) verify GGUFs; (5) clean up; (6) write provenance. Evidence: phase comments in `bin/lib/cmd-gguffer.sh`.

**R-GGUF-16** · 🔴 **Quantization failure deletes nothing**: safetensors clone and intermediate GGUF remain, and error explains how to retry **only** quantization, the expensive step. Failure must not cost another 60 GB download. Evidence: phase 3, “keeping weights at: $work”.

**R-GGUF-17** · 🔴 **Weights are deleted only if GGUF passes verification**. On failure: “NOT deleting weights: they are at $work”. User-supplied **input** GGUF is never deleted. Evidence: phases 4–5.

**R-GGUF-18** · Intermediate F16 is deleted only **after all** quants (the next quant may need it); if F16 is itself the result, it is kept. Evidence: phase 5.

#### 4.14.6 Provenance is required

**R-GGUF-19** · 🔴 Every produced GGUF gets `<out>.fuente.json` recording source, quant, imatrix, and whether MTP head used a separate quant.

> “A GGUF without knowing HOW it was made cannot be published with a clear conscience.”

Provenance records quant **by name** (`"quant": "Q4_K_M"`), not number; model card uses it. Evidence: phase 6; `ropero-code/test/test-gguffer.sh:179-182`.

**R-GGUF-20** · gguffer **does not misstate the result**: if weights were NVFP4 and nothing was quantized, final message says GGUF remains **NVFP4** (actual quant) and gives a recipe to quantize using the existing intermediate without downloading again. Evidence: end of phase 6.

#### 4.14.7 `ggufcheck`: header-only reader

**R-GGUF-21** · `bin/python/ggufcheck.py` reads **only the header**: no `read()` exceeds 100 MiB, even for a 300 MiB sparse file. It distinguishes `--file-type` (`general.file_type` / `llama_ftype`) from `--tipo-predominante` (tensor `ggml_type`). Evidence: `ropero-code/test/test-ggufcheck.sh:38-47`, `:51-75`.

**R-GGUF-54** · 🔶 gguffer **requires available llama.cpp** (official or fork), not as a detail but a prerequisite: without it, weights cannot be converted, quantized, or shards merged.

> “ropero gguffer only works if llama-server is available, either official or my fork” — owner, 2026-11-12

It looks in **`$LLAMA_DIR`** (default `~/llama.cpp`): converter `$LLAMA_DIR/convert_hf_to_gguf.py` (**no fallback**); quantizer `command -v llama-quantize` then `$LLAMA_DIR/build/bin/llama-quantize`; shard merger `command -v llama-gguf-split` then `$LLAMA_DIR/build/bin/llama-gguf-split`; `libggml` for ggufcheck at `$LLAMA_DIR/build/bin/libggml.{so,dylib}` or `$LLAMA_DIR/build/lib/libggml.so`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1376-1392`, `:840-846`; `ropero-code/bin/lib/01-env.sh:33-34`.

**R-GGUF-55** · 🔶 The converter has **no substitute**: without `$LLAMA_DIR/convert_hf_to_gguf.py`, conversion and GGUF creation are impossible. Quantizer and shard merger have PATH alternatives; converter does not. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1377`, `:1992`.

**R-GGUF-56** · 🔶 Check this prerequisite **before downloading weights**, naming the missing engine and variable. No guard exists today: conversion fails and classifier falsely says “missing files in clone: download incomplete”, encouraging a needless 60 GB redownload (H-27). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1957`, `:2035-2041`.

**R-GGUF-57** · 🔶 The two engines clone into different directories:

| Engine | Clone | Found by gguffer? |
|---|---|---|
| `llama-server` (fork) | `$LLAMA_DIR` = `~/llama.cpp` | yes, default |
| `llama-server-official` | `$LLAMA_OFFICIAL_DIR` = `~/llama.cpp-official` | no, gguffer checks only `$LLAMA_DIR` |

With only official engine installed, gguffer cannot find it unless `LLAMA_DIR=~/llama.cpp-official` is exported. Neither README nor docs mention these variables (engine help does: R-GGUF-33). Evidence: `ropero-code/bin/lib/01-env.sh:33-34`; both engine files; `cmd-gguffer.sh:1376`.

**R-GGUF-58** · 🔶 Engine installation does not link `llama-quantize` or `llama-gguf-split` into `$BIN_DIR`: installers link only the server binaries. Thus the PATH fallback gguffer supports is **not enabled by any installer**. Evidence: `ropero-code/bin/lib/cmd-install.sh:599-613`; `bin/lib/engines/llama-server.sh`.

**R-GGUF-22** · Modes are `--meta`, `--tipo-predominante`, `--file-type`, `--nombres`, `--bloque`, `--info`, and default validation, each with exact expected output. Evidence: `ropero-code/test/test-ggufcheck.sh:38-47`.

**R-GGUF-23** · Invalid GGUF fails with **rc=1, clear message, no traceback**: incomplete tensor index reports actual count; impossible `n_tensors` also fails; `n_tensors=2^40` in a 1 GiB file fails quickly (<5 s, “invalid GGUF”) without hanging or allocating 1 GiB. Evidence: `ropero-code/test/test-ggufcheck.sh:115-134`.

**R-GGUF-24** · Deprecated `solo_cabecera=True` remains accepted and ignored for backward compatibility. `model_card.py --gguf` must exit 0 without `TypeError` and generate README and CUANT. Evidence: `ropero-code/test/test-ggufcheck.sh:79-93`.

**R-GGUF-25** · `ggufcheck` supports **any** GGUF, not only Ropero's, and accepts `--lib <path>` for ggml library. gguffer verifies first with split `--dry-run`, then falls back to Python ggufcheck if unavailable. Evidence: phase 4; `bin/python/ggufcheck.py`.

#### 4.14.8 Five checkpoint classes

**R-GGUF-26** · Before download, preflight reads `config.json` and `hf_quant_config.json` (from HF over HTTP via `curl -fsS`, or local dir) and classifies:

| Class | Meaning | Expected `file_type` |
|---|---|---|
| `NVFP4` | weights already NVFP4; converter preserves | 39 |
| `MXFP4` | already MXFP4 (gpt-oss); engine repacks to MXFP4_MOE | 38 |
| `MIXED` | NVFP4 + FP8 mixed precision | 39 |
| `DEQUANT` | FP8/AWQ/GPTQ; converter dequantizes to F16 | 1 |
| `NONE` | no factory quant (BF16/F16) | 1 or 32 |

Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:288-378`, `:1647-1673`.

**R-GGUF-27** · Parsing handles three traps: `quant_algo` is nested under `quantization`, not root; `quant_method` may come from `producer.name`; and HF 404 is not JSON and is ignored, so nonexistent repos correctly classify as `NONE`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:223-245`, `:279-283`, `:305-306`.

**R-GGUF-28** · For each requested quant, plan covers four cases: already same quant → **do not quantize**; different quant → **impossible** (engine cannot requantize); unquantized + real quant → quantize with imatrix; unquantized + plain F32/F16/BF16 → dtype conversion only, **no imatrix**. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:426-483`.

**R-GGUF-29** · “Quantized” means `file_type` other than `""`, 0 (F32), 1 (F16), or 32 (BF16). Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:389-394`.

**R-GGUF-30** · Unknown quant aborts with rc 1 **before download**: parse two engine `llama-quantize --help` forms (aliases and canonical, `grep -v "alias for"`); error points to `ropero gguffer --list-quants`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1440-1465`.

#### 4.14.9 Full CLI surface

**R-GGUF-31** · `ropero gguffer` has **no subcommands**, only flags and two flag-selected modes: `--list-quants` (or `--quant` without a value) lists engine quants and published quants for base model; `--diagnostico` explains why quantization will not run, without quantizing. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1324-1326`, `:1391-1415`, `:2060-2064`.

**R-GGUF-32** · Flags:

| Flag | Argument | Default | Condition |
|---|---|---|---|
| `--repo` | `OWNER/NAME` | — | one of `--repo`/`--path`/`--gguf` |
| `--path` | `DIR` | — | exclusive with `--repo` |
| `--gguf` | `FILE` \| `OWNER/REPO` \| `OWNER/REPO:file` | — | — |
| `--gguf-file` | `NAME` | — | required if repo has several unquantized GGUFs |
| `--quant` | `TYPE[,TYPE]` | — | repeatable; no value ⇒ `--list-quants` |
| `--imatrix` | `FILE` | — | exclusive with repo pair |
| `--imatrix-repo` / `--imatrix-file` | | — | always together |
| `--out` | `NAME` \| `PATH/` \| `PATH.gguf` | `$GGUF_DIR` | with several quants, not a file |
| `--out-principal` | `PATH.gguf` \| `DIR/` | — | optional |
| `--from-subdir` | `DIR` | — | weights subdirectory in clone |
| `--include` | `PATTERN` | — | `hf download` filter |
| `--intermedio` | `f16` \| `bf16` | `f16` | invalid ⇒ rc 2 |
| `--keep-source` / `--keep-intermedio` / `--no-mtp` / `--force` | — | off | — |
| `--list-quants` / `--diagnostico` | — | off | — |
| `--chat-template` | `FILE` | — | **not in `--help`** |
| `--tokenizer` | `FILE` | — | **not in `--help`** |

All accept `--flag=value`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1271-1327`.

**R-GGUF-33** · Behavior-controlling environment variables missing from help: `GGUF_DIR` (`$HOME/models/gguf`), `GGUF_WORK_DIR` (`$HOME/.cache/ropero/gguffer`), `LLAMA_DIR` (`$HOME/llama.cpp`), `ROPERO_ROOT`, `MTP_QUANT` (`Q4_0`), `ROPERO_CHAT_TEMPLATE`. Found by reading code. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:975`, `:1271`, `:1376`, `:1476-1477`, `:1801`.

**R-GGUF-34** · TOML `[build]` has explicit `modelconf.py` mapping: `repo→--repo`, `gguf→--gguf`, `gguf_file→--gguf-file`, `imatrix→--imatrix`, `imatrix_repo→--imatrix-repo`, `imatrix_file→--imatrix-file`, `out→--out`, `out_principal→--out-principal`, `from_subdir→--from-subdir`, `include→--include`, `intermedio→--intermedio`; plus boolean `keep_source`, `keep_intermedio`, `no_mtp`, `force`. `quant` accepts list or comma string. Contract: **TOML recipe and manual command are identical**. Evidence: `ropero-code/bin/python/modelconf.py:1248-1263`, `:1326-1334`; `ropero-code/bin/lib/02-modelconf.sh:323-336`; `ropero-code/bin/quant/ACRITO.md:23-26`.

#### 4.14.10 Conversion: three fixes for real cases

**R-GGUF-35** · Intermediate filename describes contents: NVFP4 → `-NVFP4`; MXFP4 → `-MXFP4_MOE`; otherwise `-F16`/`-BF16` per `--intermedio`. Never label NVFP4 as F16. Retries reuse existing intermediate: either F16/BF16 is accepted; NVFP4/MXFP4_MOE requires exact match. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1630-1635`, `:1802-1826`.

**R-GGUF-36** · Pass `--no-mtp` **only if converter supports it**, checked with `grep -qE "no-mtp|no_nextn"`. Ropero engine's `Qwen3_5TextModel` ignores this flag, and passing it still aborts conversion. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1919-1925`, `:187-200`.

**R-GGUF-37** · Fix tokenizer before conversion: inject repo `chat_template.jinja` into `tokenizer_config.json` (llama.cpp converter **never reads** `.jinja`); replace transformers-v5 `tokenizer_class: TokenizersBackend` with `PreTrainedTokenizerFast`, saving `.bak`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1070-1141`, `:1984-1989`.

**R-GGUF-38** · Two automatic conversion retries: `Tokenizer class … does not exist` ⇒ fix tokenizer and retry; `--mtp / --no-nextn are not supported` ⇒ remove `--no-mtp` from argv and retry. If still failing, show `tail -20` converter log, delete partial GGUF, classify cause (disk full; RAM; incomplete clone), preserve weights, and print exact retry command. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1992-2051`.

**R-GGUF-39** · Quantizer flags come **before positional args**: `llama-quantize` parses options only while args begin `--`, otherwise errors `invalid nthread '--imatrix'`. Final argv: `llama-quantize [--imatrix F] [--tensor-type …] <f16> <output> <QUANT>`. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:2213-2219`.

**R-GGUF-40** · MTP block is found by metadata (`mtp_block = block_count - nextn_predict_layers`) and block-by-block imatrix comparison. Each uncovered block receives `--tensor-type "blk\.<N>\..*=Q4_0"`; `MTP_QUANT` configures Q4_0. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:2115-2166`, `:878-906`.

**R-GGUF-41** · Never compare `general.file_type` (llama_ftype: NVFP4=39, MXFP4_MOE=38, BF16=32, F16=1) with `ggml_type` (NVFP4=40, MXFP4=39): they are different number spaces. gguffer reads actual GGUF file type and rebuilds the plan from it. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1435-1438`, `:2072-2098`; `ropero-code/bin/quant/quant-tipos.sh:9-11`.

#### 4.14.11 Engine patches

**R-GGUF-42** · Ropero engine is a **private clone** (`Lorenzo-SF/ropero-llama-turboquant`) with public-fork fallback, and receives patches via `patch -p1`; it must be **rebuilt** for patches to take effect. Patch state is detected from code, not git; save `.ropero.bak` before patching. Missing patches afterward ⇒ rc 1. Evidence: `ropero-code/bin/quant/aplicar-parche-motor.sh:103-118`, `:153-158`, `:199-203`, `:206-220`; `ropero-code/bin/lib/engines/llama-server.sh:77-117`.

**R-GGUF-43** · Mixed-precision patch in gguffer is **fail-open** (`patch -p1 … || true`): if it does not apply, conversion continues and later fails less clearly. Deliberate, since actual error appears during conversion. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1959`.

#### 4.14.12 Script layer (`bin/quant/`)

**R-GGUF-44** · `bin/quant/` scripts wrap gguffer; `modelos.tsv` (7 columns: `n pesos q imr imf flags source`) is their source of truth. Destination is requested from Ropero (`ropero path "$model"`), avoiding duplicated TOML logic. Evidence: `ropero-code/bin/quant/cuantizar-todos.sh:5-6`, `:92-102`, `:113`, `:227`; `ropero-code/bin/quant/README.md:71-72`, `:78-81`.

**R-GGUF-45** · Destination names are never misleading: if TOML path does not contain main quant name, GGUF goes to a folder with its own name (otherwise launcher would think model is IQ2_M). Secondary quants go to `$GGUF_DIR/_variantes/<model>/`. Evidence: `ropero-code/bin/quant/cuantizar-todos.sh:146-186`; `ropero-code/bin/lib/cmd-gguffer.sh:105-118`, `:1853-1858`.

**R-GGUF-46** · A model failure does **not stop the run** (except `--parar`); each model has a log and final summary lists OK/FAIL per model. Evidence: `ropero-code/bin/quant/cuantizar-todos.sh:35-37`, `:204-212`, `:244-272`.

**R-GGUF-47** · `bin/quant/todo.sh` order is significant: **quantize → verify → upload**. Verify before upload; if any GGUF fails, **upload nothing** (`exit 1`). Evidence: `ropero-code/bin/quant/todo.sh:47-92`, `:66-82`.

**R-GGUF-48** · `verificar-gguf.sh` verifies after creation without GPU: ≥100 MiB, readable metadata, `arch`, >0 tensors, `block_count`; missing `nextn_predict_layers` is not an error (reported “(none)”); if expected quant supplied, compare `general.file_type` with its number. Codes: 0 success · 1 failures · 2 no arguments. Evidence: `ropero-code/bin/quant/verificar-gguf.sh:28-96`.

**R-GGUF-49** · HF upload (`subir-hf.sh`) creates `<HF_USER>/<model-name>-<QUANT>-GGUF` (name from filename and actual `general.file_type`), uploads GGUF, cached imatrix if available, README and CUANTIZATION.md, and puts `.hf-uploaded` in same folder. `base_model` comes from GGUF's `general.base_model.0.repo_url`, not a table. `_variantes/` GGUFs are not uploaded. Evidence: `ropero-code/bin/quant/subir-hf.sh:46`, `:105-129`, `:184`, `:200-300`.

**R-GGUF-50** · Model card comes from the GGUF, not a table, and must say it is not from the quantizer; if no imatrix, disclose that and possible quality loss. `.fuente.json` is the **only source** for imatrix because converter does not write `base_model` metadata. Evidence: `ropero-code/bin/python/model_card.py:21-27`, `:186-188`, `:272`, `:305-309`; `ropero-code/bin/quant/subir-hf.sh:230-252`.

#### 4.14.13 Three measured gguffer discrepancies

**R-GGUF-51** · 🟠 `subir-hf.sh --subir` does **not verify** uploaded GGUF; it checks only `.hf-uploaded`. Verification exists in gguffer phase 4 and `todo.sh`; directly invoking upload **uploads an unverified GGUF**. README says otherwise, which is true only through `todo.sh` or gguffer. Evidence: `ropero-code/bin/quant/subir-hf.sh:203-206`, `:278`; `ropero-code/bin/quant/README.md:82-83`.

**R-GGUF-52** · 🟠 Provenance writing is fail-open and return code ignored: if `_gguffer_escribe_fuente` fails, GGUF has no `.fuente.json` but run exits **0**. Evidence: `ropero-code/bin/lib/cmd-gguffer.sh:1006`, `:2315`.

**R-GGUF-53** · 🟡 `cuantizar-todos.sh` comment is stale: says safetensors are deleted whether success or failure, while code deliberately preserves them on failure. `parche-json-schema-to-grammar.patch` duplicates a hunk from another patch and no script applies it. `model_card.py::_tipos()` resolves quant aliases opposite `quant-tipos.sh::quant_nombre`: Python keeps last (alias), Bash first (canonical). Evidence: script lines 30-33 vs `cmd-gguffer.sh:2040-2047`; `bin/quant/patches/`; Python vs shell quant files.

---

### 4.12 Current operational contract (working tree)

This section records behavior implemented in the working tree and supplements [ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md) and [ADR-010](../docs/adr/ADR-010-comportamiento-operativo-flm-logs-y-remove.md).

#### 4.12.1 Configuration, catalog, and manifests

- **Default catalog (normal use):** external at `<config_dir>/models/<engine>/`, with manifests at `<config_dir>/engines/`. `<config_dir>` is `$XDG_CONFIG_HOME/ropero` or `~/.config/ropero`.
- **No repository autodiscovery:** repository TOML/manifests are for development and tests; they are not discovered automatically in normal use.
- **Preserved overrides:** `ROPERO_MODELS_DIR` and `ROPERO_ROOT`. If they point to `models/` under a catalog/repository root, manifest resolution remains at sibling `<root>/engines/`.
- **Bootstrap:** first use idempotently creates config directory and default `config.toml`, **without overwriting**.
- **Precedence:** `environment variable > config.toml > default` for `models_dir` (`ROPERO_MODELS_DIR`), `gguf_dir` (`GGUF_DIR`), `safetensors_dir` (`SAFETENSORS_DIR`), `bin_dir` (`ROPERO_BIN_DIR`), and `logs_dir` (`LOG_DIR`). Default logs path: `<config_dir>/logs`.
- **User manifests:** not embedded; read from `engines/` associated with the active catalog root.
- **Commands:** `ropero config` shows paths/origins; `ropero doctor` includes config and dependency status; `ropero template` suggests a template without writing it. They do not modify model artifacts, though missing config may trigger idempotent bootstrap.
- **Migration:** copy legacy TOMLs to `~/.config/ropero/models/<engine>/` and manifests to `~/.config/ropero/engines/` (or configured paths).

#### 4.12.2 Delegated FLM behavior

- **Schema/identity:** primary tag is `[model].path` if non-empty; otherwise the `--decision` value in `[args].base`. Relevant tags derive from the model (primary and declared auxiliary tags).
- **Delegated verbs:** install (`pull`), delete (`remove`), and list (`list`) as declared by engine manifest; startup may use `flm serve <tag> ...` with manifest defaults and `--ctx-len` if missing.
- **Status:** `flm list --json` is **best effort**; inconclusive response is not strong verification. This contract does not claim end-to-end FLM verification.

#### 4.12.3 Logs, color, and usage

- **Foreground:** live engine output in terminal (colored where appropriate) and model log written for diagnostics/status.
- **`--background`:** detached process, no terminal output and no `ROPERO_PORT` emission; logs remain under `logs_dir`.
- **Color:** shared semantic/heuristic handling; no per-engine colorizer is claimed.
- **`usage.log`:** local append-only `logs_dir/usage.log` with minimal data (time, model, engine, port if known, result, and mode), without API keys or flags/arguments.

#### 4.12.4 Model removal (`ropero <alias> remove`)

- **Plan:** only artifacts derivable from TOML and resolved paths (declared files/runtime auxiliaries), and delegated FLM operation if applicable.
- **Confirmation:** interactive TTY confirmation before deletion; without TTY or affirmative answer, nothing is removed.
- **Limits:** shared directories and undeclared data are not removed; a model's specific `GGUF_DIR` directory is removed only if empty after deleting individual files.
- **Safetensors:** if `[model].path` identifies a safetensors checkpoint/directory, automatic deletion is **blocked** for safety (manual removal if desired).
- **FLM:** remove the **primary tag** with `flm remove <tag>` and show declared tags; this operation does not automatically remove other declared tags.

---

## 5. Traceability matrix

### 5.1 Count by area

**317 requirements**, each with a unique ID and locatable evidence (`file:line`), except `[derived]` items (consequences of another requirement rather than a specific code line) and 🔶 items (the owner's **declared target state**, not yet in code—17 total).

| Area | Requirements | Main source files | Test coverage |
|---|---|---|---|
| `R-CTX` · Context and platforms | 3 | `bin/lib/engines/*.sh` (`compatible` hook) | 🔴 none |
| `R-ARQ` · Architecture | 11 | `bin/ropero`, `bin/lib/*` | 🟡 partial |
| `R-DATA` · Data model | 23 | `models/_template.toml`, `bin/python/modelconf.py` | 🟢 partial |
| `R-ENG` · Engine contract | 7 | `bin/lib/03-engines.sh`, `bin/lib/engines/*.sh` | 🟢 2 of 8 engines |
| `R-CLI` · Command line | 17 | `bin/ropero`, `bin/lib/cmd-*.sh` | 🟡 partial |
| `R-RUN` · Lifecycle | 17 | `bin/lib/04-launch.sh` | 🟡 partial |
| `R-EST` · State and ports | 15 | `bin/lib/04-launch.sh` | 🟡 partial |
| `R-API` · Endpoint | 25 | `bin/server/endpoint.py` | 🔴 **broken** (R-TST-18) |
| `R-ROUTE` · Routing | 5 | `bin/router.sh` | 🔴 none |
| `R-INST` · Installation | 27 | `install.sh`, `bin/lib/cmd-install.sh` | 🔴 none |
| `R-MOD` · Catalog and downloads | 31 | `bin/lib/cmd-models.sh`, `bin/lib/05-download.sh` | 🟢 good |
| `R-TST` · Test suite | 28 | `test/` | 🟢 is the test suite |
| `R-OC` · opencode | 11 | `opencode-plugin/index.ts` | 🔴 none |
| `R-STRATA` · Strata + `groot-strata.sh` | 38 | `bin/lib/engines/strata.sh`, `bin/server/strata-run.sh`, `groot-strata.sh` | 🟢 good (Ropero) · 🟡 script not |

| `R-GGUF` · GGUF creation | 59 | `bin/lib/cmd-gguffer.sh`, `bin/quant/*`, `bin/python/{ggufcheck,model_card}.py` | 🟢 good (not hermetic) |

### 5.2 Requirements without guaranteed tests

The key issue is **which requirements have a test guaranteeing them**. This records each requirement with observable behavior that can be checked against the current repository.

| Requirement | Test verified? | Note |
|---|---|---|
| `R-TST-18` (auth guard) | ❌ false positive | test is green with broken auth |
| `R-API-1`…`R-API-22` | ❌ | blocked by R-TST-18 |
| `R-INST-1`…`R-INST-27` | ❌ | no tests for `install.sh`/`cmd-install.sh` |
| `R-OC-1`…`R-OC-11` | ❌ | no plugin tests |
| `R-ROUTE-1`…`R-ROUTE-5` | ❌ | no tests touch `router.sh` |
| `R-CTX-1`…`R-CTX-3` (compatibility/maturity) | ❌ | `compatible` hook untested |
| `R-GGUF-54`…`R-GGUF-58` (engine prerequisite) | ❌ | 🔶 guard missing: H-27, H-28, H-29 |
| `R-CLI-14`…`R-CLI-17` (surface and `stats`) | ❌ | 🔶 target state, not implemented |
| `R-DATA-21` (`name` = file) | ❌ | validator does not check (H-23) |
| `R-EST-15` (foreign engines) | ❌ | 🔶 undecided (H-26) |
| `R-RUN-13` (stamp) | ❌ | manually verified in this audit |
| `R-MOD-24`…`R-MOD-25` (symlinks) | ✅ | `test-models-symlink.sh` |
| `R-GGUF-8`…`R-GGUF-10` (viability) | ⚠️ | logic tested; environment not (R-TST-19) |
| `R-MOD-20` (no hash) | ✅ | verified by inspection and test |
| `R-STRATA-1`…`R-STRATA-13` | ✅ | `test-strata.sh` (45 checks) |

### 5.3 Five most costly failures

| # | Requirement | Why |
|---|---|---|
| 1 | `R-STRATA-23` (`serve --profile` mode) | without it, **the two Groot models cannot be served by Ropero**; it is a necessary condition, not an enhancement |
| 2 | `R-TST-18` (propagate Python block rc) | a checker reporting green without checking consumes trust; worse than no checker |
| 3 | `R-API-13` (do not forward key to backend) | key in process argv is readable by same-host users via `/proc/<pid>/cmdline` |
| 4 | `R-EST-8` (do not kill recycled PID) | OS recycles PIDs; without identity validation, `ropero stop` could kill a browser |
| 5 | `R-STRATA-23` (`serve --profile` mode) | without it, both Groot Gyro-M models cannot be served; this is design incompatibility, not configuration |

---

## 6. Gaps, debt, and contradictions

### 6.1 Main gap

**H-1 · The specification cited by code does not exist.** Code references `spec §N` and `ADR-1` in docstrings, comments, and function names (`ropero-code/bin/lib/cmd-gguffer.sh:3` says “spec §8.15”; `bin/lib/03-engines.sh` refers to `ADR-1`; `bin/server/strata-run.sh` cites `ADR-1` for bash 3.2 constraint). `ropero-code/README.md:177` announces `docs/specs/`. **That directory is absent**; `docs/` contains only test and Strata/FLM markdown files.

⇒ **This document reconstructs that specification** from code, not from the absent spec. It documents what exists and specifies its behavior; that is why every requirement has line-level evidence.

**H-2 · The documented test-suite runner does not exist.** `ropero-code/README.md:194` and `ropero-code/CHANGELOG.md:205` point to `tests/suite/run.sh`; actual path is `test/` (moved in `4b133bf`), also correctly stated in `ropero-code/README.md:176`. There is no `.github/`, `Makefile`, or `justfile`: **no documented command runs the complete suite**.

### 6.2 Documentation/code contradictions

| # | Contradiction | Verdict |
|---|---|---|
| H-3 | CHANGELOG and `fastflowlm.sh` say FLM is **behind** `endpoint.py`; actual launch is direct `flm serve`, required by test | **test prevails**; FLM port remains locally accessible without key |
| H-4 | CHANGELOG says `llama-server-official` is **hidden**; code exposes it and `install engines` offers it equally | **code prevails** |
| H-5 | CHANGELOG says FLM refuses to launch without `LLAMA_API_KEY`; test says otherwise | **test prevails** |
| H-6 | README promises `tests/suite/run.sh` | **does not exist** |
| H-7 | Engine header/CHANGELOG describe AtomicBot fork; actual engine is private `Lorenzo-SF/ropero-llama-turboquant` | **code prevails** |

### 6.3 Measured functional debt

| # | Debt | Size | Consequence |
|---|---|---|---|
| H-8 | **`R-ROUTE-3`**: `router.sh` fallback is `minimax/m3.1-flash`, absent from catalog | 1 line | selector can return model Ropero cannot serve |
| H-9 | **`R-ROUTE-4`**: `router.sh` has no consumer and header promises nonexistent `MODELS.md` | ~40 lines | dead script references missing file |
| H-10 | **`R-ROUTE-5`**: classifier request interpolated without escaping | 1 line | quotes in description break JSON and silently trigger fallback |
| H-11 | **`R-STRATA-28`**: `vram-reserve` listed but option block commented in both Strata TOMLs | 5 lines | measured 1344 MiB reserve preventing NVIDIA driver hang cannot be passed |
| H-12 | **`R-OC-11`**: plugin default provider `"marichocho"` absent from current config | 1 line | plugin cannot route as configured |
| H-13 | **`R-API`**: endpoint has no CORS or rate limiting; key is in process argv, readable on shared host via `/proc/<pid>/cmdline` | — | actual exposure on shared host |
| H-14 | **`R-GGUF`/`R-TST-19`**: gguffer test depends on host `transformers` | 1 stub | 15/38 checks fail in clean environment |
| H-15 | **`R-TST-22`**: no tests for `[levels]` resolution | — | half the catalog config surface has no safety net |
| H-16 | **`R-TST-23`**: 21 code files untested, including 6/8 engines | 21 files | doctor, install, catalog, and six engines unguaranteed |
| H-17 | `docs/strata.md` lists models absent from catalog; CHANGELOG has duplicate FLM section | — | misleading documentation |
| H-18 | Repository front door: README and CHANGELOG do not match file tree | — | first user contact is misleading |
| H-19 | **`R-GGUF-51`**: direct `subir-hf.sh --subir` checks marker only, not GGUF; unverified artifact can be uploaded despite README claim | 1 script | broken artifact can be published |
| H-20 | **`R-GGUF-52`**: provenance write is fail-open and return code ignored; GGUF may lack provenance while run exits 0 | 1 line | untraceable artifact reported as success |
| H-21 | **`R-GGUF-53`**: stale cleanup comment, unapplied duplicate patch, and Python vs Bash quant alias ordering differ | 3 places | docs and code disagree |
| H-22 | **`R-CLI-14`**: `install` exposes five subcommands (`setup`, `engines`, `models`, `link`, `opencode-plugin`), target is two | 3 subcommands | user must know more than necessary |
| H-23 | **`R-DATA-21`**: validator does not require `[model].name` to match filename | 1 check | duplicate model names could enter catalog |
| H-24 | **`R-CLI-17`**: `ropero stats` absent; nearest are doctor and one-shot `bin/diagnostico.sh`; no live CPU/GPU metrics or `--watch` | whole command | requested indicator missing |
| H-25 | **`R-API-24`**: FLM header/CHANGELOG claim proxy; launch/test show direct | 2 places | engine docs contradict behavior |
| H-26 | **`R-EST-15`**: Ropero cannot see manually launched engines; Linux `nvidia-smi --query-compute-apps` could, macOS unresolved | 1 capability | “Ropero won't start” and “GPU occupied by other engine” look alike |
| H-27 | **`R-GGUF-56`**: gguffer does not check engine before downloading weights; conversion fails and classifier calls it incomplete download (H-28) | 1 guard | environment failure misdiagnosed as download failure |
| H-28 | **`R-GGUF-57`**: with only official engine, clone is `~/llama.cpp-official`, but gguffer checks `~/llama.cpp`; not found unless manually setting `LLAMA_DIR`; no gguffer env vars appear in help | 1 variable | half of installed engines cannot enable creation |
| H-29 | **`R-GGUF-58`**: install does not link `llama-quantize` or `llama-gguf-split`, so PATH fallback is never enabled | 2 binaries | gguffer only works with clone in `$LLAMA_DIR` |
| H-30 | **`R-MOD-29`**: v2 ignores unavailable/already-default `--prio`; this catalog documented usage error (exit 2), owner decision 2026-11-14 | 1 rule | inheriting v1 aborts installation instead of continuing with other models |

### 6.4 What this document does **not** cover

- **Model quality:** evaluation suite produces a judge report, not a reproducible score. This document specifies the mechanism, not outcome.
- **Measured performance:** `bench-motor.sh` is a benchmark, not a test; repository has no throughput figures. No speed claim is made here.
- **Host security:** Ropero protects model access, not the host. An endpoint on `0.0.0.0` without CORS/rate limiting is an exposure decision, not a Ropero defect—but someone must make that decision.
- **Windows support:** repo is Bash/Unix; no claim is made.

---

## 7. Glossary

| Term | Meaning in Ropero |
|---|---|
| **engine** | Inference engine. In code, **one file** (`bin/lib/engines/<id>.sh`) with 8 hooks; not a class or plugin. |
| **model** | Catalog entry: one TOML. A model is **data**; engine serves it. |
| **TOML** | Catalog format. Describes **what** to serve, **which** flags, and **which** level; never **how** to launch (engine assembles argv). |
| **level** | Configuration profile for same model (`think`, `instruct`…). Each level yields **different args**, so server restarts. |
| **alias** | Alternate model or level name; represented by an extra catalog-array row. |
| **fired** | **Retired** model: kept as record, not installed or launched. |
| **stamp** | `cksum` of launch args; detects TOML changes while model runs. |
| **state** | `$STATE_DIR/ports/<port>` files with `MODEL= ENGINE= PID= PORT= LEVEL= TS= LOG= STAMP=` lines; source of truth. |
| **preflight** | Checks **before** touching port: engine installed, argv valid, artifact on disk, chat template present. |
| **readiness** | Wait for model to **respond**, not merely for port to listen. |
| **endpoint** | `bin/server/endpoint.py`: OpenAI-compatible proxy with its own auth; single gateway. |
| **proxy** | Endpoint behavior toward backend: rewrites `model`, removes `authorization`, translates `/v1/completions`. |
| **backend** | Process behind proxy (`llama-server`, `flm serve`, `strata launch`…). Only proxy is network-exposed. |
| **pack** | (Strata) Absolute tensor-position index **for one specific GGUF**; cannot be reused across GGUFs. |
| **expert-profile** | (Strata) Expert ranking deciding what stays in VRAM; it is the expert-cache ceiling. |
| **run config** | (Strata) JSON launch args for a model. Written by `setup.py`, modified by script per GGUF. |
| **tune** | (Strata) Machine measurement producing preset applied at each launch. |
| **gguffer** | GGUF creation: download → conversion → imatrix → quantization → verification → model card. |
| **imatrix** | Importance matrix indicating tensors to quantize with more bits; very-low-bit quants abort without it. |
| **MTP** | Next-token prediction head, an **extra** block (65th) not covered by base-model imatrix. |
| **fuente.json** | GGUF provenance: source, quant, imatrix, and whether MTP was separate. |
| **symlink-only** | Installer policy: HOME receives only symlinks; repository is source of truth and never modified. |
| **test double** | Fake executable in `test/fakes/` replacing real tool to verify contract without GPU, network, or touching user state. |
| **level (or mode)** | Named model profile (`think`, `instruct`, `low`…) in `[levels]`, applied via `--<level>`. Default applies with `ropero <model>`. |
| **flag** | Model switch (`--flash`) adding launch arguments; in `[flags]`. |
| **option** | Model `key=value` (`--temp 0.7`) with default value; in `[options]`. |
| **build** | Creation path: download weights, convert to unquantized GGUF, then quantize. Other path is **download** (fetch GGUF or safetensors as-is). |
| **gguffer** | Manual GGUF creation command; equivalent to `ropero install models --prio build`. |
| **proxy** | `bin/server/endpoint.py`: OpenAI-compatible authenticated gateway for engines without a built-in server. |
| **preflight** | Checks before touching port: installed engine, valid argv, artifact on disk, chat template present. |

---

## 8. Document provenance

### 8.1 What was read

| Source | Size | Method |
|---|---|---|
| `bin/ropero` + `bin/lib/*` | 11,022 Bash lines | direct reading of 6 base modules and 7 `cmd-*` files |
| `bin/python/*` | 4,188 Python lines | direct reading |
| `bin/lib/engines/*.sh` | 8 files | direct reading of all 8 |
| `bin/server/*` | endpoint + bridge | direct reading + subagent |
| `install.sh`, `models/`, `opencode-plugin/`, `test/`, `docs/` | all | extraction subagents + direct reading |
| attached `groot-strata.sh` | 486 lines | complete line-by-line reading |

**Extraction subagents** (read-only, no repository changes): four parallel tasks—endpoint/router, gguffer/quant, test suite, installation/models. Each ended with clean `git status --porcelain`.

### 8.2 What was measured (not supplemented)

- **317 requirements**, unique IDs and `file:line` evidence (17 are owner-declared target state, not implemented).
- **Test suite**, run with temporary `HOME` and fakes on PATH: **5/5 suites pass in ~2 s, exit 0**, no GPU, network, or process killing.
- **Auth false positive**, reproduced: break `authed()` in a copy so any key is accepted → Python block reports 5 failures, script exits **0**.
- **gguffer non-hermeticity:** 15/38 checks fail in sandbox without `transformers`.
- **`bin/router.sh` fallback:** does not match any catalog model.
- **Catalog:** 8 engines, 17 active models, 28 retired, 16 test scripts.

### 8.3 What was not verified

- **Nothing from real runtime was run**: no engine launched, model downloaded, or GPU touched. Strata throughput figures (228 t/s at 4K, 273 under real load) are **quotes from Strata documentation**, not this audit's measurements.
- **Installed engine state** on owner's machine: `ropero doctor` was not run.
- **Correctness of all 28 retired models:** discovery mechanism was verified, not every file under `fired/`.

### 8.4 What does not come from code

The **17 🔶 requirements** come from the owner's statement (2026-11-12), not code lines. They define the contract the project **wants**; current code does not meet them, and each gap is recorded in §6. Disputing them requires discussing **intent**, not code.

### 8.5 How to falsify this document

Each requirement is a locatable claim. To refute one, open the file, go to its line, and inspect it. If the line does not say what this document claims, the requirement is false and must be corrected. That is the repository's own criterion (P-1: evidence takes precedence over assumption), and this document applies it to itself.
