# Changelog

[Read this changelog in Spanish](CHANGELOG.es.md).

## Unreleased — 2026-11-18

### Changes

- `status` now uses a compact CPU/GPU, RAM/swap/zram layout and lists active
  Ropero models; `stop` reads only the process state it needs.
- Terminal colors use RGB truecolor derived from the documented hexadecimal
  palette. The engines table no longer includes a color-name column.
- Strata profile lookup checks the exact installed run-config first, and model
  availability inspects only catalog-referenced profiles.
- Interactive `models add` selects installed engines, configures common args
  before extensions, previews the normalized TOML, and can edit a block or cancel.
  Repositories, build routes, and multiple auxiliary files remain configurable.
- The model template is shorter and leaves `[args].base` empty.
- `ropero test` now accepts `--model` with exactly one of `--test` or
  `--test-group`, and optional `--path`; interactive selection lists available
  models, groups, and tests. Each run writes a Markdown report directory and
  stops a server it started.

- Model format declarations are checked against the selected engine manifest;
  MLX now receives its resolved model path and is bound to loopback by default.
- MLX and TensorRT maturity labels now match the recorded evidence. The
  TensorRT NVFP4 checkpoint rejected by the tested release is retired.
- Strata accepts its profile-backed `strata` format; its model TOMLs and guide
  now describe the installed-profile IDs returned by the upstream CLI.
- The FLM install manifest uses TOML 1.0-compatible dotted keys.

- `models add`, `info`, and `command` are complete; `models` replaces `list`.
- `config --lang ES|EN` saves the language preference. FLM uses the canonical
  `fastflowllm` id, and Ignis models separate their internal name from their
  public alias.
- `connect` uses `--remove`; `--list` reports only implemented integrations.
- `connect` prints manual setup instructions for TRAE and TRAE CN. OpenCode
  preserves comments and trailing commas when editing `opencode.jsonc`, and
  leaves the file untouched if the JSONC is invalid.
- Model startup accepts the levels, flags, and options declared in each TOML.
- Codex stores the provider and per-engine profile in `CODEX_HOME` (or
  `~/.codex`) and rejects engines without documented Responses API support.
  llama.cpp receives the public alias through `--alias`.
- `ropero test` uses `--model`, `--test`, `--test-group`, and `--path`.
- Ignis stores and looks up each download under `ARTIFACTS_DIR/<internal-name>`;
  Codex validates optional TOML tables without panic-prone indexing.
- `models` checks Strata's model inventory using the upstream `models inspect`
  command for each referenced run config.
- `info`, `config`, help, and the `test` Markdown report follow the saved
  language. The report links subagent traces and metrics.
- Public aliases are applied where supported, including Strata through a
  private copy of its profile and MLX-Serve through its per-path global
  settings; partial engines are identified as such in `models --all`.
- `connect --agent codex` no longer panics if `config.toml` lacks
  `[model_providers]`; it creates the table and validates its structure.

### Engine installation work

- `ropero install engines` runs declared `[install]` recipes to build upstream
  `llama-server` and the public `turbo-llama` fallback before creating the
  symlink. The Turbo recipe does not apply the private turboquant patches;
  other engines without recipes still require their runtime or binary.

This file records changes to **ropero v2**, the Rust reimplementation being
built in `crates/`. The v1 changelog is not included in this checkout.

The format follows [Keep a Changelog](https://keepachangelog.com/es/1.1.0/) and
versioning follows [SemVer](https://semver.org/lang/es/).

## [Unreleased] (working tree)

Contract and operational changes documented for the current working tree:

- **Configuration and catalog (ADR-008):** external catalog by default under
  `<config>/models/<engine>/` with manifests under `<config>/engines/`; an
  idempotent bootstrap of `<config>/` and `config.toml` without overwriting;
  precedence `environment > config.toml > default` for `models_dir`, `gguf_dir`,
  `safetensors_dir`, `bin_dir`, and `logs_dir` (default `<config>/logs`).
  `ROPERO_MODELS_DIR`/`ROPERO_ROOT` overrides are preserved; repository
  TOMLs/manifests are for development and tests and are not auto-discovered in
  normal use.
- **CLI surface:** `ropero config` shows paths and their source; `ropero doctor`
  includes configuration status; `ropero template` suggests a template without
  writing it. These commands do not modify model artifacts (an idempotent
  configuration bootstrap may occur).
- **Delegated FLM (ADR-010):** primary tag from `[model].path` or `--decision`;
  installation/removal/listing delegated to the manifest where applicable;
  `flm serve <tag>` with defaults and `--ctx-len` when missing; status from
  `flm list --json` is **best effort** (without claiming end-to-end verification).
- **Execution and logs (ADR-010):** detached `--background` without terminal
  output or `ROPERO_PORT`, logs under `logs_dir`; foreground mode with live
  output (common semantic coloring plus engine-specific patterns) and a model
  log; append-only usage records in `logs_dir/usage.log` (without API keys or
  flags/arguments).
- **Model removal (ADR-010):** `ropero <alias> remove` plans derived artifacts,
  requires interactive confirmation in a TTY, does not delete shared or
  undeclared files, removes a model directory under `GGUF_DIR` only if empty,
  removes only the declared safetensors checkpoint, and delegates removal of
  all declared FLM tags that were installed.
- **Migration:** copy legacy TOMLs to `~/.config/ropero/models/<engine>/` and
  manifests to `~/.config/ropero/engines/` (or configured paths).

## [0.1.0] - 2026-11-15

This is the workspace version (`Cargo.toml`). It gathers the v2 work published
through the forge, ordered by phase: **F0** foundations, **F1** data model,
**F2** engines, **F3 and F4** runtime and state, **F5** catalog and downloads,
**F6** forge (`ropero gguffer`).

### Added

**F0 · Foundations**

- **Six-crate Rust workspace** (doc B T-01: `ropero-core`, `ropero-runtime`,
  `ropero-download`, `ropero-forge`, `ropero-server`, and `ropero-cli`), with
  Rust 1.100.0 pinned in `rust-toolchain.toml`, edition 2024, `Cargo.lock`, and
  a release profile (`lto = thin`, `codegen-units = 1`, stripped symbols,
  `panic = abort`). The work had lived on disk without version control, one
  persistence failure away from being lost: versioning it makes continued
  development and verification possible.
- **Reproducible toolchain, configuration, and CI:** `rustfmt.toml`,
  `clippy.toml`, `deny.toml`, and a Linux/macOS workflow with `fmt`,
  `clippy --all-targets --all-features -D warnings`, `cargo-deny`, `test`,
  `nextest` with `--all-features`, and coverage with `cargo llvm-cov --lcov`.
  These pin versions and make changes following the build specification
  comparable.
- **Architecture test** (`R-ARQ-1`, T-02 gate): `ropero-core` does not depend on
  forbidden I/O or higher-level crates, the crate graph has no cycles, and
  `ropero-cli` remains a dispatcher under a line-count threshold. The threshold
  has been recalibrated three times **by measurement, not estimation**:
  400 → 800 ([ADR-005](docs/adr/ADR-005-umbral-de-lineas-del-cli.md)),
  800 → 1100
  ([ADR-006](docs/adr/ADR-006-umbral-de-lineas-del-cli-tras-install.md)), and
  1100 → 1200
  ([ADR-007](docs/adr/ADR-007-umbral-de-lineas-del-cli-tras-gguffer.md)). The
  measurement remains the same: ensuring logic does not migrate into the CLI.
- **The three F0 ADRs, accepted:** [ADR-001](docs/adr/ADR-001-por-que-rust.md)
  chooses Rust; [ADR-002](docs/adr/ADR-002-extraccion-vs-reescritura.md) chooses
  a layered extraction and keeps v1 and v2 side by side until F9; and
  [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md) makes
  `ropero-code/` **a reference, not a dependency**, and gives v2 a new,
  independent catalog. The second avoids a single jump that would break
  contracts and on-disk state; the third avoids inheriting v1's structure and
  defective data.

**F1 · Data model**

- **Typed data model and catalog validator** with the contract's **15 rules**
  (doc B §4.1.1) and `path:line` errors. Pure validation checks the name and its
  uniqueness, that the name matches the file, the format, files, downloads,
  placeholders, levels, aliases, options, minimum RAM, unknown keys, and that
  `context`, when declared, is a multiple of 32. Unknown-key parse errors and
  unknown engines are classified as `UnknownKey` and `EngineInvalid`; syntax
  failure is `ParseInvalid`, which is not a rule. This ensures the reported
  rule identifies the actual failure. TOML types close `R-DATA-1…16` and the
  validator closes `R-DATA-17…23` (doc B T-06 and T-07): invalid TOML does not
  pass without explaining where it failed.
- **Catalog with uniqueness and aliases**, discovery of active and retired
  models, and name and alias resolution. An unknown engine directory is
  reported as a catalog error but does not abort scanning the remaining models.
- **The v2 catalog under `models/`** (doc B §2.4): 46 versioned TOMLs (17
  active + 28 retired under `fired/`, each under its engine directory, plus
  `_template.toml`), generated from v1 TOMLs as a **read-only reference**.
  Two corrections: `context = 133072` → `131072` in
  `mlx/metalnemotron.toml` and `mlx/metalqwen.toml` (the standard 2¹⁷ = 128 K,
  a multiple of 32; 133072 was a typo), and `tensorrt/groot.toml` renamed to
  `tensorrt/groot-trt.toml` so the filename matches `name = "groot-trt"` (the
  name itself is **not** changed: names are globally unique and
  `llama/groot.toml` already uses `groot`). Everything else is identical to
  the source, and `ropero validate` on the new catalog reports
  `OK: 45 model(s) validated` (exit 0).
- **`ModelSpec` and its paths**: the format declares where an artifact lives
  (R-MOD-6) — `gguf` is `$GGUF_DIR/<model>/<[files].gguf>`, `safetensors` is
  `$SAFETENSORS_DIR/<[model].path>`, and for `q4nx`, `[model].path` is an FLM
  tag; for `strata`, it is `<family>/<package>`. A single resolver calculates
  paths for runtime, download, and listing so a model cannot live in one place
  and be searched for in another.
- **Argument vector assembly** in the order `base → level → flags → options`,
  with placeholders, `requires` checks, level aliases, and `replace`. `replace`
  substitutes the pair in the base at its original position so an option is not
  duplicated and engine precedence is not left to chance. Covers `R-DATA-16`
  and `R-DATA-17`; `R-DATA-13` remains tied to the engine contract.
- **Golden-master argument vector** frozen from v1 output for four real models.
  Reference TOMLs are frozen under `crates/ropero-core/tests/golden/models/`,
  so tests do not depend on the live catalog. Observable behavior is fixed
  before reimplementing a function to catch regressions during extraction.
- **Engine identifiers and legacy aliases:** canonical ids and inherited
  aliases are canonicalized with a notice, never silently. Closes `R-ENG-1`
  within the `EngineId` enum.

**F2 · Engines**

- **Eight engine manifests** (`engines/*.toml`), one per engine alongside the
  catalog: id, label, color, description, formats, maturity, port range,
  dependencies (what `ropero doctor` checks), OS and hardware `compat` rules,
  and `[detect]` with binary and installation paths. Engines become **data**:
  the legend, ports, and dependencies each have one source of truth, and an
  unknown field is rejected with the offending field named. `maturity`
  distinguishes the six engines actually started on the owner's machine
  (`proven`) from the two integrated but never started (`vllm`, `strata`), and
  the criterion is recorded in each engine TOML.
- **Canonical engine names**
  ([ADR-004](docs/adr/ADR-004-nomenclatura-de-motores.md)):
  `llama-server` is the **official** llama.cpp repository and `turbo-llama` is
  the owner's **private fork**. The change **reverses** the two v1 names, where
  `llama-server` referred to the fork; legacy aliases (`llama-server-official`,
  `llama-official`, `llama`, `turbo`) remain accepted with a notice. The catalog
  is migrated by renaming `models/llama/` → `models/turbo-llama/` and
  `models/llama-official/` → `models/llama-server/`; legacy directories keep
  mapping so an old catalog is read the same way.
- **Compatibility evaluation accumulates all reasons:** manifest rules are
  **alternatives** (the machine qualifies if it satisfies at least one), while
  conditions within a rule are **cumulative**. A Linux user who wants
  TensorRT-LLM needs to know that two things are missing (an NVIDIA GPU and
  32 GB RAM), not just one. Machine information is passed as a parameter —
  `ropero-core` still does not read from disk — and data that could not be read
  is treated as absent; it never brings down the scan.
- **Non-overlapping per-engine ports:** `llama-server` 8100-8199,
  `turbo-llama` 8200-8299, `mlx-serve` 8300-8399, `fastflowlm` 8400-8499,
  `tensorrt-llm` 8500-8599, `vllm` 8600-8699, `airllm` 8700-8799, and `strata`
  9900-9999. Several engines can serve at once without colliding, and each
  launch checks that no process is listening on the port it will use.

**F3 and F4 · Runtime and state**

- **Pure startup preflight and port selection:** preflight (engine installed,
  model installed, VRAM/RAM) accumulates **all** reasons and each reason says
  what to do; `pick_port` returns the lowest free port in the engine's range,
  and an inverted range is reported as an error rather than silently fixing
  its endpoints. These are decisions, not reads, so they can be tested without
  a machine.
- **Model startup with preflight, probe, identity seal, and on-disk state**
  (doc B §3.3): preflight, `--force`, port, `spawn`, probe, and state. The
  **identity seal** is the complete identity (`PID` + `start_time` + binary
  digest), which prevents killing unrelated processes (P-3). State is written
  atomically (tmp + rename) and retains keys that were not supplied; new keys
  are compatible additions, so a v1 reader still understands the file. The
  engine is launched in its own process group so its output goes to the log;
  without this, an engine that aborts would not explain why.
- **Three-level readiness probe:** TCP (port accepts connections) → HTTP (health
  endpoint returns 200) → inference (`GET /v1/models`). They are tried in order
  and the first passing check is sufficient: waiting for inference on a model
  that loads quickly wastes the user's time, while an early open port is a
  false positive. This is the same test as v1, with the API key, and it reports
  **which** model is serving.
- **Foreground mode propagates Ctrl-C:** Ropero waits for the engine, catches
  SIGINT, and sends it to the process group; otherwise Ctrl-C would leave the
  engine serving and Ropero dead. It exits with **130** (doc CLI v2 §3). The
  port is announced as `ROPERO_PORT=<n>` as soon as the engine is ready: it is
  the last stdout line and the value read by scripts.
- **Stop by verified PID, never by process name** (R-EST-8): state exists, the
  process is alive, `start_time` matches, and the digest matches; then SIGTERM
  is sent to the group, followed by a wait and SIGKILL. It never kills by
  process name (a `pkill -f` would kill engines from another project), a
  recycled PID, or state lacking `start_time`. Stopping one model respects
  other models sharing the engine because it kills by PID.
- **Twelve commands and two invocations:** `install`, `stop`, `gguffer`,
  `status`, `doctor`, `list`, `engines`, `template`, `validate`, `args`,
  `levels`, and `path`, plus bare `ropero` and `ropero <model>` (six flags:
  `--force`, `--cpu`, `--port`, `--api-key`, `--no-api-key`, `--ctx`). The
  contract lives in the domain (`ropero_core::surface`) and the CLI translates
  it: a command that cannot yet run is **parsed** and says what is missing
  (exit 1), while an invalid flag remains a usage error (2). `build.rs` pins
  the version, build date (with `SOURCE_DATE_EPOCH`), and author.
- **Summary, colorized `list`, `status`, and `doctor`:** bare `ropero` shows
  name, build, author, engines and models (available and installed), and the
  `--help` suggestion; `ropero list` lists **only** available models (engine
  installed **and** model downloaded), grouped by engine, with a 256-color ANSI
  legend; `--all` adds unavailable entries and marks them; an uninstalled
  engine is **folded**, with its count and reason. `ropero status` shows the
  machine (CPU, dGPU, iGPU, RAM/SWAP, with zram shown separately to avoid
  confusion); unreadable values appear as “unavailable”: a misleading status is
  worse than one with gaps. `ropero doctor` checks Ropero's own dependencies
  first and then those of **each** engine, reporting whether each is met and,
  if not, what to do.

**F5 · Catalog and downloads**

- **`ropero install engines [<list>]` and `ropero install models [<list>]`:**
  with a list, install directly (unattended mode); without one, show a menu, and
  the model menu offers **only models from installed engines**. Without a
  terminal, the menu is a usage error (2): a menu that consumes EOF could make
  a selection by itself. Engine installation is **only a symlink** (R-INST-1)
  to its binary, resolved with precedence **PATH → `$BIN_DIR` → long paths**
  (R-INST-24) and linked using its `basename` (R-INST-25): no copying,
  downloading, or compiling, and the engine used is the one on `PATH`, not a
  newly compiled one. An already linked engine is reported and left alone, and
  a real file at the destination is not overwritten.
- **The `--prio` contract** (doc B §4.10.7), in full: `--prio=download|build`
  may be **global** (before the first model) or **per model** (applies to the
  last preceding model, and its scope ends at the next model), with **per-model
  precedence**; it is **ignored** if that model lacks the selected path — and
  the notice does **not** abort other models — and the user is **prompted** if
  the model declares both paths and no `--prio` was supplied. An invalid value
  is a usage error that includes the complete received value; `--prio` with
  `install engines` is a usage error because it applies only to
  `install models`. This is pure domain logic, so the entire contract is tested
  with inputs.
- **Real `hf` download following the exact R-MOD-16 sequence:** the pure plan
  calculates destination, repository, file, informational size, excludes,
  auxiliaries, and the exact `hf` arguments; execution follows the spec (final
  file present and non-empty → “already downloaded”; file named after the repo
  → `mv`; **ask before running `hf`**; confirmation shows size and defaults to
  **no**; `hf download` to staging; search by `basename`; rename comparing
  absolute paths; `rm -rf` staging). GGUF uses its own staging area **inside
  the destination** (`mv` is atomic and two downloads with the same filename do
  not collide); checkpoints do **not** (R-MOD-17): `hf` resumes using a fixed
  `--local-dir`, and staging would destroy the resumability of a 12 GB download
  interrupted halfway. Verification checks actual contents (R-MOD-18) and is
  **repeated even if `hf` exits 0** (R-MOD-19): `hf` output does not prove the
  file exists. There is no hash verification; state is inferred from disk.
  Auxiliary files with `use = run` are downloaded into the GGUF directory, and
  `hf` output goes to stderr so stdout remains available for data.
- **`ropero install models <list>` really installs the `download` path**, and
  each model's line says what happened (“already downloaded”, “downloaded”,
  “skipped by user”, or “error: …”), with exit codes 0 (all installed or
  skipped), 1 (something failed), and 2 (usage). The `build` path tells the
  truth: producing the GGUF is the forge.

**F6 · Forge**

- **`ropero gguffer`**, the last command in the v2 CLI contract (§11): builds a
  GGUF from a Hugging Face repository, a local weights directory, or an
  existing GGUF. Its **seventeen flags** — the fifteen in the doc's table
  (§11.2) plus `--gguf` and `--out-principal`, from R-GGUF-3/4 — include
  repeatable, comma-separated `--quant`. The `--imatrix-repo`/`--imatrix-file`
  pair is checked **by the domain**, not by `clap`, so the message matches the
  Spanish doc and says what to do. `--list-quants` shows the quants published
  by the engine (name ↔ `file_type`) and those already published for that
  model; without an engine it shows the fallback table and what to do.
  `--diagnostico` shows which engine is present, where it is, and what is
  missing, and is exempt from the imatrix rule. The logic does not live in the
  CLI: `gguffer` is 77 lines, 19 of them comments.
- **The six run phases** (R-GGUF-15), in fixed order: source → conversion →
  quantization → verification → cleanup → provenance. Conversion produces an
  intermediate only when quantization will follow; placement or flat dtype
  conversion goes directly to the final file, without imatrix or hours of CPU.
  The quantizer runs with a separate log and its last 20 lines go to stderr; an
  imatrix error is translated for both MTP output heads.
- **Per-phase checkpoint with a request fingerprint**
  (`<work>/.ropero-forge.json`): the fingerprint is the canonical request
  (source, quants in order, intermediate format, imatrix, `out`,
  `out-principal`, `no-mtp`/`no-dflash`/`force`); `--keep-*` is excluded because
  it does not change what is produced. A different fingerprint discards the
  checkpoint and **restarts** the run, and `--force` always discards it; the
  report says so. This prevents an expensive run from repeating for free: if
  quantization fails at three in the morning, the download (an hour, 60 GB) and
  conversion are **not** repeated. A checkpoint without a fingerprint or one
  that is corrupt does not resume.
- **Cleanup policy** (R-GGUF-16/17/18) is in **one pure function** executed by
  phase 5: quantization failure **deletes nothing** — a quantization failure
  must not cost 60 GB of downloads; weights are deleted only after successful
  verification because a corrupt GGUF with safetensors is worth more than
  60 GB; a user-supplied GGUF is **never** deleted; the intermediate is deleted
  after all quants, but if F16 **is** the result it remains untouched. The
  cleanup checkpoint is written **before** deletions because cleanup may
  delete the work directory containing the checkpoint.
- **Provenance is mandatory** (`<out>.fuente.json`, R-GGUF-19): model source,
  quant **by name** (not number: that is what goes into the model card), the
  importance matrix with its file, repository and sha256, and the MTP head if
  it ended up in a separate quant. JSON is written by hand, like the rest of
  the forge, so the crate gains no extra dependencies. **A GGUF without a
  sidecar does not exit successfully**: the “already exists” path used to exit
  0 without provenance; now the run fails with guidance (delete the GGUF and
  rebuild, or rebuild with `--force`). “A GGUF whose build process is unknown
  cannot be published with a clean conscience.”
- **Actual state of each output** (R-GGUF-20): **converted** (created by the
  converter), **placed** (the source GGUF already had that quant and was copied
  as is), **quantized** (passed through the quantizer), or **already existed**
  (not rebuilt). The sidecar and report name the state, and the checkpoint
  stores it so a resumed run continues to tell the truth: it reports what the
  phases did, not what the run planned to do.
- **GGUF header read using format constants:** magic `GGUF`, `u32` version (1,
  2, or 3), `n_tensors` and `n_kv` (`u64`), and key/value pairs using the type
  table from `gguf.h`, with `u32` lengths through v1 and `u64` from v2 onward.
  Reading is **bounded** (1 MiB) and returns `None` if the file is truncated,
  the version is invalid, or `general.file_type` is absent. The GGUF class comes
  from its header, not its filename: a file that says BF16 may contain IQ2_M.
- **Engine prerequisite, before downloading** (R-GGUF-56, T-29): without
  `llama.cpp` there is no conversion, quantization, or shard merge, and **not a
  byte is downloaded**. The converter (`convert_hf_to_gguf.py`) has **no
  substitute**; the quantizer and merger do have alternatives on `PATH`. The
  engine is searched under `$LLAMA_DIR` (the fork) and `$LLAMA_OFFICIAL_DIR`
  (official), and the error names the missing engine and the variable pointing
  to it. Quantization class feasibility is checked with **repository metadata
  only** (a few KB, R-GGUF-8), before downloading weights: if the GGUF is in a
  quant that cannot be requantized, the error suggests the unquantized sibling
  repository. Hard request validation is performed **before anything else**
  (R-GGUF-6): downloading 60 GB by mistake is just as easy as a usage error.

### Changed

- **`ropero-core` domain logic is pure:** it receives resolved data and does
  not open files, processes, or network connections; catalog scanning and
  machine capture move to `ropero-runtime`, the I/O layer. Separating decisions
  from I/O makes behavior testable without network or GPU, makes failures
  easier to locate, and keeps the CLI as a dispatcher.
- **The parser moves from hand-written code to `clap`'s builder**, and the flag
  contract lives in the domain (`ropero_core::surface`): adding a flag means
  adding one line to the domain and none to the CLI. `derive` was unsuitable: it
  required repeating each flag and could not express `ropero <model>` (a token
  that may be either command or model) without `external_subcommand`, which
  cannot parse flags.
- **CLI help lists commands that exist** and is **in Spanish**, including the
  headings that `clap` emits in English. It avoids advertising a surface that
  is not yet implemented.
- **`Limits::min_ram_gb` is `Option<f64>`**: the field is optional and is checked
  only when declared; a declared value must be greater than zero, as in v1.
- **`[files]` placeholder resolution uses one predicate:** it recognizes both
  declared `[files]` keys and available environment variables.
- **`ropero validate` also validates engine manifests**, and the v1 catalog
  (`ropero-code/models`) is **no longer validated**: it is reference only, and
  v2 is developed as if that directory did not exist. This is the decision in
  [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md). The three v1 files
  with both `[download]` **and** `[build]` (`llama/nemotron.toml`,
  `llama-official/nemotron-o.toml`, and `llama/fired/granite.toml`) violated a
  rule that was **removed**: both paths are a valid TOML state, and path choice
  is the contract of `ropero install models` (R-MOD-29), not the validator.
- **The default test suite no longer depends on `ropero-code/`:** golden-master
  TOMLs are frozen in the crate, and the two comparisons against real v1 are
  marked `#[ignore = …]` and run explicitly
  (`cargo test --locked --workspace -- --ignored`). The v2 catalog
  (`crates/ropero-core/tests/catalogo_real.rs`) always runs in the test suite
  and CI.
- **The redundant API and dead types are removed** from `ropero-core`. One API
  makes it harder to keep duplicate contracts during the extraction.

### Fixed

- **Retry port selection when an engine dies during `bind`:** choosing the
  lowest free port and starting is a **race** — two processes using the same
  range see and select the same port, and the loser died with “address already
  in use”, leaving startup failed, no disk state, and the port occupied by the
  winning engine. Ropero now tries the next free port in the range, with a loop
  **bounded** by range size, and does not retry when the user requested the
  port (`--port`); `--force` remains **before** the loop so a process that was
  stopped remains stopped.
- **A zombie engine no longer looks alive:** `kill(pid, 0)` treated an engine
  that died during `bind` and had not been reaped by its parent as alive, so
  startup could not distinguish a healthy engine from a dead one. `alive()` now
  checks the state reported by the kernel, and a dead engine is reaped as soon
  as it is observed, so retry does not leave zombies. The probe asks the engine
  **first** and accepts the response only if the engine is still alive
  (previously another process that acquired the port could answer); stop uses
  the same check, so a zombie counts as stopped instead of waiting for the
  deadline.
- **The CLI test's fake engine no longer survives as a grandchild:** its parent
  was Ropero, so when the test exited only Ropero was killed and the engine
  remained listening on the port range, where the next test — or next test-suite
  run — found it occupied. `Drop` now also stops the engine using the same
  verified (PID + `start_time`) stop method as `ropero stop`, avoiding killing a
  recycled PID.
- **Catalog paths are confined to the model directory** (path traversal):
  `Path::join` lets an absolute path replace its base and `..` ascend, so a
  `[download].file = "../../../secret.txt"` could make the R-MOD-16.2 `mv` move
  an outside file, and an outside `[download.aux.*].name` could overwrite it. A
  single helper (`ropero_download::ruta_dentro`) checks the path **before**
  joining: it rejects absolute roots and `..`, and requires the normalized path
  to start under the base. It applies to `download.file`, `download.name`,
  `aux.file`, `aux.name`, and `model.path`, and during execution before every
  `join`, `rename`, and `create_dir_all`. The catalog error (exit 1) names the
  field and received value.
- **The GGUF is downloaded where the runtime looks for it** (R-MOD-6): the
  destination used the `basename` of `[files].gguf`, so with
  `[files].gguf = "folder/my.gguf"` the file landed at `…/<model>/my.gguf`,
  while `ropero path <model>` pointed to `…/<model>/folder/my.gguf`. The model
  was unusable and the next install downloaded it again. The full relative path
  is now used and the subdirectory is created during download.
- **Menu selection is consent to download** (R-MOD-27): the `ropero install
  models` menu asked “Download X (~4 GB)? [y/N]?” and pressing Enter downloaded
  nothing (exit 0): selecting a model did not consent to its download. Selecting
  a model **does** consent, and the size confirmation is not repeated; the
  command always runs unattended.
- **The summary counts catalog TOMLs, not alias rows:** four TOMLs with
  `[model].alias` each create an extra row, and bare `ropero` said “available
  models: 49” for 45 TOMLs. `counts()` now counts **unique models by real name**
  —the alias row and its source TOML are the same model—so the counts in
  `ropero` and `ropero validate` agree. The listing is unchanged: alias rows
  remain because the alias responds just like the name.
- **The GGUF header is read using the format's canonical type table:** the
  reader used the wrong types (UINT32=6, STRING=12, ARRAY=13), so it could not
  find `general.file_type` in any real GGUF and always inferred the class from
  the filename. Fixtures are rebuilt **from the format, not the broken code**
  (v1, v2, v3, each value type's `file_type`, nested arrays, truncated file,
  and invalid version).
- **The remote imatrix hash reaches the sidecar:** it was downloaded into the
  work cache, and cleanup (phase 5) deleted the work **before** provenance
  (phase 6) was written. Thus sha256 was not written, and the sidecar named the
  file and repository but not **which** matrix was used. The hash is now
  calculated in phase 3 while the imatrix is present and stored in the
  checkpoint.
- **The sidecar names each output's actual state:** it said “the GGUF already
  existed: it was not quantized” for a GGUF the converter had just created in
  that same run. The second clause was true; the first was not.
- **The checkpoint records which request it belongs to:** it stored only the
  phase, so another request for the same source resumed with the wrong
  checkpoint and exited 0 while announcing a GGUF that had not been produced.
  A different fingerprint now discards it and restarts the run.
- **The listing finds published GGUFs:** it searched only top-level files, so
  GGUFs in model subdirectories were missing. It now searches the first level
  of subdirectories and the `<model>.gguf` artifact; quant is taken from the
  segment after the last `-` **only** if the filename belongs to that model, so
  listings do not mix models.
- **Digits are no longer lost from the source filename.**
- **Messages and errors say what to do** (P-13): an invalid `--quant` exits 2
  (usage error), like the rest of the surface, rather than 1; a report cannot
  show a GGUF missing from disk (new `Error::SalidaFaltante`); a **local**
  imatrix is checked and its error names the source; hints say “rebuild with
  `--force`” without deleting weights, “check that it can be written”, “check
  the disk and environment”, and “retry: the checkpoint resumes where it left
  off”. The v1 false message (“files are missing from clone: download is
  incomplete”) suggested deleting 60 GB for a problem unrelated to the
  download.
- **`hf` execution retries on `ETXTBSY`** (“Text file busy”): this was a
  millisecond race while another process had the file open for writing, causing
  `exec` to fail. With the retry, 60 consecutive runs succeed; previously 5%
  failed.
- **The plan does not repeat the quant suffix:** requesting NVFP4 from the
  `acme/model-NVFP4` repository used to create `model-NVFP4-NVFP4.gguf`.
- **The CLI test suite is hermetic:** it uses its own test catalog and does not
  depend on the real v1 catalog. Its results therefore do not change when a
  production TOML changes, and engine logs from tests go to a temporary
  directory.
- **The CLI contract has its own gate** (`cli_contract`) covering exit codes
  and output shape. `ropero <cmd> | head` exits 0: a broken pipe (SIGPIPE/EPIPE)
  is not confused with a real error.
- **An invalid `--prio` value is shown in full** (not truncated), `--prio` with
  `install engines` is a usage error, a model **with no paths** exits 2, and
  per-model errors go to **stderr** (stdout is for data, R-MOD-11).
- **There are no panics in `ropero-core`:** `unreachable!` in model resolution
  was a real error path, and sha256 is hashed **in chunks** (8 KiB blocks)
  without copying the entire file into memory.

### Known debt

- The version tree contains **653 `#[test]` functions, two marked
  `#[ignore = …]`**: the two golden-master comparisons against real v1, which
  run explicitly. The default test suite and CI are hermetic and do not depend
  on `ropero-code/`.
- **Four v1 checks are still unimplemented:** that `[args.level.<n>]` occurs in
  `[levels].list`, that `[args.flag.<f>]` and `[args.option.<o>]` are declared,
  and that `[download]` or `[build]` is present. Doc B §4.1.1 records them as
  debt and is the v2 contract, not the complete v1 contract.
- **The `context` multiple-of-32 rule appears only in doc B §4.1.1.** v1 accepts
  `133072`, which is not a multiple of 32, while v2 is stricter (the validator
  enforces it). It remains to be promoted to R-DATA in doc A or removed.
- **v2 applies `replace` where it occurred in the base; v1 applied it at the
  end.** The difference is covered by synthetic tests, but not by a real
  catalog case, and needs its own PR to reconcile behavior. In addition, if the
  replaced token is the last item in the base, v2 removes one token while v1
  removes two or one (`_drop_flag_pair`).
- The PDFs under `docs-especificacion/` are out of sync with the normative
  Markdown documents.
- **Phases F7 to F9 remain**: F7 is the OpenAI-compatible gateway (endpoint,
  auth, streaming, routing), F8 is installation and OpenCode integration, and
  F9 is v1/v2 coexistence, migration, and v1 removal.

## Unreleased

<!-- Entries added after 0.1.0 go here. -->

## How to read this alongside the plan

Version entries describe verifiable changes, not the details of outstanding
work. To find which task or phase closes each entry, use the `R-*`, `T-*`, and
`F-*` references in the build document. Phases **F0 to F6** are complete; **F7**
(the gateway) is next.

[Read this changelog in Spanish](CHANGELOG.es.md).
