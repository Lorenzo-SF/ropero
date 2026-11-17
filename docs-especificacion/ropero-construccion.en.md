---
title: "ropero — Build specification"
subtitle: "How to build, from scratch and autonomously, the local-model manager described by the requirements specification"
author: "Lorenzo-SF"
date: "2026-11-12"
lang: en
---

# ropero — Build specification

[Español: original](ropero-construccion.md) · [English companion](ropero-construccion.en.md)

> This is a **build** document, not a requirements document. Requirements—what
> the system must do, with exact semantics and evidence—are in
> [ropero-especificacion.en.md](ropero-especificacion.en.md) ([Spanish original](ropero-especificacion.md); 317 requirements, R-ARQ…R-GGUF). This document answers a different question: **what to build it with, how to organize it, in what order, and how to know when it is finished.**
>
> Read together, the two documents are enough for an AI to build all of Ropero
> without questions: one says *what*, the other *how*.

---

## 0. How to use this document

### 0.1 The two documents and the contract between them

| | `ropero-especificacion.md` | `ropero-construccion.md` (this document) |
|---|---|---|
| Question answered | What does Ropero do and with what semantics? | How is Ropero built? |
| Nature | Behavioral contract | Execution plan |
| Unit | Requirement (R-XXX-N) | Module, task, phase, acceptance criterion |
| Authority | **NORMATIVE** for behavior | **NORMATIVE** for construction |
| State | describes current code behavior, with `file:line` evidence | describes what must be written, with versions and signatures |

**Conflict rule.** If this document and the requirements document disagree about
*what the system must do*, **the requirements document prevails**. If they
disagree about *how to build it*, **this document prevails**. Behavior changes
cannot be introduced from here: if a task here implies changing behavior
described in the other document, the task is wrong and must be corrected, not
implemented.

**Completeness rule.** An AI with only this document cannot build Ropero: it
lacks the 317 requirements, exact vocabulary (`level`/`flag`/`option`,
`[args].base → level → flags → options`), eight engine-contract hooks, argv
assembly order, and 29 known debts. An AI with only the requirements document
cannot build it either: it does not specify language, versions, modules, tests,
or order. **Both are necessary; neither is sufficient.**

### 0.2 Conventions

- **Code language**: English (identifiers, types, `error!` messages).
  **Documentation and user-facing message language**: Spanish. This is the
  convention already established in the repository (`bin/ropero`, `install.sh`).
- **User error messages** are written in Spanish, without jargon, and always
  include **the action the user should take** (P-2: silent failure is worse
  than a visible error).
- **No network in the execution path.** Ropero commands access the network only
  to download models and install engines; both operations are explicit and
  observable. `install.sh` has hard rule R6, which remains in force.
- **No `sleep` for synchronization.** Wait for an event with a timeout (poll
  with backoff and deadline), never by blindly pausing.

### 0.3 Declared assumptions

This document was written without being able to ask the owner at the time. The
assumptions made are **explicit and reversible**, and each states its reversal
cost:

| # | Assumption | Why | Cost to reverse |
|---|---|---|---|
| S-1 | Goal is to **reimplement Ropero in Rust**, preserving doc A's observable behavior. | Earlier question: “if I wanted to create this application seriously, which language”; answer was Rust with phased extraction. | High: switching to Go/Python rewrites §§2, 3, and 9. |
| S-2 | **CLI does not change**: same commands, options, exit codes, and contractual stdout. | Doc A defines stdout contract (`ROPERO_PORT=…`) consumed by opencode plugin. Breaking it breaks ecosystem. | High. |
| S-3 | **Model TOML schema does not change**: add validation, do not change its shape. | Repo has 45 TOML files and `models/_template.toml` is executable format documentation. | Medium: schema migration touches 45 files. |
| S-4 | Preserve installation's **symlink-only thesis**: installer links into home, does not copy files. | Owner decision already written (R-INST), with hard rules R1–R13. | Medium: single binary contradicts R1 and requires rewriting `install.sh`. |
| S-5 | **Rewrite endpoint in Rust** (`ropero serve`), do not keep `endpoint.py`. | One binary and language; auth test that currently gives false green is rebuilt so false green is impossible. | Medium: keeping Python requires Python runtime on user's machine. |
| S-6 | Do **not reimplement external engines**: llama.cpp, MLX, TensorRT-LLM, vLLM, AirLLM, FastFlowLM, and Strata remain external processes. | Third-party software with hundreds of thousands of lines; Ropero *drives* them. | None (correct decision). |
| S-7 | Turn gguffer's **unchecked prerequisite** into an explicit requirement and guard that fails before downloading 60 GB. | It is the most costly measured gap (H-27). | None. |

**What is NOT assumed:** anything about business model, price, distribution
outside this repository, or support for more than two systems. If needed, this
document does not cover it and that must be stated.

### 0.4 Reference conventions

| Symbol | Meaning |
|---|---|
| `R-XXX-N` | Doc A requirement. **Every task in §9 cites the requirements it closes.** |
| `M-N` | System module, defined in §3. |
| `T-NN` | Build task, defined in §8. |
| `F-N` | Plan phase, defined in §8. |
| `P-N` | Design principle: twelve from doc A plus new ones in §5.2. |
| `H-N` | Known debt from doc A. Close it or document it; never silently inherit it. |
| 🔶 | Doc A requirement in target state (not implemented today). |

---

## 1. What is being built

### 1.1 The thesis in one sentence

**Ropero is a driver for local inference engines**: it reads models declared in
TOML, checks whether the machine can serve them, starts the appropriate engine
on a port it owns, waits until it is *actually* ready, and exposes one
OpenAI-compatible gateway. Everything else—downloading weights, creating
quantizations, deciding which engine to use—is instrumentation for that act.

### 1.2 Three facts respected by the entire design

These are not preferences; they are observations about the domain. Ignoring
them produces a system that appears to work but does not.

**Fact 1 — Starting an inference engine is slow and opaque.** There is no
reliable way to know whether `llama-server` is ready other than asking it.
Therefore *readiness* is a hierarchy of levels, not a boolean, and “wait one
second and retry” is a design flaw, not a simplification.

**Fact 2 — A model without its file is not a model.** Half of the failures users
see in model managers are really files missing from where the system expects,
partially downloaded `.gguf` files, or checkpoints quantized to a different
precision than requested. Therefore a model is **data** (P-6), and artifact
*provenance* is not optional (R-GGUF-18).

**Fact 3 — The user's machine is not a clean environment.** Another program
may own port 9999; an engine may have been started manually; another process
may occupy the GPU; or llama.cpp may be cloned somewhere unexpected. A model
manager that cannot distinguish “my engine” from “an engine” may kill another
user's process. Hence P-3 and process identity `pid + start_time`.

### 1.3 What this document is NOT

- **Not a Rust tutorial.** It does not explain enums or `async fn`; it assumes
  the implementer knows Rust.
- **Not the requirements specification.** It does not repeat 317 requirements;
  it cites them. Duplicating 140 KB of semantics is the quickest way for two
  copies to diverge.
- **Not a business plan.** It does not decide pricing, commercial packaging,
  or distribution channels beyond the existing `install.sh`.

---

## 2. Stack and versions

### 2.1 Principle

**Fix exact stack versions, record them, and update them with an explanation.**
An unpinned dependency accrues compound-interest debt: when crate X changes its
API, the user's machine—not CI—gets a broken build.

Operational consequences:

- Commit `Cargo.lock` (this is an application, not a library).
- Pin compiler version in `rust-toolchain.toml`.
- Every new dependency goes through §6.8 (ADR) before inclusion.
- Update dependencies in separate commits so a build failure can be attributed
  to the update, not a logic change.

### 2.2 Toolchain

| Item | Version | Pinned in | Note |
|---|---|---|---|
| **Rust (stable)** | **1.100.0** (2026-11-09) | `rust-toolchain.toml` | Declared MSRV. Checked against `static.rust-lang.org/dist/channel-rust-stable.toml` on 2026-11-12. |
| Edition | **2024** | `Cargo.toml` | `edition = "2024"`, `rust-version = "1.100"` in each crate. |
| `release` profile | `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"` in CLI binary | `Cargo.toml` | `panic = "abort"` **only** in binary: server needs unwinding to avoid killing whole process. |
| Main target | `x86_64-unknown-linux-gnu` (CachyOS) | CI | |
| Release targets | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin` | `.github/workflows/release.yml` | macOS arm64 is mandatory: owner's reference hardware is a Mac with Metal. |
| Binary format | static with `musl` on Linux | CI | GNU target depends on glibc; musl does not. Static binary eliminates “won't start on your machine” class of failures. |
| Support-script shell | `bash 3.2` (macOS version) | `shellcheck` + CI | **Inherited hard constraint**: no `readlink -f`, `mapfile`, `declare -A`, nameref, `${var^^}`. Installer rule R10. |

### 2.3 Production dependencies

Versions checked against crates.io index on **2026-11-12**. This document
specifies the *minimum accepted version*; `Cargo.lock` pins the exact build.

**Count: 33 production dependencies** (§§2.3.1–2.3.5) and **9 development
tools** (§2.3.6, not included in binary). Seven forbidden dependencies in
§2.3.7 are not installed.

#### 2.3.1 Domain core

| Crate | Version | Purpose | Why this one |
|---|---|---|---|
| `serde` | 1.0.229 | Derive `Serialize`/`Deserialize` for domain types | De facto standard; `toml` and `axum` already depend on it, adding no tree weight. |
| `toml` | 1.2.1 | Parse model and engine-manifest TOML | Preserves key order and reports line/column on errors. |
| `thiserror` | 2.0.21 | Derive `Error` for domain variants | Typed, exhaustive domain errors. |
| `anyhow` | 1.0.104 | Context at boundaries (CLI, binary) | Boundary only: domain returns concrete types. |
| `schemars` | 1.2.3 | Generate model TOML JSON Schema | Publishable: `ropero schema --json` feeds editors and external validator. |
| `camino` | 1.2.6 | UTF-8 paths instead of `PathBuf` | Model/file names may be non-ASCII; Unix `Path` does not guarantee this and can break. |
| `directories` | 6.0.0 | Resolve `~/.local/bin`, `~/.config`, cache | Respects XDG on Linux and correct paths on macOS. |

#### 2.3.2 CLI

| Crate | Version | Purpose |
|---|---|---|
| `clap` | 4.6.7 | Argument parser, with derive and subcommands |
| `clap_complete` | 4.6.11 | Generate shell completions (`ropero completion bash`) |
| `shell-words` | 1.1.1 | Split command line into argv without invoking shell |
| `is-terminal` (via `std::io::IsTerminal`) | std | Detect TTY; without TTY, no interactive menu (R-CLI-16) |

#### 2.3.3 Runtime: processes, ports, system

| Crate | Version | Purpose |
|---|---|---|
| `tokio` | 1.54.1 | Async runtime: timeouts, cancellation, `select!` |
| `nix` | 0.31.3 | `fork`/`setsid`/signals/`waitpid`/`prctl`—process identity and group termination |
| `libc` | 0.2.191 | System calls not exposed by `nix` |
| `sysinfo` | 0.40.3 | Process/system CPU/RAM/GPU metrics (`ropero stats`) |
| `fs4` | 1.1.0 | Inter-process file lock (port lock) with expiry |
| `signal-hook` | 0.4.6 | Safe (`async-signal-safe`) `SIGINT`/`SIGTERM` handlers |
| `which` | 8.0.6 | Find binaries on `PATH` |
| `rustix` | 1.1.6 | `mount`/`statvfs`/disk space: “will the GGUF fit?” before download |
| `notify` | 8.2.2 | Watch model directory changes (optional, for `ropero watch`) |

#### 2.3.4 Network and server

| Crate | Version | Purpose |
|---|---|---|
| `axum` | 0.8.9 | OpenAI-compatible endpoint HTTP server |
| `hyper` | 1.12.0 | HTTP runtime (via axum) |
| `hyper-util` | 0.1.21 | `hyper` utilities (server, upgrades) |
| `http-body-util` | 0.1.5 | HTTP bodies as `Body` for streaming |
| `tower` | 0.5.3 | Middleware (auth, CORS, rate limiting, traces) |
| `tower-http` | 0.7.1 | CORS, body limit, compression |
| `reqwest` | 0.13.5 | HTTP client: retry/resume downloads, proxy to engine |
| `futures-util` | 0.3.34 | `StreamExt` for bidirectional SSE streaming |
| `bytes` | 1.12.1 | Buffers efficiently |
| `parking_lot` | 0.13.0 | Non-poisoning mutexes (port registry) |

#### 2.3.5 Tracing and errors

| Crate | Version | Purpose |
|---|---|---|
| `tracing` | 0.1.44 | Structured instrumentation |
| `tracing-subscriber` | 0.3.23 | Wrapper, json/pretty formatting, `ROPERO_LOG` filter |
| `tracing-error` | 0.2.1 | Convert `anyhow` errors into spans with cause |
| `miette` | 7.6.0 | Diagnostic error presentation (code, source, help) |

> **`miette` vs `thiserror`:** `thiserror` defines domain error types;
> `miette` presents them. Domain errors do not depend on `miette` (they can be
> tested without comparing text), and CLI formatting cannot be tested in a
> unit test. This separation avoids tests that assert on a *string*.

#### 2.3.6 Development tools (not in binary)

| Tool | Version | Use |
|---|---|---|
| `cargo-nextest` | 0.9.151 | Test runner: process-level parallelism, `--retries`, profiles |
| `cargo-llvm-cov` | 0.9.1 | LLVM coverage with lcov report |
| `cargo-deny` | 0.20.2 | Licenses, advisories, bans, duplicate sources |
| `cargo-binstall` | 1.27.0 | Install prebuilt binaries (saves CI minutes) |
| `cargo-edit` | 0.13.14 | Controlled `Cargo.toml` edits via `cargo add/rm/upgrade` |
| `cargo-sort` | 2.1.4 | Sort `Cargo.toml` dependencies (reduces diff noise) |
| `taplo` | 0.14.0 | Format model TOML and engine manifests |
| `shellcheck` | 0.10.0 | Support scripts that remain Bash |
| `rustfmt` | with toolchain | `cargo fmt --check` |

#### 2.3.7 **Forbidden** dependencies (and why)

| Crate | Reason |
|---|---|
| `openssl` / `native-tls` | HTTP client must work without system TLS stack; `reqwest` with `rustls` handles it. |
| `chrono` | `std::time::SystemTime` covers use case; timestamps use epoch and manually formatted RFC-3330. A timezone-aware date dependency is a bug waiting to happen. |
| `lazy_static` / `once_cell` | `std::sync::OnceLock` (stable since 1.70) covers the case. |
| `dotenv` | Process inherits environment; there is no `.env` file to read. |
| `indicatif` | Use `hf` progress bars; a second bar duplicates noise. |
| `reqwest` with excess default features (brings Tokio and OpenSSL) | Set `default-features = false` and list only used features: `rustls-tls`, `json`, `stream`. |

### 2.4 Workspace structure

One distributable binary and four internal libraries with strictly descending
dependencies. **The graph must be acyclic**, checked by architecture test
(§8, T-01).

```
ropero/
├── Cargo.toml                 # workspace: members, release profile, lints
├── Cargo.lock                 # versioned (application, not library)
├── rust-toolchain.toml        # channel + components (rustfmt, clippy)
├── rustfmt.toml               # edition = "2024"
├── clippy.toml                # additional lints
├── deny.toml                  # cargo-deny: licenses, bans, advisories
├── README.md
├── CHANGELOG.md
├── crates/
│   ├── ropero-core/           # M-01 · pure domain. NO process or network I/O.
│   ├── ropero-runtime/        # M-02 · processes, ports, on-disk state.
│   ├── ropero-download/       # M-03 · downloads, verification, resume.
│   ├── ropero-forge/          # M-04 · GGUF creation (gguffer).
│   ├── ropero-server/         # M-05 · OpenAI-compatible endpoint.
│   └── ropero-cli/            # M-06 · `ropero` binary. Wiring/presentation only.
├── models/                    # model TOML (data, not code)
├── engines/                   # engine manifests (data, not code)
├── test/                      # integration test harnesses
├── scripts/                   # remaining Bash, justified in §6.9
├── opencode-plugin/           # opencode TypeScript plugin, outside Rust workspace
└── docs/
```

**`ropero-code/` (v1) is deliberately absent**: it is **reference, not a
dependency** ([ADR-003](../docs/adr/ADR-003-ropero-code-es-referencia.md)). All
v2 development proceeds as if that directory did not exist; consult it to
learn, compare, and contrast, never as source of truth.

**Current contract (working tree, catalog/config):** in normal use, catalog is
**external** at `<config_dir>/models/<engine>/`, manifests at
`<config_dir>/engines/` (`$XDG_CONFIG_HOME/ropero` or `~/.config/ropero`).
Bootstrap idempotently creates directory and default `config.toml` without
overwriting. Precedence: `environment variable > config.toml > default` for
`models_dir`, `gguf_dir`, `safetensors_dir`, `bin_dir`, `logs_dir`. Preserve
`ROPERO_MODELS_DIR` and `ROPERO_ROOT` overrides (including `<root>/engines/`
lookup when pointing to `<root>/models/`). Repository TOML/manifests are for
development/tests and are **not auto-discovered** in normal use; workspace
also has its own `models/` development reference catalog (not integrated into
executable). See [ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md).

**Dependency rule** (verified by test, §9 T-01):

```
ropero-cli ──► ropero-core ──► (nothing)
     │             ▲
     ├──► ropero-runtime ──┘
     ├──► ropero-download ─┘
     ├──► ropero-forge ────┘
     └──► ropero-server ───┘
```

- `ropero-core` depends on **nothing** in the project: not `tokio`, `reqwest`,
  or `nix`. It is pure: types, rules, state transitions. This makes 80% of
  logic testable in milliseconds without real processes.
- `ropero-runtime`, `-download`, `-forge`, and `-server` depend only on
  `ropero-core` among project crates.
- `ropero-cli` depends on all crates and is the only one that prints to screen.

### 2.5 What the project does NOT use (and why)

| Not used | Reason |
|---|---|
| `async-trait` | Async traits are stable in Rust 2024 without macro. One fewer dependency. |
| `serde_yaml` | Data format is TOML. YAML adds type ambiguity and a dependency. |
| `clap` derive vs builder | Use derive. Reserve builder for non-derivable subcommand structure; if needed, that signals too many variants. |
| ORM or database | State is a directory of files. Database would be more infrastructure than data. |
| CLI framework such as `ratatui` | Ropero is a pipe-oriented CLI, not TUI. Output goes to log or pipe. |
| `openssl` | Covered in §2.3.7. |

---

## 3. Architecture

### 3.1 The four layers and why they are separate

```
┌────────────────────────────────────────────────────────────────┐
│  M-06 ropero-cli          Presentation: parsing, output, codes │
│  (`ropero` binary)        exit codes, TTY or not, localization  │
└───────────────┬────────────────────────────────────────────────┘
                │ uses
┌───────────────▼────────────────────────────────────────────────┐
│  M-05 ropero-server      HTTP: proxy, auth, SSE streaming      │
│  M-04 ropero-forge       Creation: phases, checkpoints         │
│  M-03 ropero-download    Download: retries, verification       │
│  M-02 ropero-runtime     Processes: spawn, probe, stop, state  │
└───────────────┬────────────────────────────────────────────────┘
                │ depends on
┌───────────────▼────────────────────────────────────────────────┐
│  M-01 ropero-core         Pure domain: types, rules,           │
│  (no I/O)                 state transitions, errors            │
└────────────────────────────────────────────────────────────────┘
```

**The rule defining the boundary:** `ropero-core` performs no I/O. “There is
no `std::process::Command` in `ropero-core`” is checkable with `grep` in CI, so
CI will check it (§8, T-01).

Benefits of each boundary:

- **Domain separated from I/O** ⇒ logic is tested without processes, network,
  or disk. Domain test takes microseconds; equivalent integration test takes
  seconds. Full suite must run in seconds, or it will not run.
- **One engine per crate** ⇒ gguffer work (currently 2,338 Bash lines in
  `cmd-gguffer.sh`) does not contend for lock with model startup.
- **CLI above everything** ⇒ server, forge, downloader can be used as
  libraries from another program without CLI.

### 3.2 M-01 · `ropero-core` — domain

Contains **everything that can be decided without touching the outside world**.

#### 3.2.1 Model types

```rust
// crates/ropero-core/src/model.rs

/// A catalog model. It is DATA: nothing is executed here (P-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub model: ModelMeta,
    pub files: Files,
    pub download: Option<Download>,
    pub build: Option<Build>,
    pub levels: Option<Levels>,
    pub flags: Option<Flags>,
    pub options: Option<Options>,
    pub aliases: BTreeMap<String, String>,
    pub limits: Option<Limits>,
    pub args: Args,
    /// Source TOML path, retained for errors: validation error must say WHICH FILE.
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMeta {
    pub name: String,          // kebab-case, globally unique
    pub engine: EngineId,      // canonical ID (see §4.2)
    pub context: u32,
    pub alias: Option<String>,
    pub port: Option<u16>,     // pinned port
    pub format: ModelFormat,   // Gguf | Safetensors | Q4nx | Strata
    pub path: Option<String>,  // required for safetensors, forbidden for gguf
}
```

**`source` is intentional.** Current validator (`modelconf.py`) reports
`model X: field Y`, not `model X (file Z)`: line 3. In a 45-model catalog,
missing path requires manual `grep`. With `source`, error says
`models/llama/groot.toml:12: [model].format = "safetensors" requires
[model].path`. That is the difference between actionable and interpretive
errors.

#### 3.2.2 Vocabulary: level, flag, option

Doc A fixes this vocabulary (R-CLI) as part of the **public contract**:

| Concept | What it is | Declared in | CLI form |
|---|---|---|---|
| **level** | Mutually exclusive reasoning mode. | `[levels].list` | `--<level>` |
| **flag** | Accumulable boolean switch. | `[flags].list` | `--<flag>` |
| **option** | Value; repeatable, last one wins. | `[options].list` | `--<option> <value>` |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgSelector {
    Level(Level),
    Flag(Flag),
    Option(OptionKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Level {
    Named(String),   // "high"
    Alias(String),   // "x" → resolves to a Named
}
```

**A level alias is a level.** `[aliases] x = "high"` means `--x` is exactly
`--high`, with same argv and consequences; it is not a different shortcut.
Treating it differently is a latent bug: if alias points to a flag instead of
a level, “undeclared level → exit 2” validation no longer applies.

#### 3.2.3 Assembling argv

Order is **fixed and normative** (R-DATA-15, `models/_template.toml`):

```
[args].base  →  [args.level.<level>]  →  [args.flag.<f>]  →  [args.option.<o>]
```

```rust
// crates/ropero-core/src/args.rs

/// Assemble final engine argv. PURE: no disk reads, environment queries, or
/// process launches. All required inputs are passed as parameters.
pub fn build_argv(
    spec: &ModelSpec,
    request: &LaunchRequest,
    ctx: &ResolveContext,
) -> Result<Vec<String>, ArgvError>;

pub struct LaunchRequest {
    pub level: Option<String>,
    pub flags: Vec<String>,     // in user-requested order
    pub options: Vec<(String, String)>,
}

pub struct ResolveContext {
    /// Effective context: [model].context, or ROPERO_<NAME>_CTX override.
    pub context: u32,
    /// Already-resolved, existing paths (gguf, mtp, mmproj, chat_template).
    pub files: ResolvedFiles,
    /// Captured environment variables.
    pub env: BTreeMap<String, String>,
}
```

**Flag order is the user's requested order**, not TOML order. Observable and
tested (R-DATA-16).

**`replace` replaces in base.** `[args.option.temp] base = ["--temp",
"${value}"]` with `replace = "--temp"` removes the base `--temp 0.7` pair and
puts `--temp <value>` in its place. Otherwise argv would contain
`--temp 0.7 … --temp 0.9`; engine choosing the last would work by accident,
not design, and fail if order changed.

**`requires` is a disk precondition.** `[args.flag.no-vision] requires =
["mmproj"]` ⇒ if `mmproj` is absent, `--no-vision` is a **usage error** (exit
2), not a warning. User requested something impossible; clearly saying so is
the only honest response.

#### 3.2.4 Placeholders

Nine placeholders are resolved in two phases (text substitution, then path
resolution). Exact names are in doc A §4.2.3; governing rule here:

> **An unresolved placeholder is an error, never an empty string.**
> `${gguf}` without a file ⇒ `ArgvError::MissingFile`. Never `"--model" ""`:
> engine would interpret that as an empty model and fail 30 seconds later with
> a useless message.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgvError {
    UnknownLevel { level: String, declared: Vec<String> },
    UnknownFlag  { flag: String, declared: Vec<String> },
    UnknownOption{ option: String, declared: Vec<String> },
    MissingFile  { placeholder: &'static str, key: String },
    MissingEnv   { var: String },
    NotDeclared  { selector: String },
}
```

Each variant carries the **declared list**, so message can say “--alto is not a
level; levels are: low, medium, high”.

#### 3.2.5 Runtime state machine

Managed-model state is a value, not a collection of loose files (R-EST). The
state machine is explicit:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    /// No process. Initial and final state.
    Stopped,
    /// Process exists but does not yet respond.
    Starting,
    /// Responding. PID, port, and stamp are present.
    Ready,
    /// Process died during startup.
    Failed,
    /// Stop requested; waiting.
    Stopping,
    /// Process alive but health lost.
    Degraded,
}
```

Allowed transitions—**any other transition is a programming bug**, exhaustively
checked by §8 T-02 test:

```
Stopped  → Starting        (start)
Starting → Ready           (probe OK)
Starting → Failed          (probe failed / process dead)
Starting → Stopping        (cancellation)
Ready    → Stopping        (stop)
Ready    → Degraded        (subsequent probe fails)
Degraded → Ready           (probe returns OK)
Degraded → Stopping        (stop)
Failed   → Starting        (retry)
Stopping → Stopped         (process reaped)
Stopping → Failed          (won't die: must kill it)
```

`Failed` and `Degraded` are deliberately distinct: `Failed` means “never
started” (nothing to ask); `Degraded` means “started and stopped working” (must
be stopped). Confusing them causes “Ropero says model is alive when it isn't”.

#### 3.2.6 Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("unknown model: {name}")]
    UnknownModel { name: String, known: Vec<String> },

    #[error("{path}:{line}: {message}")]
    Config { path: PathBuf, line: usize, message: String },

    #[error("engine {engine} is incompatible with this machine: {reason}")]
    IncompatibleEngine { engine: EngineId, reason: String },

    #[error("port {port} is occupied by a foreign process (pid {pid})")]
    PortTakenByForeign { port: u16, pid: u32 },

    #[error("invalid state transition: {from} → {to}")]
    IllegalTransition { from: RuntimeState, to: RuntimeState },
}
```

**Rule:** a `CoreError` contains **no presentation text**. CLI layer composes
Spanish text and tips. Unit tests can check
`matches!(err, CoreError::UnknownModel { .. })` instead of strings, and
messages can change without breaking tests.

### 3.3 M-02 · `ropero-runtime` — processes, ports, and state

#### 3.3.1 Engine contract in Rust

Doc A defines eight Bash hooks (R-ENG-1). In Rust, an engine is **a data
manifest** (§4.3) plus, optionally, a module with custom logic. Hooks now
implemented as functions become **manifest fields**:

```rust
// crates/ropero-core/src/engine.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineManifest {
    pub id: EngineId,
    pub label: String,
    /// Compatibility conditions (OS, hardware). See §4.3.
    pub compat: Vec<CompatRule>,
    /// External commands this engine depends on.
    pub deps: Vec<String>,
    /// Installation method. `Manual` means Ropero does not install it.
    pub install: InstallSpec,
    /// Default engine port.
    pub port: u16,
    /// Whether engine speaks OpenAI protocol natively.
    pub native_auth: bool,
    /// Command construction: `Template` for 90%; `Custom` Rust module for Strata and FLM.
    pub launch: LaunchSpec,
    /// Declared maturity state (R-CTX-3).
    pub maturity: Maturity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maturity {
    /// Tested on owner's machine.
    Proven,
    /// Integrated but never launched.
    Integrated,
    /// Partially integrated.
    Partial,
}
```

**`Maturity` is a requirement, not a comment.** Doc A measures six engines
tested and two untested. A manifest field lets `ropero install engines` display
the distinction and `ropero doctor` warn, turning R-CTX-3 into a verifiable
mechanism rather than a sentence in docs.

#### 3.3.2 Compatibility

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompatRule {
    Os { any: Vec<Os> },
    /// Requires NVIDIA GPU with `nvidia-smi` present.
    NvidiaSmi,
    /// Requires Apple Silicon with Metal.
    Metal,
    /// Requires AMD XDNA NPU.
    AmdXnpu,
    /// Requires engine already installed.
    Installed,
    /// Requires minimum RAM (GB).
    MinRam { gb: u32 },
}
```

Evaluate **all** rules and accumulate them: incompatible engine explains
**every** reason, not just first. A Linux user installing TensorRT-LLM needs
to know they lack *both* NVIDIA and 32 GB RAM, not only one.

#### 3.3.3 Process identity: `pid + start_time`

This is what prevents killing foreign processes (P-3).

```rust
// crates/ropero-runtime/src/process.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcIdentity {
    pub pid: u32,
    /// Process start time in epoch seconds. Linux: field 22 in
    /// /proc/<pid>/stat. macOS: `start time` from sysctl KERN_PROC_PID.
    pub start_time: u64,
    /// Executed binary hash, if resolved.
    pub exe_digest: Option<[u8; 32]>,
}
```

**PID alone is insufficient**: PIDs are recycled. A port guard checking only
PID can, in the worst case, kill a foreign process that inherited that number.
`start_time` makes identity real. `exe_digest` (binary SHA-256) is a third
layer: even if PID and start time match—which is practically impossible—the
binary must be ours.

**Check order when stopping** (R-EST-8):

```
1. Is anything listening on port?       → if not, nothing to stop.
2. Is state PID the port's PID?          → if not, foreign. Do NOT touch.
3. Does start_time match?                → if not, another process. Do NOT touch.
4. Does exe_digest match?                → if not, another program. Do NOT touch.
5. Only now: SIGTERM process group.
6. Wait until deadline.
7. SIGKILL process group.
```

Checks 2–4 separate “Ropero stops what it started” from “Ropero kills what it
finds”.

#### 3.3.4 On-disk state

**Disk state is source of truth** (P-4): after a process restart, Ropero must
know what had been running.

```
$STATE_DIR/                       # default ~/.local/state/ropero
├── ports/
│   └── 9999.state                # one file per managed port
├── locks/
│   └── 9999.lock                 # exclusive lock, PID and timestamp
└── logs/
    └── 9999.log                  # engine log
```

`ports/9999.state` format is **`KEY=value` lines**, as required by doc A
R-ARQ-9. Doc A governs observable behavior; state-file format is observable
(on disk, readable by user and tools) and **must not change**.

```ini
MODEL=groot
ENGINE=llama-server
PORT=9999
PID=42137
STARTTIME=812345678
EXEDIGEST=3f2a9c...
LEVEL=high
STAMP=a1b2c3d4e5f6...
TS=2026-11-12T18:45:12Z
LOG=/home/user/.local/state/ropero/logs/9999.log
VERSION=1
```

Three new keys (`STARTTIME`, `EXEDIGEST`, `VERSION`) are **compatible
additions**: all eight doc A keys remain with same names and meanings, so a v1
reader still understands the file. Writes remain **atomic** (temporary file +
`rename`) and **preserve** keys not supplied in the call (R-ARQ-9).

`stamp` is the seal (R-RUN-8): hash of model, level, context, and files. If
state stamp **differs** from stamp produced by current config, model is
**stale** and must restart. Thus changing `[model].context` cannot leave a
server with old context. Comparison is string equality; nothing is “recomputed”.

#### 3.3.5 Port lock

POSIX `mkdir` is atomic. Lock is directory `locks/9999.lock` with PID inside,
and **expires** if owning Ropero process dies (R-RUN-17): process finding lock
checks PID; if it no longer exists, lock is stale and removed.

```
1. mkdir locks/9999.lock        → EEXIST means someone already holds it.
2. Write PID inside.
3. Check PID still alive (read lock, verify, write lock).
   If not → another process took it meanwhile → retry (max 3).
```

#### 3.3.6 Readiness probe

Doc A defines a hierarchy of probe levels. In Rust:

```rust
pub enum Probe {
    /// Port accepts TCP connections.
    Tcp,
    /// Engine health endpoint responds 200.
    Http { path: &'static str },
    /// Engine responds to minimal inference.
    Inference { prompt: &'static str },
}
pub struct ProbePlan {
    /// Test in order; first passing level is enough.
    pub levels: Vec<Probe>,
    /// Timeout per level.
    pub timeout: Duration,
    /// Delay between attempts.
    pub backoff: Duration,
}
```

**Inference probe alone proves the model is *ready*.** TCP only proves a
socket exists; HTTP only proves web server responds. For 60 GB model,
difference between “port open” and “model responds” is difference between
“Ropero works” and “Ropero says it works”.

### 3.4 M-03 · `ropero-download` — downloads

Doc A fixes exact sequence (R-MOD-16), with `hf` as sole downloader (R-MOD-18).
This module **wraps `hf` with guarantees it lacks today**:

```rust
pub struct DownloadRequest {
    pub repo: String,          // "owner/repo"
    pub file: String,
    pub dest: PathBuf,         // final path
    pub size_hint: Option<String>,
}
pub enum DownloadOutcome {
    /// Already existed and is non-empty.
    AlreadyPresent,
    /// Downloaded now.
    Fetched { bytes: u64 },
    /// Resumed from a partial file.
    Resumed { bytes: u64 },
}
```

**An empty file does not count as downloaded.** Ignoring this may cost a 40 GB
download: `hf` leaves a zero-byte file when it fails, and existence-only check
would accept it.

### 3.5 M-04 · `ropero-forge` — GGUF creation

Largest module and one with most debt. Split into clear submodules:

```
ropero-forge/
├── src/
│   ├── lib.rs
│   ├── pipeline.rs     # six phases as a state machine
│   ├── phases.rs       # each phase: precondition, action, postcondition
│   ├── checkpoint.rs   # five checkpoint classes (R-GGUF-32)
│   ├── imatrix.rs      # importance matrix
│   ├── mtp.rs          # MTP head: block not covered by imatrix
│   ├── engine.rs       # PREREQUISITE: llama.cpp engine (S-7, H-27)
│   ├── gguf.rs         # GGUF header reader (replaces ggufcheck.py)
│   └── provenance.rs   # provenance sidecar makes artifact auditable
```

#### 3.5.1 Engine prerequisite

This is the highest-value fix in the document. Today (R-GGUF-55), conversion
fails without engine and classifier says *“missing files in clone: download
incomplete”*—a **false diagnosis** prompting deletion and another 60 GB fetch.

```rust
// crates/ropero-forge/src/engine.rs
/// Check gguffer prerequisite BEFORE downloading anything.
/// Return correct diagnosis, not "download incomplete".
pub fn ensure_toolchain(spec: &ToolchainSpec) -> Result<Toolchain, ToolchainError>;

#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
    /// No llama.cpp engine installed.
    #[error("gguffer requires a llama.cpp engine (official or your fork).\n\
             None is installed.\n\
             Install it with: ropero install engines llama-server")]
    NoEngineInstalled,
    /// Engine exists, but in a directory gguffer does not inspect.
    #[error("llama.cpp engine found at {found}, but gguffer searches {expected}.\n\
             Install engine with `ropero install engines llama-server` \
             (or export LLAMA_DIR={found})")]
    EngineInWrongPath { found: PathBuf, expected: PathBuf },
    /// Engine exists, but one tool is missing.
    #[error("engine at {root} lacks {tool} (needed for {purpose}).\n\
             Rebuild engine: ropero install engines llama-server --force")]
    MissingTool { root: PathBuf, tool: &'static str, purpose: &'static str },
}
```

**Every variant includes an action.** This applies P-2 to diagnostics: say
what to do, not only what failed.

Most importantly, **check runs before phase 1 (download)**. In current order,
failure happens after downloading 60 GB.

#### 3.5.2 Six phases

Doc A defines phases (R-GGUF-16…21). Each is a function with explicit
precondition, action, and postcondition:

```rust
pub enum Phase {
    Resolve,     // 1 · resolve repo and files
    Download,    // 2 · fetch weights (or source GGUF)
    Convert,     // 3 · convert to intermediate GGUF
    Imatrix,     // 4 · calculate importance matrix
    Quantize,    // 5 · quantize to target
    Verify,      // 6 · verify result and write provenance
}
```

**Each phase writes a checkpoint** before starting and marks it complete after
(R-GGUF-32). Failure in phase 5 (expensive) **deletes nothing**: clone and
intermediate stay, and error says exactly how to retry only quantization.
Losing 60 GB due to quantization failure is forbidden by R-GGUF-16 and current
code violates it.

#### 3.5.3 Checkpoint taxonomy

Five classes (R-GGUF-32..36). Each checkpoint has:

```rust
pub struct Checkpoint {
    pub phase: Phase,
    /// State within phase: resume location.
    pub state: CheckpointState,
    /// Data needed for phase to resume itself.
    pub resume: ResumeData,
    pub ts: SystemTime,
}
```

Key distinction: **non-verifiable** checkpoint (cannot be checked without
repeating work) differs from **verifiable** one (cheaply checked and resumed).
Intermediate GGUF is verified by its header; imatrix by size and format.

### 3.6 M-05 · `ropero-server` — the single gateway

Replaces `bin/server/endpoint.py` (1,598 lines) and auth test that **gives
false green** (R-TST-19, wound #1).

```rust
// crates/ropero-server/src/lib.rs
pub struct ServerConfig {
    pub bind: SocketAddr,
    /// API key. Never in engine argv (P-8).
    pub api_key: Option<String>,
    /// Destination for each request.
    pub router: Arc<dyn Router>,
    pub upstream: UpstreamConfig,
}
/// Start server. Returns when ready to accept.
pub async fn serve(cfg: ServerConfig, shutdown: Shutdown) -> anyhow::Result<()>;
```

**Fix for wound #1 is structural, not cosmetic.** Current test does:

```bash
rc=$?          # ← Python block rc captured here
...            # ← and NEVER checked again
exit 0         # ← unreachable, but if reached, test passes
```

Rust equivalent is structurally different: test calls the **same function** as
server for authentication and asserts on **result**, not a subprocess exit code:

```rust
#[tokio::test]
async fn rejects_request_without_key() {
    let app = build_app(test_server()).await;
    let res = app.oneshot(Request::get("/v1/models").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_request_with_wrong_key() {
    let app = build_app(test_server()).await;
    let res = app.oneshot(
        Request::get("/v1/models")
            .header("authorization", "Bearer incorrect")
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

These tests cannot pass with broken auth: if `build_app` stops checking key,
both fail. This tests **behavior** rather than **a process's exit code**.

**API key never goes in engine argv** (P-8; measured wound: any same-host user
can read it from `/proc/<pid>/cmdline`). In Rust pass it through **environment**
or **fd**:

```
--api-key-file /run/user/1000/ropero/key   (mode 0600)
```

Environment has known limitation (`/proc/<pid>/environ` is readable by same
user), but is strictly better than argv, readable by **everyone**. Mode-0600
file is the only complete solution.

### 3.7 M-06 · `ropero-cli` — presentation

```rust
// crates/ropero-cli/src/main.rs
#[derive(Parser)]
#[command(name = "ropero", version, about = "local model manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve a model (shortcut for `use`).
    Use { model: String, #[command(flatten)] opts: RunOpts },
    Stop { target: Option<String> },
    Status { model: Option<String> },
    List { #[arg(long)] all: bool },
    Template,
    Args { model: String },
    Levels { model: String },
    Path { model: String },
    Install { #[command(subcommand)] action: InstallAction },
    Engines,
    Models,
    Doctor,
    Check,
    Config { engine: Option<String> },
    Gguffer { #[command(subcommand)] action: GgufferAction },
    Serve { #[arg(long)] port: Option<u16> },
    Stats { #[arg(long)] watch: bool },
    Test { model: String },
    /// Development only: emit model TOML JSON Schema.
    Schema,
    /// Shell completion.
    Completion { shell: Shell },
}
```

**CLI rule:** logic does not live here. Business logic in `main.rs` match is an
architecture bug; CLI tests check **exit code and stdout**, not logic.

**Exit codes are contractual** (R-CLI-4):

| Code | Meaning |
|---|---|
| 0 | OK |
| 1 | Execution error |
| 2 | Usage error (arguments, undeclared level, unknown option) |
| 130 | Interrupted by Ctrl-C |

The opencode plugin and user scripts depend on this table.

---

## 4. Data contracts

A data contract is a format two programs use to understand one another.
Ropero has five, and **all five are versioned**: an incompatible change
requires migration, which must be explicit.

### 4.1 Model TOML (`models/<engine>/<name>.toml`)

**Stabilized.** Schema is doc A §4.2.1 and `models/_template.toml`. This
document does **not change it** (S-3); it turns it into a validated Rust type.

```rust
// crates/ropero-core/src/model.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]     // ← decision that enables real validation
pub struct ModelSpec { /* … */ }
```

**`deny_unknown_fields` is the format's most important design decision.**
Today an unknown key in `[build]` generates a **warning** (per
`models/_template.toml`), and validator accepts it. With
`deny_unknown_fields`, typo becomes an **error** that identifies the key:

```
models/llama/groot.toml:34: unknown key in [build]: "cuantization"
  did you mean "quant"?
```

A `cuantization` typo currently slips through, so quantization uses default
instead of requested value. User notices three days later when model quality
is not as expected. Error costs one second and prevents a mystery.

**Consequence:** `models/_template.toml` and all 45 existing TOMLs must pass
new schema **before** new validator goes into production. This is F1 task T-05.

#### 4.1.1 Validation rules (complete contract)

There are **15 rules**. Every catalog violation exits with code 1.

| Rule | Violation | Exit code |
|---|---|---|
| `[model].name` is kebab-case and globally unique (active + `fired/`) | error | 1 |
| `[model].name` **must match filename** | error | 1 |
| `engine` is canonical ID or known legacy alias | error | 1 |
| `format = "safetensors"` ⇒ `[model].path` **required** | error | 1 |
| `format ≠ "safetensors"` ⇒ `[model].path` **forbidden** | error | 1 |
| `format = "gguf"` ⇒ `[files].gguf` required | error | 1 |
| `[download]` present without `[build]` ⇒ `repo` and (`file` for gguf) required | error | 1 |
| `${file}` in `[args]` not declared in `[files]` | error | 1 |
| `[levels].default` must be in `[levels].list` | error | 1 |
| `[aliases]` points to nonexistent level | error | 1 |
| `[options.default]` key must be in `[options].list` | error | 1 |
| `[args.flag.<f>].requires` references nonexistent `[files]` key | error | 1 |
| `[limits].min_ram_gb > 0` | error | 1 |
| Unknown key in any table | error | 1 |
| `context` must be > 0 and multiple of 32 | error | 1 |

This table is v2 validation contract. Four additional checks in v1—levels,
flags, options, and presence of `[download]`/`[build]`—are not implemented yet
and are recorded as CHANGELOG debt.

**Having both `[download]` and `[build]` is valid TOML state, not an error.**
Validator accepts both; row prohibiting them (exit 2) was removed (owner
decision, 2026-11-14). Declaring both paths means model **can be obtained in two
ways**, a model property rather than a defect in its record. Thus:

- **Validator** says nothing about which path to use: both are valid and
  validated separately.
- **Path selection** belongs to `ropero install models` (F5/T-25, R-MOD-29):
  with both paths ask user; `--prio` chooses without prompting. Full `--prio`
  contract is §4.10.7.
- **Exit 2** (usage error) is reserved for commands, not validator.

**`name` == filename rule** is measured debt (H-23): validator does **not**
check it today. Add it here.

**Validation errors must include path and line.** This makes validator a tool,
not an obstacle:

```
models/llama/groot.toml:12
  [model].format = "safetensors" requires [model].path
```

### 4.2 Engine identifiers

Eight engines with **canonical** IDs and **legacy** aliases:

| Canonical | Accepted legacy aliases |
|---|---|
| `llama-server` | `llama` |
| `llama-server-official` | — |
| `mlx-serve` | `mlx_lm` |
| `fastflowlm` | `flm` |
| `tensorrt-llm` | `tensorrt` |
| `vllm` | — |
| `airllm` | — |
| `strata` | `strata_engine` |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineId {
    LlamaServer,
    LlamaServerOfficial,
    MlxServe,
    FastFlowLm,
    TensorRtLlM,
    Vllm,
    AirLlm,
    Strata,
}
```

The enum **is** engine registry: add engine by adding variant (compiler flags
every `match` needing update) and a manifest. This directly answers “engine
integration is development”: today new engine means Bash file with eight
hooks; here it is enum variant + data file, and compiler identifies missing
updates.

Legacy alias is canonicalized **with warning**, never silently:

```
WARNING: engine 'mlx_lm' is a legacy alias; using 'mlx-serve'
```

### 4.3 Engine manifest (`engines/<id>.toml`)

**New in v2.** It embodies “engine as data” (§3.3.1).

```toml
# engines/llama-server.toml
id = "llama-server"
label = "llama-server (fork TurboQuant)"
maturity = "proven"

compat = [
  { os = ["linux"] },
  { os = ["macos"], metal = true },
]
deps = ["cmake", "git", "ninja"]
port = 9999
native_auth = false

[install]
kind = "source"
repo = "Lorenzo-SF/ropero-llama-turboquant"
public_fallback = "https://github.com/Lorenzo-SF/ropero-llama-turboquant"
dir = "LLAMA_DIR"
build = "cmake"
patches = ["patches/nvfp4-y-pattern.patch"]

[install.build.linux]
requires = ["nvidia-smi", "nvcc", "g++"]
cmake_args = ["-DGGML_CUDA=on"]

[install.build.macos]
requires = ["cmake", "clang"]
cmake_args = ["-DGGML_METAL=on"]

[launch]
kind = "template"
binary = "${LLAMA_DIR}/build/bin/llama-server"
template = [
  "--model", "${gguf}",
  "--host", "127.0.0.1",
  "--port", "${port}",
]
auth = "none"
```

**Three engines do not fit `kind = "template"`**, hence enum variant:

| Engine | Why not a template |
|---|---|
| `strata` | Not a server: needs its own *pack*, *expert-profile*, and *tune*. Actual launch is `bin/server/strata-run.sh`. |
| `fastflowlm` | Server launches through FLM-specific CLI, data model, and NPU check. |
| `airllm` | Model lives **inside proxy process**, not subprocess; data flow differs radically. |

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LaunchSpec {
    /// Most engines: argv template.
    Template { binary: String, template: Vec<String>, auth: AuthMode },
    /// Custom Rust module.
    Custom { module: String },
}
```

`Custom` is an **escape hatch**: when engine cannot be expressed as template,
write Rust for it in `ropero-runtime/src/engines/`, not scattered across code.

### 4.4 On-disk state (§3.3.4)

`KEY=value` format, one file per port (§3.3.4). **Format is stable**: Ropero v2
must read state written by v1 because user may have both installed during
transition (§9, F7).

`VERSION` enables this:

- **No `VERSION`** (v1-written state): migrate on read. Missing keys
  (`STARTTIME`, `EXEDIGEST`) are derived or left empty; process is treated as
  **foreign** until identity verified by other means (§3.3.3). Safe posture:
  without `start_time`, cannot prove PID is ours.
- **`VERSION=1`**: native v2 state.
- **`VERSION` greater than known**: **reject with clear message** (“state
  written by a newer Ropero version”), do not partially interpret. Partial
  state is worse than no state.

### 4.5 Stdout contract (`ropero use`)

Public contract consumed by opencode plugin (R-OC).

```
ROPERO_PORT=9999
ROPERO_MODEL=groot
ROPERO_ENGINE=llama-server
```

**Three rules:**

1. **Last line is always `ROPERO_PORT=<n>`**, with nothing after; consumer can
   use `tail -1`.
2. **Only these lines** go to stdout. Progress, warnings, and errors go to
   **stderr**. Enables `PORT=$(ropero use groot | tail -1)`.
3. In `--background` mode, emit all three lines **after** engine is ready, not
   before. Consumer must not receive a port that does not yet respond.

Rule 3 is most underestimated and most often broken: if `ropero use` returns
port before engine is ready, consumer connects and gets *connection refused*,
blaming HTTP server instead of Ropero.

### 4.6 Endpoint API

OpenAI compatible. Endpoints implemented by Ropero:

| Method | Route | Behavior |
|---|---|---|
| `GET` | `/v1/models` | List models served now |
| `GET` | `/v1/models/{id}` | One model |
| `POST` | `/v1/chat/completions` | Chat (SSE streaming or not) |
| `POST` | `/v1/completions` | Completions |
| `POST` | `/v1/embeddings` | Embeddings |
| `GET` | `/health` | Proxy health (no auth; reveals no information) |
| `GET` | `/ready` | All upstreams ready |

**Proxy rules:**

- **Rewrite `model`:** client requests `groot`; destination engine may expect
  `Qwen3.8-Flash-Next-Gyro-M-TQ2_0`. Proxy rewrites request body before
  forwarding and **does not rewrite response** (R-API-8).
- **Streaming:** if client requests `stream: true`, forward response
  **token-by-token**, without buffering. Buffering breaks local model's
  perceived latency, its only advantage.
- **Auth:** `Authorization: Bearer <key>`. No key ⇒ 401; wrong key ⇒ 401.
  **Same error for both** (distinguishing missing from invalid key is an
  oracle for attackers).
- **Upstream errors:** translate to OpenAI-shaped errors with correct status.
  Engine 503 must not become HTTP 200 with error body.

---

## 5. Design patterns

A design pattern not named in code is a pattern no one can evaluate. Patterns
used here are justified by concrete problems measured in doc A.

### 5.1 The eleven patterns and where they apply

| # | Pattern | Actual problem solved | Location |
|---|---|---|---|
| 1 | **Newtype** | `Port` and `ModelName` are `u16` and `String`, confused with each other and arbitrary values. `Port(9999)` cannot be passed where level is expected. | `core::types::{Port, ModelName, LevelName}` |
| 2 | **Closed enum** (instead of engine `String`) | Engine set has 8 values. With `String`, typo (`"llama-server "`) is found at runtime by engine, after 40 GB download. | `core::engine::EngineId` |
| 3 | **State machine** (explicit states) | State spread across files goes out of sync. Doc A measured `stop` killing foreign process because state cannot distinguish “mine” and “foreign”. | `core::state::RuntimeState` + `runtime::supervisor` |
| 4 | **Strategy** (per engine) | Engines launch differently. Today 8 Bash files with 8 hooks; tomorrow 8 `match` arms, and every change touches all 8. | `runtime::engines::{trait Engine, …}` |
| 5 | **Repository** (on-disk state) | Six sites access disk, each reimplementing parse and error handling. | `runtime::state::StateStore` |
| 6 | **Ports & Adapters** (hexagonal) | Domain must be testable without real processes or disk. | `core` / `runtime` separation |
| 7 | **Builder** (argv and requests) | Four argv sources combine in fixed order; positional constructors invite order mistakes. | `core::args::ArgvBuilder` |
| 8 | **Null Object / sentinel** (probe) | Probe has three levels; only highest proves model responds. | `runtime::probe::Probe` |
| 9 | **Dependency Injection** (manual, constructor) | Tests need fake engine. If created inside function, cannot test. | All `runtime` constructors |
| 10 | **Result type / typed errors** | String error cannot be tested without comparing text. | `thiserror` throughout domain |
| 11 | **Adapter** (HTTP↔engine translation) | OpenAI API and each engine API differ; adapter reconciles them. | `server::upstream::{OpenAiUpstream, …}` |

### 5.2 Principles extending doc A

Doc A defines twelve principles (P-1…P-12). Add these four:

| # | Principle | Formulation | Why |
|---|---|---|---|
| **P-13** | **An error says what to do** | Every error includes action that resolves issue, or reason no action exists. | gguffer wound (“download incomplete”) **lies** and prompts destructive action. Error without action is ignored. |
| **P-14** | **A test that can pass without checking is not a test** | Every test must be capable of failing. Test depending on unchecked exit code is worse than no test. | Exactly wound #1 (false-green auth). |
| **P-15** | **State is a machine, not a set of files** | Runtime state is read from one place and interpreted by explicit state machine. | Enables distinction between `Failed` and `Degraded`. |
| **P-16** | **Check precondition before expensive action** | Before downloading 60 GB, check engine, disk space, and RAM. | R-GGUF-55 and R-GGUF-56 are measured cases. |

### 5.3 Forbidden patterns (and why)

| Unused pattern | Why forbidden here |
|---|---|
| `unwrap()` / `expect()` in production | Panic in destructor or signal handler leaves state halfway. Use `expect()` **only** in `main` and compiler-proven invariants. |
| `unwrap_or_default()` on config value | Silent default for port/context becomes production bug experienced as “model is slow”. |
| `println!` in `core`, `runtime`, `server` | These crates do not know about a screen. They use `tracing`. |
| `std::env::set_var` in code | Process environment belongs to launcher. **Read**, don't modify; modification silently affects children. |
| Untyped `Box<dyn Error>` | Matching on errors permits different response to “unknown model” and “port occupied”. |
| `sleep` for synchronization | Blind wait. Wait for event with deadline. |
| English comments / messages | Repo convention: code in English, text in Spanish. |
| Global flags (`--verbose` as env variable read everywhere) | `ropero use --json` must reach endpoint; reading env in three layers makes origin impossible to know. |

---

## 6. Work methodology

### 6.1 The rule governing all others

> **Test first, and the test must be able to fail.**

V1 has 16 test files, 138 assertions, and three measured wounds (R-TST-19…23).
The first—the auth test passing with broken auth—is not a writing oversight: it
is a **test whose result does not depend on what it tests**. Thus v2 has two
distinct mandatory rules:

**Rule T-1 (test first).** Write no production function without a test that
already fails. Observable commit order: test (red), then implementation
(green). Commit message body describes failure produced by test.

**Rule T-2 (test can fail).** Before accepting test, verify it **goes red when
behavior breaks**, in both ways:

- **Manual mutation:** deliberately change code (invert condition, change exit
  code) and verify test turns red. Revert mutation.
- **Probe test:** for tests about “system behaves this way”, a second test
  deliberately breaks behavior and verifies first test fails.

T-2 prevents wound #1. A test unable to fail is worse than no test: it creates
false confidence and occupies suite space.

### 6.2 Work cycle

Every task (§9) follows same cycle; **no step is skipped**:

```
1. READ      Read requirement (doc A) and evidence. Note its verb.
2. MODEL     Write type (struct/enum) or signature with doc comment.
             If signature cannot be written, requirement is not understood: return to 1.
3. TEST      Write failing test. Run it. VERIFY IT FAILS.
             (A test passing before anything is written is empty.)
4. CODE      Write minimum implementation that passes test.
5. VERIFY    Run full suite. Fix failures—do not lower test bar.
6. MUTATE    Break behavior deliberately. Verify test turns red (T-2).
7. REFACTOR  Clean up. Tests remain green.
8. COMMIT    One commit. Format in §6.4.
9. DOC       If change alters contract, update doc A in SAME commit (§6.6).
```

**Step 3 is mandatory.** Writing implementation first and test afterward makes
test pass immediately and feels controlled. That is exactly how wound #1
arose: test written after code verifies what code does, not what it should do.

### 6.3 Commits

**Conventional Commits, in Spanish.** V1 already uses them.

```
<type>(<scope>): <imperative description>

<body: why, not what. Reference R-XXX-N it closes.>
<footer: issues>
```

| Type | When |
|---|---|
| `feat` | New capability |
| `fix` | Correct behavior |
| `refactor` | Restructure **without** behavior change |
| `test` | Tests only |
| `docs` | Documentation only |
| `build` | Dependencies, toolchain, CI |
| `chore` | Everything else |

**One commit = one verifiable change.** If message needs “and”, make two commits.

**Body explains why.** Real example:

```
fix(forge): check engine prerequisite before downloading

gguffer searched for engine only in $LLAMA_DIR. With official engine
installed ($LLAMA_OFFICIAL_DIR), it was not found and error classifier
blamed "incomplete download", prompting deletion and re-download of 60 GB
to resolve an environment issue.

Now ensure_toolchain() runs before phase 1 and every cause has its own error
variant with corresponding action.

Closes: R-GGUF-54, R-GGUF-55, R-GGUF-56, R-GGUF-57, R-GGUF-58
Refs: H-27, H-28, H-29
```

### 6.4 Branches and pull requests

- Branch per task: `v2/<area>-<short-description>`, e.g. `v2/forge-engine-prerequisite`.
- **Never commit directly to `main`.**
- PR states requirement closed, tests covering it, and **what was measured**
  (numbers in PR must be measured, not estimated).
- Owner merges with squash. Existing repo convention; unchanged.

### 6.5 What may and may not be changed

| Allowed in a task | Not allowed |
|---|---|
| Task's crate and tests | `main` |
| Corresponding test file | Public CLI (unless task says so) |
| Doc A if behavior changes (and only then) | Model TOML schema |
| `Cargo.toml` adding dependencies, with ADR | Manual edits to generated `Cargo.lock` |

### 6.6 Update doc A with code

**Hard rule.** If task changes behavior, update doc A **in same commit**, with
same requirement number and new evidence. Documentation describing nonexistent
code is worse than no docs: specification is golden-master oracle; misaligned
oracle makes tests validate fantasy.

**Evidence format:** `file:line` or `file:line-line`, not filename alone;
reader must be able to locate it.

### 6.7 When something is undefined

An AI building this will find gaps. These rules prevent silent improvisation:

| Situation | Action |
|---|---|
| Doc A defines behavior | Implement it as written and cite R-XXX-N in commit. |
| Doc A marks requirement 🔶 | Implement as target state and mark it in code and CHANGELOG. |
| Doc A defines contradictory requirements | **Stop and document it.** Do not commit “implement R-X-3 and R-X-7” when they conflict. |
| Doc A says nothing and gap is **minor** (message, name, default) | Decide, record in ADR (§6.8), continue. Minor gap does not justify stopping. |
| Doc A says nothing and gap is **major** (new format/API/observable behavior change) | **Stop and ask.** Record gap in issue and wait. |
| Gap affects **security** (process identity, key, permissions) | **Stop and ask. Always.** |
| Gap affects **model TOML format** | **Stop and ask.** It is public contract. |

Never invent behavior and assume it is accepted. An invented requirement that
works is still unrequested and unmaintained.

### 6.8 ADRs

An ADR (Architecture Decision Record) is a 10–20-line file recording decision
and rationale. Stored in `docs/adr/`.

**Write an ADR when:**

- Adding a non-trivial dependency.
- Changing a written rule (including this document).
- Rejecting an alternative likely to be proposed again.
- Making a decision with irreversible cost.

**Template:**

```markdown
# ADR-NN · <title>

**Status**: accepted | rejected | superseded by ADR-NN
**Date**: 2026-11-12

## Context
<problem requiring decision; include figures if available>

## Decision
<decision in one sentence>

## Alternatives considered
- <alternative> — <why not>

## Consequences
<gains; losses; constraints introduced>
```

**ADRs are project memory.** Doc A explains *what* Ropero does; ADR explains
*why*. Without ADR, someone asks in a year “why is state `KEY=value`, not
TOML?” and answer is “because it always was”, which is no answer.

### 6.9 What remains Bash

Default rule: **nothing**. Three justified exceptions, each with reason and
review date:

| What | Why it remains Bash | Review point |
|---|---|---|
| `install.sh` | User's **first** contact with Ropero; must work where Rust, compiler, and network are absent. Installer requiring compiler cannot install anything. | When repo distributes signed binaries, replace with 20-line script downloading binary. |
| `opencode-plugin/` | TypeScript because opencode loads Node plugins. Not a choice: it is consumed format. | Never (while opencode uses Node). |
| `test/fakes/` | Fake `hf`, `llama-quantize` must resemble real binaries. Rust would require separate, larger binaries. | When `ropero-fakes` crate exists. |

**Everything else is Rust.** New `bin/*.sh` requires ADR explaining why.

### 6.10 Deliverables at end of each phase

Each phase delivers:

1. **Compiling code** and green full suite.
2. **New tests** that failed before, demonstrated in PR.
3. **Updated doc A** if behavior changed.
4. **ADR** if non-trivial decision was made.
5. **CHANGELOG** entry.
6. **PR** with description and measured figures.

A phase without green suite is **not complete**, even if code “works on my
machine”. P-2 and P-14 apply to delivery.

---

## 7. Definition of Done per requirement

### 7.1 What a Definition of Done means here

An acceptance gate is **not** “it is implemented”. It states **what is checked
and where**, in a way a reviewer (human or AI) can execute without asking
anything.

Every row in §7.4 meets four properties:

1. **Names an artifact.** A specific file, module, or test. No “in the right place”.
2. **Names a test that can fail.** If test cannot turn red, gate is invalid (P-14).
3. **Is locatable.** Doc A evidence (`file:line`) is the *v1 seed*, starting
   point for golden master (§8.1).
4. **Distinguishes implemented from target state.** 🔶 requirements carry their
   marker and architecture-change note.

### 7.2 Global gate

A task is complete when **all** are true:

| # | Gate | How to check |
|---|---|---|
| G-1 | Code compiles without warnings | `cargo clippy --all-targets -- -D warnings` |
| G-2 | Formatting is rustfmt-clean | `cargo fmt --check` |
| G-3 | Full suite passes | `cargo nextest run --all-features` |
| G-4 | Each new test failed before | Failure evidence in PR |
| G-5 | Each new test can fail again | Deliberate mutation (§6.1, T-2) |
| G-6 | Coverage does not decrease | `cargo llvm-cov` ≥ §9.4 threshold |
| G-7 | No new dependency without ADR | `cargo deny check` + review |
| G-8 | Doc A updated if behavior changed | Diff review |
| G-9 | CHANGELOG entry exists | Diff review |
| G-10 | Commit message says **why** | Diff review |
| G-11 | CLI contract unchanged | `cargo test --test cli_contract` |
| G-12 | No production `unwrap()` | `grep -rn '\.unwrap()' crates/ --include='*.rs'`, except `main.rs` |

**G-5 is the gate missing in v1** and would have detected wound #1 automatically.

### 7.3 Gates by family

Each requirement family has a **gate type** and a **characteristic risk** it
must cover:

| Family | Gate type | Risk covered |
|---|---|---|
| R-ARQ | Structural: module graph and CLI size | Logic disperses and system becomes incomprehensible |
| R-DATA | Contract: input TOML, output error with `file:line` | Malformed data passes silently |
| R-ENG | Enumeration: engine is a `enum` case | New engine integrated only halfway |
| R-CLI | Interface: exit code + stdout + stderr, byte-for-byte | User/opencode contract breaks |
| R-RUN | Process: real process whose state is checked | “Started” does not mean alive and responding |
| R-EST | Non-destruction: foreign process survives operation | Ropero kills what it does not own |
| R-API | Protocol: real request to server, status + body | Proxy returns non-OpenAI-shaped responses |
| R-ROUTE | Selection: same input reaches correct destination | Routing silently degrades quality |
| R-INST | Effect: inspect **disk**, not output | Installer touches something it must not |
| R-MOD | Download: fake `hf` records calls | Unrequested or duplicate download |
| R-TST | Meta: test can fail, suite has its own probe | Suite gives false green |
| R-OC | opencode contract: plugin receives expected data | Integration fails at point of use |
| R-STRATA | Artifact: generated pack/profile are correct | Wrong model served under right name |
| R-GGUF | Artifact: final GGUF exists, readable, traceable | Expensive work lost without explanation |

### 7.4 The table

The 317 rows, ordered by the doc A section they come from.

### 3.2 Supported engines

*v2 module*: `crates/ropero-core/src/engine.rs` · *Test area*:
`crates/ropero-runtime/tests/engines.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-CTX-1** | `install engines` offers only engines compatible with machine OS/hardware; Metal is absent on Linux. | `crates/ropero-runtime/tests/engines.rs`<br>*v1 seed*: `bin/lib/03-engines.sh:5-17`, `bin/lib/cmd-install.sh:495-499`. | Test `r-ctx_01_install_engines_solo_ofrece_los_motores` in `crates/ropero-runtime/tests/engines.rs` (unit): checks requirement against `crates/ropero-core/src/engine.rs`; turns red if behavior changes. |
| **R-CTX-2** | `install models` offers only models belonging to installed engines: with 80 catalog models and one engine installed, offer its 15 models. | `crates/ropero-runtime/tests/engines.rs`<br>*v1 seed*: `bin/lib/cmd-models.sh:136-145`, `:602-767`. | Test `r-ctx_02_install_models_solo_ofrece_los_modelos` in same file (unit): checks requirement against engine implementation; turns red if behavior changes. |
| **R-CTX-3** 🔶 | Engines and models do not all have same maturity; spec must not pretend otherwise (plus table in doc A). | `crates/ropero-core/src/engine.rs`<br>*Architecture change*: new `maturity` field required as target state by doc A. | `ropero install engines` and `ropero doctor` show maturity from manifest field; `ropero list --all` marks models with `maturity != proven`. |

### 4.1 Architecture and modules

*v2 module*: `crates/ropero-*/` · *Test area*:
`crates/ropero-core/tests/architecture.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-ARQ-1** | Thin dispatcher (`bin/ropero`, ~200 lines) with lazy-loaded modules. | `crates/ropero-*/`<br>*Architecture change*: replace 200-line dispatcher with `clap` derive. | `ropero-cli` has no business logic, <400 lines, and every subcommand `match` delegates to another crate. |
| **R-ARQ-2** | Routing table complete from day one and not edited later; each phase only adds `cmd-*.sh` and `rp_cmd_<command>`. | `crates/ropero-core/tests/architecture.rs` | Test `r-arq_02_tabla_ruteo_completa_desde_primer_dia` checks implementation and turns red on change. |
| **R-ARQ-3** | Unported command says `not yet ported (phase N)` and exits 2. | `crates/ropero-core/tests/architecture.rs`<br>*v1 seed*: `bin/ropero:252-263`. | Test `r-arq_03_comando_portado_responde_aun_portado_fase` checks agreed exit code and turns red on change. |
| **R-ARQ-4** | Resolve `ROPERO_ROOT` by following `$0` symlink chain in `readlink` + `cd/pwd` loop, without `readlink -f` (absent from macOS Bash 3.2). | Same test file<br>*v1 seed*: `bin/ropero:66-99`. | Test `r-arq_04_roperoroot_resuelve_siguiendo_cadena_symlinks_con` verifies behavior and turns red on change. |
| **R-ARQ-5** | Modules include `00-utils` (utilities/presentation), `01-env` (environment), `02-modelconf` (catalog wrapper), `03-engines` (engine loader), `04-launch` (runtime), … | Same test file | Test `r-arq_05_los_modulos_son_utils_utilidades_presentacion` checks structure and turns red on change. |
| **R-ARQ-6** | Catalog has one source of truth: Python parser `bin/python/modelconf.py`. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:5-7`, `:176-184`. | Test `r-arq_06_catalogo_tiene_una_unica_fuente_verdad` checks implementation and turns red on change. |
| **R-ARQ-7** | Parser is self-contained: includes own TOML reader (`parse_toml`, line 478) and uses `tomllib` only if available (`_tomllib()`, line 519). | Same test file | Test `r-arq_07_parser_autocontenido_trae_propio_lector_toml` checks implementation and turns red on change. |
| **R-ARQ-8** | Python interpreter is tested, not assumed: iterate candidates and run `python -c 'import sys'`. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:60-90`. | Test `r-arq_08_interprete_python_prueba_supone_recorren_los` checks behavior and turns red on change. |
| **R-ARQ-9** | Runtime state is `$STATE_DIR/ports/<port>`, a `KEY=value` file with `MODEL=`, `ENGINE=`, `PID=`, `PORT=`, `LEVEL=`, `TS=`, `LOG=`, `STAMP=`. | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:249-302`. | Test `r-arq_09_estado_runtime_vive_statedir_ports_puerto` checks implementation and turns red on change. |
| **R-ARQ-10** | Model directory has two levels: active `models/<engine>/` and retired `models/<engine>/fired/`. | Same test file<br>*v1 seed*: `models/_template.toml:5-7`. | Test `r-arq_10_directorio_modelos_tiene_dos_niveles_models` checks structure and turns red on change. |
| **R-ARQ-11** | Tool state can be reconstructed from disk: managed ports derive from `$PORT`, `$CPU_PORT`, active TOML `[model].port`, and files that… | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:304-334`. | Test `r-arq_11_estado_herramienta_puede_reconstruir_desde_disco` checks implementation and turns red on change. |

### 4.2.1 Tables and rules

*v2 module*: `crates/ropero-core/src/model.rs` · *Test area*:
`crates/ropero-core/tests/model.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-DATA-1** | `[model]` is required. | `crates/ropero-core/tests/model.rs`<br>*v1 seed*: `bin/python/modelconf.py:1474-1477`. | Test `r-data_01_model_obligatoria` verifies required field against model implementation and turns red if behavior changes. |
| **R-DATA-2** | `[files]` is required. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1479-1494`. | Test `r-data_02_files_obligatoria` verifies requirement and turns red on change. |
| **R-DATA-3** | `[download]` is required unless model is built through `[build]`. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1496-1529`, `models/_template.toml:47-58`. | Test `r-data_03_download_obligatoria_salvo_que_modelo_fabrique` verifies requirement and turns red on change. |
| **R-DATA-4** | `[download.aux.<type>]` describes auxiliary files. | Same test file<br>*v1 seed*: `models/_template.toml:60-68`. | Test `r-data_04_download_aux_tipo_describe_auxiliares` verifies behavior and turns red on change. |
| **R-DATA-5** | `[build]` declares how to create GGUF: `repo`, `gguf`, `gguf_file`, `quant`, `imatrix`, `imatrix_repo`, `imatrix_file`, `out`, `out_principal`, `from_subdir`, `include`,… | Same test file<br>*v1 seed*: `models/_template.toml:71-93`. | Test `r-data_05_build_declara_como_fabrica_gguf_repo` verifies requirement and turns red on change. |
| **R-DATA-6** | If TOML has `[download]` and `[build]`, `ropero install models <model>` requires `--download` or `--build` to choose path. | Same test file<br>*v1 seed*: `README.md:72-73`, `models/_template.toml:72-73`. | Test `r-data_06_toml_tiene_download_build_ropero_install` verifies required behavior and turns red on change. |
| **R-DATA-7** | `[levels]`, `[flags]`, and `[options]` declare what model accepts on command line. | Same test file<br>*v1 seed*: `models/_template.toml:96-98`, `bin/lib/cmd-runtime.sh:143`, `cmd-catalog.sh:272`. | Test `r-data_07_levels_flags_options_declaran_que_modelo` verifies behavior and turns red on change. |
| **R-DATA-8** | `[levels].default` must be in `[levels].list`. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1531-1555`. | Test `r-data_08_levels_default_tiene_que_estar_levels` verifies requirement and turns red on change. |
| **R-DATA-9** | `[limits].min_ram_gb` is a number > 0. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1557-1565`, `bin/lib/04-launch.sh:598-617`. | Test `r-data_09_limits_minramgb_numero` verifies requirement and turns red on change. |
| **R-DATA-10** | `[args]` is required for `engine = llama-server` and must contain `base` as a list. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1567-1582`. | Test `r-data_10_args_obligatoria_para_engine_llama_server` verifies required field and turns red on change. |
| **R-DATA-11** | `${file}` used in `[args]` but not declared in `[files]` is an error. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1625-1644`. | Test `r-data_11_fichero_usado_args_pero_declarado_files` verifies error and turns red on change. |
| **R-DATA-12** | TOML folder must be a known `engine_dir` (`llama`, `llama-official`, `mlx`, `airllm`, `flm`, `tensorrt`, `vllm`, `strata`). | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1646-1649`. | Test `r-data_12_carpeta_del_toml_tiene_que_ser` verifies requirement and turns red on change. |

### 4.2.2 Assembling argv

*v2 module*: `crates/ropero-core/src/args.rs` · *Test area*:
`crates/ropero-core/tests/args.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-DATA-13** | Model is not executable: `[args]` is a list of *tokens*, and final argv is assembled in fixed order. | `crates/ropero-core/tests/args.rs`<br>*v1 seed*: `models/_template.toml:144-146`. | Test `r-data_13_modelo_ejecuta_sus_args_son_una` checks returned argv and turns red on change. |
| **R-DATA-14** | One list element = one argument. | Same test file | Test `r-data_14_elemento_lista_argumento` checks returned argv and turns red on change. |
| **R-DATA-15** | `[args.level.<n>]` accepts three equivalent ways to express level: literal `base` args, `reasoning_effort` shortcut translated to `--chat-template-kwargs <json>`, and… | Same test file<br>*v1 seed*: `models/_template.toml:118-122`. | Test `r-data_15_args_level_admite_tres_formas_equivalentes` checks behavior and turns red on change. |
| **R-DATA-16** | `[args.flag.<f>]` can declare `requires = [<keys in [files]>]`; if file is absent from disk, flag is an error. | Same test file<br>*v1 seed*: `models/_template.toml:124-129`, `bin/python/modelconf.py:1608-1623`. | Test `r-data_16_args_flag_puede_declarar_requires_claves` checks error and turns red on change. |

### 4.2.3 Placeholders

*v2 module*: `crates/ropero-core/src/placeholders.rs` · *Test area*:
`crates/ropero-core/tests/placeholders.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-DATA-17** | Valid placeholders in any `base[]`: `${gguf}`, `${mtp}`, `${mmproj}`, `${chat_template}`, `${ctx}`, `${value}` (only in `[args.option]`), `${gguf_dir}`,… | `crates/ropero-core/tests/placeholders.rs`<br>*v1 seed*: `models/_template.toml:131-142`. | Test `r-data_17_los_placeholders_validos_cualquier_base_son` checks behavior and turns red on change. |
| **R-DATA-18** | Override without editing TOML via environment: `ROPERO_<NAME>_CTX` (effective context), `ROPERO_<NAME>_GGUF` (point `${gguf}` to another quant of same… | Same test file<br>*v1 seed*: `models/_template.toml:148-151`, `bin/lib/04-launch.sh:895`. | Test `r-data_18_pueden_sobrescribir_sin_tocar_toml_con` checks behavior and turns red on change. |

### 4.2.4 Formats

*v2 module*: `crates/ropero-core/src/format.rs` · *Test area*:
`crates/ropero-core/tests/format.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-DATA-19** | Four artifact formats: `gguf` (local file), `safetensors` (checkpoint under `$SAFETENSORS_DIR`), `q4nx` (FLM store: `[model].path` is a TAG), and `strata` (the… | `crates/ropero-core/tests/format.rs`<br>*v1 seed*: `models/_template.toml:29-32`. | Test `r-data_19_los_cuatro_formatos_artefacto_son_gguf` checks format and turns red on change. |
| **R-DATA-20** | `format = gguf` without `[download]` and `[build]` is local-only: its GGUF must already be on disk. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1521-1529`. | Test `r-data_20_modelo_format_gguf_sin_download_sin` verifies behavior and turns red on change. |
| **R-DATA-21** 🔶 | `[model].name` MUST equal TOML filename without extension; this value is alias used to start and stop model. | `crates/ropero-core/src/format.rs`<br>*Architecture change*: implemented as §4.1. | Rust schema (`serde`, `deny_unknown_fields`) accepts all 45 repository TOMLs unchanged, and `ropero schema` emits their JSON Schema. |
| **R-DATA-22** | CLI-accepted model vocabulary has three classes, each with a TOML section (plus doc A table). | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1487-1529`, `:1549-1554`; `models/_template.toml` commented example, lines 230-289. | Test `r-data_22_vocabulario_que_modelo_acepta_del_cli` checks behavior and turns red on change. |
| **R-DATA-23** | Model may use different files for download and build paths, each from a different repo: chat template, MTP, mmproj (vision part of… | Same test file<br>*v1 seed*: `models/_template.toml:205-224`; `bin/lib/cmd-models.sh:321-336`. | Test `r-data_23_modelo_puede_tener_distintos_ficheros_para` checks behavior and turns red on change. |

### 4.3 Engine contract

*v2 module*: `crates/ropero-core/src/engine.rs` · *Test area*:
`crates/ropero-runtime/tests/engines.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-ENG-1** | An engine is `bin/lib/engines/<id>.sh` defining eight required hooks (plus doc A table). | `crates/ropero-core/src/engine.rs`<br>*Architecture change*: §§3.3.1, 4.2–4.3.<br>*v1 seed*: `bin/lib/03-engines.sh:6-25`. | Engine is TOML manifest with eight contract fields; function hooks become fields and `EngineId` is closed enum. |
| **R-ENG-2** | Loader verifies all eight; missing one is named: `engine 'x': missing hook rp_eng_x_label`. | `crates/ropero-runtime/tests/engines.rs`<br>*v1 seed*: `bin/lib/03-engines.sh:93-105`. | Test `r-eng_02_cargador_verifica_los_ocho_falta_uno` verifies error and turns red on change. |
| **R-ENG-3** | Engine IDs use hyphens and are not translated: valid Bash identifiers (`rp_eng_llama-server_label`). | Same test file<br>*v1 seed*: `bin/lib/03-engines.sh:26-33`, `:54-64`. | Test `r-eng_03_los_ids_motor_llevan_guiones_traducen` verifies behavior and turns red on change. |
| **R-ENG-4** | Add engine = add one file (and `models/<dir>/` if it has models). | Same test file<br>*v1 seed*: `README.md:179-180`. | Test `r-eng_04_anadir_motor_anadir_fichero_tiene_modelos` verifies implementation and turns red on change. |
| **R-ENG-5** | Broken engine does not abort install: `_rp_engine_deps_ok` returns 1 with missing list; caller decides. | Same test file<br>*v1 seed*: `bin/lib/03-engines.sh:186-195`. | Test `r-eng_05_motor_roto_tumba_install_rpenginedepsok_devuelve` verifies return and turns red on change. |
| **R-ENG-6** | Probe checks Ropero port, not engine's internal port: Ropero endpoint `/health` must answer. | Same test file<br>*v1 seed*: `bin/lib/engines/strata.sh:143-156`. | Test `r-eng_06_probe_mira_puerto_ropero_interno_del` verifies implementation and turns red on change. |
| **R-ENG-7** | Engine without native auth (`native_auth = no`) starts without key; Ropero proxy sits in front. | Same test file<br>*v1 seed*: `bin/lib/engines/strata.sh:158-167`. | Test `r-eng_07_motor_que_tiene_auth_nativa_nativeauth` verifies implementation and turns red on change. |

### 4.4 Command line

*v2 module*: `crates/ropero-cli/src/` · *Test area*:
`crates/ropero-cli/tests/cli.rs` · *Type*: cli

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-CLI-1** | Public surface is fixed. | `crates/ropero-cli/src/`<br>*Architecture change*: S-2, CLI unchanged.<br>*v1 seed*: `bin/ropero:8-11`. | Public surface matches doc A command by command; `ropero --help` lists same 19 commands. |
| **R-CLI-2** | Commands include `use`, `stop`, `status`, `list`, `template`, `args`, `levels`, `path`, `models`, `install` (`setup`/`engines`/`link`/`opencode-plugin`), `engines`,… | `crates/ropero-cli/tests/cli.rs`<br>*v1 seed*: `bin/ropero:150-168`, routing table 160-179. | Test `r-cli_02_los_comandos_son_use_stop_status` checks surface and turns red on change. |
| **R-CLI-14** 🔶 | Public `install` surface is two commands: `ropero install engines` and `ropero install models`. | `crates/ropero-cli/src/`<br>*Architecture change*: target state in doc A; absent today. | `ropero stats [--watch]` exists, measures CPU/RAM/GPU with `sysinfo`; default is readable text, `--json` emits one JSON line per sample. |
| **R-CLI-15** | Bare `ropero install` runs full process: environment preflight → engines → models, in that order. | `crates/ropero-cli/tests/cli.rs`<br>*v1 seed*: `bin/lib/cmd-install.sh:117-130` (setup → engines → models), `:463-467`. | Test `r-cli_15_ropero_install_secas_ejecuta_proceso_completo` checks behavior and turns red on change. |
| **R-CLI-16** | `ropero install engines <id>...` and `ropero install models <alias>...` accept explicit lists for unattended use: without TTY there is no menu and a list is required. | Same test file<br>*v1 seed*: `bin/lib/cmd-install.sh:91-96`, `:464-467`; `bin/lib/cmd-models.sh:641-645`. | Test `r-cli_16_ropero_install_engines_ropero_install_models` verifies requirement and turns red on change. |
| **R-CLI-17** 🔶 | `ropero stats [--watch]` reports system CPU, RAM, GPU, with automatic refresh under `--watch`. | `crates/ropero-cli/src/`<br>*Architecture change*: avoid duplicating metrics source of truth. | `ropero stats` and `ropero doctor` share sampling layer, so metrics cannot diverge. |
| **R-CLI-3** | Exit codes: `0` success · `1` execution error · `2` usage error (including “command not yet ported”) · `130` foreground Ctrl-C. | Same test file<br>*v1 seed*: `bin/lib/00-utils.sh:170-182`, `bin/ropero:252-263`, `bin/lib/04-launch.sh:1159-1161`. | Test `r-cli_03_codigos_salida_exito_error_ejecucion_error` checks error code and turns red on change. |
| **R-CLI-4** | Usage error prints error and help pointer (`ropero <cmd> --help → usage for <cmd>`), not a twenty-line dump. | Same test file<br>*v1 seed*: `bin/lib/00-utils.sh:170-182`, `bin/ropero:236-241`. | Test `r-cli_04_error_uso_imprime_error_puntero_ayuda` verifies error and turns red on change. |
| **R-CLI-5** | General help is inline in dispatcher and works with only `00-utils.sh`/`01-env.sh`, even without `alaja` or ANSI. | Same test file<br>*v1 seed*: `bin/ropero:20-22`, `:199-213`. | Test `r-cli_05_ayuda_general_esta_linea_dispatcher_tiene` verifies behavior and turns red on change. |
| **R-CLI-6** | Unknown command not starting with `-` is treated as direct model launch, not error. | Same test file<br>*v1 seed*: `bin/ropero:265-278`. | Test `r-cli_06_comando_conocido_que_empieza_por_interpreta` checks behavior and turns red on change. |
| **R-CLI-7** | `ropero list` shows active models. | Same test file<br>*v1 seed*: `README.md:37`, `bin/lib/04-launch.sh:848-852`. | Test `r-cli_07_ropero_list_muestra_los_modelos_activos` verifies behavior and turns red on change. |
| **R-CLI-8** | `ropero args <model>` prints final argv, one argument per line. | Same test file<br>*v1 seed*: `bin/lib/cmd-catalog.sh:221`, `:299`, `:346`. | Test `r-cli_08_ropero_args_modelo_imprime_argv_final` verifies output and turns red on change. |
| **R-CLI-9** | Levels, flags, and options are checked against TOML; undeclared value is usage error, with message listing declared values… | Same test file<br>*v1 seed*: `bin/lib/cmd-runtime.sh:143`, `bin/lib/cmd-catalog.sh:258`, `:272`. | Test `r-cli_09_los_niveles_flags_opciones_validan_contra` checks error and turns red on change. |
| **R-CLI-10** | Argument grammar after model name: first argument not `k=v` is level. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:269-307`. | Test `r-cli_10_gramatica_los_argumentos_tras_nombre_del` checks behavior and turns red on change. |
| **R-CLI-11** | `ropero status <model>` returns 0 serving · 1 not · 2 unknown. | Same test file<br>*v1 seed*: `README.md:43`. | Test `r-cli_11_ropero_status_modelo_devuelve_sirviendose_desconocido` checks return and turns red on change. |
| **R-CLI-12** | `--cpu` exists only on Linux (usage error on macOS) and only with `engine = llama-server` (`--n-gpu-layers 0`, engine-specific flag). | Same test file<br>*v1 seed*: `bin/lib/cmd-runtime.sh:85-92`, `bin/lib/04-launch.sh:865-870`. | Test `r-cli_12_cpu_solo_existe_linux_macos_error` verifies error and turns red on change. |
| **R-CLI-13** | Repeated `--port`: last wins, with warning. | Same test file<br>*v1 seed*: `bin/lib/cmd-runtime.sh:94-113`, `bin/lib/04-launch.sh:142-145`. | Test `r-cli_13_port_repetido_gana_ultimo_con_aviso` verifies behavior and turns red on change. |

### 4.5.1 Preflight

*v2 module*: `crates/ropero-runtime/src/preflight.rs` · *Test area*:
`crates/ropero-runtime/tests/preflight.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-RUN-1** | Preflight occurs before touching port: doomed launch (missing artifact, invalid argv, engine not installed) does not kill running process. | `crates/ropero-runtime/tests/preflight.rs`<br>*v1 seed*: `bin/lib/04-launch.sh:857-972`. | Test `r-run_01_preflight_ocurre_antes_tocar_puerto_lanzamiento` verifies that foreign process is untouched and turns red on change. |
| **R-RUN-2** | Check chat template on disk before launch: if llama-server cannot find it, it aborts with wall of built-in templates and real error is buried near top of log. | Same test area | Test `r-run_02_chat_template_comprueba_disco_antes_arrancar` verifies error and turns red on change. |
| **R-RUN-3** | Model context reaches engine: inject `-c <context>` (or `--ctx-size`) before TOML args, so TOML's own `-c` wins. | Same test area | Test `r-run_03_contexto_del_modelo_tiene_que_llegar` verifies argv and turns red on change. |
| **R-RUN-4** | For `engine = llama-server`, Ropero adds `--model <gguf-path>` to argv: TOML declares flags, not path. | Same test area<br>*v1 seed*: `bin/lib/04-launch.sh:928-933`, `:957-963`. | Test `r-run_04_para_engine_llama_server_ropero_anade` verifies argv and turns red on change. |
| **R-RUN-5** | Resolve `ROPERO_<NAME>_GGUF` override during preflight, using same file that later fills `${gguf}`; otherwise preflight could approve a different quant from one actually launched… | `crates/ropero-runtime/src/preflight.rs`<br>*Architecture change*: P-6, idempotent by default.<br>*v1 seed*: `bin/lib/04-launch.sh:888-900`. | `ropero use` on ready model with same stamp does not restart: PID remains same and command exits in <2 s. |

### 4.5.2 Readiness

*v2 module*: `crates/ropero-runtime/src/probe.rs` · *Test area*:
`crates/ropero-runtime/tests/probe.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-RUN-6** | Wait for model, not just port: `rp_wait_ready` polls engine probe and, when it responds, requires `/v1/models` to publish requested model by name, GGUF, or… | `crates/ropero-runtime/tests/probe.rs` | Test `r-run_06_espera_modelo_solo_puerto_rpwaitready_hace` verifies behavior and turns red on change. |
| **R-RUN-7** | If process dies before listening, fail immediately; do not wait N seconds. | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:704-709`. | Test `r-run_07_proceso_muere_antes_escuchar_fallo_inmediato` verifies behavior and turns red on change. |
| **R-RUN-8** | Default timeout is `ROPERO_START_TIMEOUT=300 s`, polling every 2 s. | Same test file<br>*v1 seed*: `bin/lib/01-env.sh:63`, `bin/lib/04-launch.sh:698`, `docs/strata.md:67-72`. | Test `r-run_08_timeout_por_defecto_roperostarttimeout_300_con` verifies behavior and turns red on change. |
| **R-RUN-9** | Query `/v1/models` with API key: llama-server requires key even to list. | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:338-351`. | Test `r-run_09_models_consulta_con_api_key_llama` verifies request and turns red on change. |
| **R-RUN-10** | Failed launch kills process it started and cleans state. | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:650-682`. | Test `r-run_10_arranque_fallido_mata_proceso_que_arranco` verifies cleanup and turns red on change. |
| **R-RUN-11** | Capture PID through temporary file, never command substitution: `$( )` creates process in subshell, orphaning engine and making `wait` report “not a child…” | Same test file | Test `r-run_11_pid_captura_por_fichero_temporal_nunca` checks prohibited behavior and turns red on change. |

### 4.5.3 Stamp (STAMP)

*v2 module*: `crates/ropero-core/src/stamp.rs` · *Test area*:
`crates/ropero-core/tests/stamp.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-RUN-12** | Startup-config *stamp* is `cksum` of args (not `sha256sum`, absent on macOS). | `crates/ropero-core/tests/stamp.rs` | Test `r-run_12_sello_configuracion_arranque_cksum_los_args` verifies implementation and turns red on change. |
| **R-RUN-13** | If model already runs on target port and stamp matches, touch nothing (“already running”), exit 0. | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:986-1010`. | Test `r-run_13_modelo_corre_puerto_destino_con_sello` verifies behavior and turns red on change. |

### 4.5.4 Foreground and Ctrl-C

*v2 module*: `crates/ropero-runtime/src/supervisor.rs` · *Test area*:
`crates/ropero-runtime/tests/supervisor.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-RUN-14** | In foreground, engine always starts in background with output to logfile; terminal shows live, colored log from current launch offset (to avoid… | `crates/ropero-runtime/tests/supervisor.rs`<br>*v1 seed*: `bin/lib/04-launch.sh:1040-1048`, `:1113-1123`. | Test `r-run_14_primer_plano_engine_siempre_arranca_background` verifies behavior and turns red on change. |
| **R-RUN-15** | Foreground wait is `kill -0` loop, not `wait`: Bash delays INT trap until waited-on engine exits, and engine ignores SIGINT (`nohup`… | Same test file | Test `r-run_15_espera_primer_plano_bucle_kill_wait` verifies behavior and turns red on change. |
| **R-RUN-16** | Ctrl-C exits 130; recognize 130 from process code, not trap (which may not run). | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:1127-1128`, `:1159-1161`; `bin/lib/06-styles.sh:12-20`. | Test `r-run_16_ctrl_sale_con_130_130_reconoce` verifies exit code and turns red on change. |
| **R-RUN-17** | Startup is locked per port for duration: `.lock` directory contains PID (`mkdir` atomic). | `crates/ropero-runtime/src/supervisor.rs`<br>*Architecture change*: lock expires, never blocks forever.<br>*v1 seed*: `bin/lib/04-launch.sh:216-247`. | Next attempt removes lock whose PID is gone; live PID lock blocks with message naming owner. |

### 4.6 Ports, state, and stopping

*v2 module*: `crates/ropero-runtime/src/state.rs` · *Test area*:
`crates/ropero-runtime/tests/state.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-EST-1** | Destination port precedence: `--port N` > pinned `[model].port` > CPU slot (`:9998`, Linux only) > engine base slot. | `crates/ropero-runtime/tests/state.rs`<br>*v1 seed*: `bin/lib/04-launch.sh:147-182`. | Test `r-est_01_precedencia_del_puerto_destino_port_model` checks behavior and turns red on change. |
| **R-EST-2** | Managed ports are canonical list for `status`, `stop all`, `use` idempotence: `$PORT` + `$CPU_PORT` (Linux only) + active TOML `[model].port` + … | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:304-334`. | Test `r-est_02_los_puertos_gestionados_son_lista_canonica` checks returned list and turns red on change. |
| **R-EST-3** | Four cascading ways to ask “who listens on this port?”: `lsof` → `ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd` (socket inode). | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:52-123`. | Test `r-est_03_para_preguntar_quien_escucha_este_puerto` verifies behavior and turns red on change. |
| **R-EST-4** | Model identity on a port has four-level hierarchy: (1) `/v1/models` ID mapped to catalog; … | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:391-426`. | Test `r-est_04_identidad_del_modelo_puerto_tiene_una` verifies implementation and turns red on change. |
| **R-EST-5** | Published ID matches model if it is name, GGUF (with/without extension), or checkpoint path (basename/full path). | Same test file<br>*v1 seed*: `bin/lib/04-launch.sh:353-372`. | Test `r-est_05_publicado_casa_con_modelo_nombre_gguf` verifies behavior and turns red on change. |
| **R-EST-6** | `stop` kills by port, never global `pkill -f` (would kill parallel slots). | `crates/ropero-runtime/src/state.rs`<br>*Architecture change*: P-3 applied to mass case.<br>*v1 seed*: `bin/lib/04-launch.sh:450-500`. | Bare `ropero stop` stops **only** Ropero-managed ports (those with state file), verified with foreign open port during test. |
| **R-EST-7** | Kill state PID even if port does not listen yet: server loading model has no listener, but used to remain alive and serve after launch failed. | Test area<br>*v1 seed*: `bin/lib/04-launch.sh:458-465`. | Test `r-est_07_pid_del_estado_mata_aunque_puerto` verifies behavior and turns red on change. |
| **R-EST-8** | Before killing, verify PID was not recycled: OS reuses PIDs; stale state may point to browser. | `crates/ropero-runtime/src/state.rs`<br>*Architecture change*: P-3, never kill foreign process.<br>*v1 seed*: `bin/lib/04-launch.sh:430-448`, `:474-484`. | `ropero stop` kills only if **pid + start_time + binary digest** match. Test starts foreign process on port with live PID and different start_time; verifies it survives. |
| **R-EST-9** | Staged stop: `SIGTERM` entire tree → up to 10 s grace (check port/PIDs) → safety `SIGKILL` → delete state. | Test area<br>*v1 seed*: `bin/lib/04-launch.sh:508-536`. | Test `r-est_09_parada_escalonada_sigterm_todo_arbol_hasta` verifies behavior and turns red on change. |
| **R-EST-10** | `stop` on free port returns 1 (nothing to stop), not 0. | Test area<br>*v1 seed*: `bin/lib/04-launch.sh:502-506`. | Test `r-est_10_stop_puerto_libre_devuelve_habia_nada` verifies return and turns red on change. |
| **R-EST-11** | One API key for all engines; precedence: `LLAMA_API_KEY` > `$STATE_DIR/api_key` > generated `sk-ropero-<hex>` (openssl, fallback…). | Test area<br>*v1 seed*: `bin/lib/04-launch.sh:184-214`. | Test `r-est_11_api_key_unica_para_todos_los` verifies implementation and turns red on change. |
| **R-EST-12** | State is source of truth, written atomically (tmp + `mv`), preserving keys absent from call. | Test area<br>*v1 seed*: `bin/lib/04-launch.sh:268-296`. | Test `r-est_12_estado_fuente_verdad_escribe_forma_atomica` verifies atomicity and turns red on change. |
| **R-EST-13** | `ropero use <model>` is idempotent by (model, level) and emits `ROPERO_PORT=<n>` as last stdout line. | Test area<br>*v1 seed*: `bin/lib/cmd-runtime.sh:265-292`, `opencode-plugin/README.md:161-162`. | Test `r-est_13_ropero_use_modelo_idempotente_por_modelo` verifies behavior and turns red on change. |
| **R-EST-14** | Bare `ropero stop` stops all Ropero-managed ports. | Test area<br>*v1 seed*: `bin/lib/cmd-runtime.sh:314-324` (default target all), `:325-360`; `bin/lib/04-launch.sh:304-334`. | Test `r-est_14_ropero_stop_sin_argumentos_para_todos` verifies behavior and turns red on change. |
| **R-EST-15** 🔶 | Detecting foreign engines is an open question. Reporting “these processes use the GPU and are not mine” differs from stopping them; Ropero currently cannot… | `crates/ropero-runtime/src/state.rs`<br>*Architecture change*: data contract §4.4. | Every on-disk state has `version = 1`; unknown version is rejected with a message naming version read. |

### 4.7 The single gateway: OpenAI-compatible endpoint

*v2 module*: `crates/ropero-server/src/` · *Test area*:
`crates/ropero-server/tests/` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-API-1** | `bin/server/endpoint.py` is FastAPI/uvicorn server exposing OpenAI-compatible API with own auth and reverse-proxying (or generating in-process) for engines that… | `crates/ropero-server/tests/`<br>*v1 seed*: `bin/server/endpoint.py:13-20`. | Test `r-api_01_bin_server_endpoint_servidor_fastapi_uvicorn` verifies behavior and turns red on change. |
| **R-API-2** | Exactly five routes (plus doc A table). | Same test area<br>*v1 seed*: `bin/server/endpoint.py:1326-1429`. | Test `r-api_02_las_rutas_son_exactamente_cinco` verifies routes and turns red on change. |
| **R-API-3** | `/health` always returns 200, even while model is loading or backend is dead; actual state is in `backend.ready` and `backend.error`. | Same test area | Test `r-api_03_health_siempre_devuelve_200_incluso_con` checks error/state and turns red on change. |
| **R-API-4** | `/v1/models` returns exactly one model, with `id` = Ropero name, not backend name. | Same test area<br>*v1 seed*: `bin/server/endpoint.py:1362-1378`, `bin/server/README.md:17-18`. | Test `r-api_04_models_devuelve_exactamente_modelo_con_nombre` checks response and turns red on change. |
| **R-API-5** | Request-body `model` is always rewritten to backend's expected value: `model_id` for `tensorrt` and `strata`, FLM TAG for `flm` (FLM validates field; rewriting to Ropero ID… | Same test area<br>*v1 seed*: `bin/server/endpoint.py:886-903`, `:1532-1540`. | Test `r-api_05_campo_model_del_body_reescribe_siempre` verifies behavior and turns red on change. |
| **R-API-6** | Translate `/v1/completions` to backend `/v1/chat/completions` (FLM does not implement former), giving four backends same semantics. | Same test area<br>*v1 seed*: `bin/server/endpoint.py:1408-1423`. | Test `r-api_06_completions_traduce_chat_completions_hacia_backend` verifies request and turns red on change. |
| **R-API-7** | Errors use OpenAI format `{"error":{"message":…,"type":…,"code":…}}`, `type` ∈ `invalid_request_error` / `server_error` / `upstream_unavailable` and `code` ∈… | Same test area<br>*v1 seed*: `bin/server/endpoint.py:198-204`, analysis note §8.1. | Test `r-api_07_los_errores_usan_formato_openai_error` verifies error and turns red on change. |
| **R-API-8** | Body that is not JSON object, or invalid JSON, returns 400 `invalid_json`, never 500. | Same test area<br>*v1 seed*: `bin/server/endpoint.py:228-243`, `:1419-1422`. | Test `r-api_08_body_que_objeto_json_json_invalido` verifies rejection and turns red on change. |
| **R-API-9** | Message `content` can be a list of VLM parts: `{"type":"text"}` passes text and `{"type":"image_url"}` becomes literal `"[image]"`. | Same test area<br>*v1 seed*: `bin/server/endpoint.py:158-175`. | Test `r-api_09_content_mensaje_puede_ser_una_lista` checks response and turns red on change. |
| **R-API-10** | `airllm` does not implement tool calling; endpoint returns explicit 400 instead of pretending support (client would hang). | Same test area<br>*v1 seed*: `bin/server/endpoint.py:556-563`. | Test `r-api_10_airllm_implementa_tool_calling_endpoint_dice` verifies behavior and turns red on change. |
| **R-API-11** | Authentication accepts two headers: `Authorization: Bearer <key>` and `X-Api-Key: <key>`. | Same test area<br>*v1 seed*: `bin/server/endpoint.py:207-225`, `test/test-endpoint-auth.sh:66-70`. | Test `r-api_11_autenticacion_acepta_dos_cabeceras_authorization_bearer` verifies both and turns red on change. |
| **R-API-12** | Without configured key endpoint is open and says so in log. | `crates/ropero-server/src/`<br>*Architecture change*: distinguishing cases is attacker oracle.<br>*v1 seed*: `bin/server/endpoint.py:213-214`, `:1542-1544`. | Proxy returns 401 with **same body** for missing and incorrect key. |
| **R-API-13** | Key is never forwarded to backend: `authorization` is in `HOP_BY_HOP` and removed from upstream headers. | Test area<br>*v1 seed*: `bin/server/endpoint.py:127-129`, `:141`, `:864-884`. | Test `r-api_13_clave_nunca_reenvia_backend_authorization_esta` verifies prohibition and turns red on change. |
| **R-API-14** | Key never appears in log; endpoint always logs to stderr (stdout is consumed by Ropero). | `crates/ropero-server/src/`<br>*Architecture change*: closes proxy wound #3.<br>*v1 seed*: `bin/server/endpoint.py:145-152`. | Key reaches upstream via 0600 file or env, never argv; test checks real process. |
| **R-API-15** | Backend subprocess always listens on `0.0.0.0.1`: only network-exposed process is proxy. | Test area<br>*v1 seed*: `bin/server/endpoint.py:44-45`, `:1226`, `:1251`, `:1313`. | Test `r-api_15_sub_proceso_del_backend_siempre_escucha` verifies behavior and turns red on change. |
| **R-API-16** | Proxy may run without backend: external port opens before model is ready and returns `503 model_loading` instead of hanging. | Test area<br>*v1 seed*: `bin/server/endpoint.py:332-337`, `:549-554`. | Test `r-api_16_proxy_puede_vivir_sin_backend_puerto` verifies behavior and turns red on change. |
| **R-API-17** | One uvicorn worker: proxied backend model lives in subprocess; AirLLM model lives in proxy process and cannot be shared between workers. | `crates/ropero-server/src/`<br>*Architecture change*: preserve doc A restriction.<br>*v1 seed*: `bin/server/endpoint.py:1576-1583`. | Server uses one worker; model is in backend subprocess or, for AirLLM, proxy process. |
| **R-API-18** | `airllm` serializes generations with global lock: model moves layer-by-layer; two concurrent generations are not faster, just same throughput split and double… | Test area<br>*v1 seed*: `bin/server/endpoint.py:326-329`, `:531`. | Test `r-api_18_airllm_serializa_las_generaciones_con_lock` verifies and turns red on change. |
| **R-API-19** | If backend binary rejects flag (`No such option`), proxy removes it from argv, caches removal, and relaunches. | Test area<br>*v1 seed*: `bin/server/endpoint.py:678-717`, `:773-813`. | Test `r-api_19_binario_del_backend_rechaza_flag_such` verifies behavior and turns red on change. |
| **R-API-20** | Engine and proxy logs share file (`$LOG_DIR/<model>.log`). | Test area<br>*v1 seed*: `bin/server/endpoint.py:742-769`, `bin/lib/04-launch.sh:1035`. | Test `r-api_20_log_del_motor_del_proxy_comparten` verifies behavior and turns red on change. |
| **R-API-21** | Backend 4xx/5xx error is forwarded unchanged (same status, same bytes). | Test area<br>*v1 seed*: `bin/server/endpoint.py:922-936`, `:996-1006`. | Test `r-api_21_error_4xx_5xx_del_backend_reenvia` checks error and turns red on change. |
| **R-API-22** | For SSE streaming, proxy emits `data: [DONE]` in `finally` even if upstream cuts out (AirLLM has no server to do it). | Test area<br>*v1 seed*: `bin/server/endpoint.py:628-632`. | Test `r-api_22_streaming_sse_proxy_emite_data_done` verifies behavior and turns red on change. |
| **R-API-23** 🔶 | Proxy choice is not global: engine decides. Each engine knows if proxy is needed; its `launch` hook builds command with or without it. | `crates/ropero-server/src/`<br>*Architecture change*: invert current doc A rule (R-API-23).<br>*v1 seed*: AirLLM/Strata/TensorRT launch endpoint; MLX/FLM do not. | Engine, not Ropero, decides proxy routing: manifest declares `launch.auth` and `launch.proxy`, proxy applies it. |
| **R-API-24** 🔶 | Which engines use proxy today (measured; plus doc A table). | `crates/ropero-server/src/`<br>*Architecture change*: closes measured readable-key wound. | Proxy never puts key in engine argv; pass via mode-0600 file and test checks `/proc/<pid>/cmdline` lacks key. |
| **R-API-25** 🔶 | Endpoint is untested. Owner explicitly says they do not know whether this is only AirLLM or TensorRT because they have not tested them. | `crates/ropero-server/src/`<br>*Architecture change*: current endpoint has no rate limiting. | Proxy applies per-client `limits` (requests/minute and concurrent requests) and returns 429 with `Retry-After` when exceeded. |

### 4.8 Routing

*v2 module*: `crates/ropero-server/src/router.rs` · *Test area*:
`crates/ropero-server/tests/router.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-ROUTE-1** | Actual Ropero routing is by port and on-disk state, not request rules. | `crates/ropero-server/tests/router.rs` | Test `r-route_01_enrutado_real_ropero_por_puerto_por` checks behavior and turns red on change. |
| **R-ROUTE-2** | `bin/router.sh` does not route HTTP requests; it is a semantic model selector. | Same test file<br>*v1 seed*: `bin/router.sh:9-38`. | Test `r-route_02_bin_router_enruta_peticiones_http_selector` verifies behavior and turns red on change. |
| **R-ROUTE-3** | `router.sh` fallback is `minimax/m3.1-flash`, absent from Ropero catalog. | Same test file<br>*v1 seed*: `bin/router.sh:31-38` vs `models/`. | Test `r-route_03_fallback_router_minimax_flash_que_corresponde` checks fallback and turns red on change. |
| **R-ROUTE-4** | No repository consumer for `router.sh`; header promises `MODELS.md`, absent from repo. | Same test file<br>*v1 seed*: `bin/router.sh:6`, grep with no matches. | Test `r-route_04_router_tiene_consumidor_repo_cabecera_promete` verifies and turns red on change. |
| **R-ROUTE-5** | Classifier request body is interpolated without escaping: double quotes in description break JSON and silently trigger fallback. | Same test file<br>*v1 seed*: `bin/router.sh:27-35`. | Test `r-route_05_cuerpo_peticion_clasificador_construye_por_interpolacion` verifies behavior and turns red on change. |

### 4.9.1 The thesis: symlink-only

*v2 module*: `crates/ropero-cli/src/install.rs` · *Test area*: `test/install_test.sh` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-1 (v2)** | `install engines` builds engines with a declared `[install]` recipe and creates the symlink; without a recipe, requires an existing binary. Never deletes foreign source trees. | `crates/ropero-runtime/src/install.rs`; recipes in `engines/*.toml`. | Build is declared by manifest. Installed binary links in `$BIN_DIR`; `git`/`cmake` processes run without a shell. |
| **R-INST-2** | Installation leaves exactly two symlinks. | `test/install_test.sh`<br>*v1 seed*: `install.sh:5-20`, `:367-392`; same contract in `bin/lib/cmd-install.sh:530-537`. | Test `r-inst_02_instalacion_deja_exactamente_dos_symlinks` verifies the required writes against `crates/ropero-cli/src/install.rs`; it turns red if behavior changes. |
| **R-INST-3** | The repo is the source of truth: `git pull` immediately updates installed code without reinstalling. | `test/install_test.sh`<br>*v1 seed*: `install.sh:25-28` (rule R3). | Test `r-inst_03_repo_fuente_verdad_git_pull_actualiza` checks the requirement against `crates/ropero-cli/src/install.rs`; it turns red if behavior changes. |
| **R-INST-4** | The installer never writes inside `<repo>/`. | `test/install_test.sh`<br>*v1 seed*: `install.sh:45-47` (R11), `bin/lib/cmd-install.sh:538-549`. | Test `r-inst_04_instalador_nunca_escribe_dentro_repo` checks that the prohibited behavior never occurs; it turns red if behavior changes. |
| **R-INST-5** | Offline: no network mode, no `--tarball`, no stdin bootstrap. | `test/install_test.sh`<br>*v1 seed*: `install.sh:35-36` (R6), `:556-557`. | Test `r-inst_05_sin_red_hay_modo_red_tarball` checks the requirement; it turns red if behavior changes. |
| **R-INST-6** | The user's PATH is never modified: if `~/.local/bin` is absent from PATH, Ropero warns and prints the missing `export`, but does not edit shell files. | `test/install_test.sh`<br>*v1 seed*: `install.sh:408-415` (R13). | Test `r-inst_06_path_del_usuario_modifica_nunca_local` checks that the prohibited change never occurs; it turns red if behavior changes. |
| **R-INST-7** | The installer creates only `$BIN_DIR` and `dirname "$DIR"`. | `test/install_test.sh`<br>*v1 seed*: `install.sh:363-364`, `bin/lib/01-env.sh:75-78`. | Test `r-inst_07_instalador_solo_crea_bindir_dirname_dir` verifies the requirement; it turns red if behavior changes. |

### 4.9.2 Flags and exit codes

*v2 module*: `crates/ropero-cli/src/install.rs` · *Test area*: `test/install_test.sh` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-8** | `install.sh` flags are a closed set (plus table in doc A). | `test/install_test.sh`<br>*v1 seed*: `install.sh:69-75`, `:535-562`. | Test `r-inst_08_los_flags_install_son_conjunto_cerrado` checks the requirement; it turns red if behavior changes. |
| **R-INST-9** | `--local` together with `--check` → `--check` wins: check runs before migration or linking. | `test/install_test.sh`<br>*v1 seed*: `install.sh:563`, `:567-570`. | Test `r-inst_09_local_junto_check_gana_check_comprobacion` verifies the precedence; it turns red if behavior changes. |
| **R-INST-10** | Installer uses only `pipefail`, without `errexit` or `nounset`, so an implicit failure does not abort a partial install without a message. | `test/install_test.sh`<br>*v1 seed*: `install.sh:61-66`. | Test `r-inst_10_instalador_usa_solo_pipefail_sin_errexit` checks the requirement; it turns red if behavior changes. |
| **R-INST-11** | Output goes to stderr with `[✓] [!] [✗] [-]` markers, in Spanish, with final count “N ok · M warnings · K failures”. | `test/install_test.sh`<br>*v1 seed*: `install.sh:41-42`, `:86-108`. | Test `r-inst_11_salida_stderr_con_marcadores_espanol_con` checks output; it turns red if behavior changes. |
| **R-INST-12** | Installer verifies its result: after linking, runs `bash <repo>/bin/ropero --help`; nonzero is a failure (install exit 1), not a warning. | `test/install_test.sh`<br>*v1 seed*: `install.sh:423-434` (R9). | Test `r-inst_12_instalador_verifica_propio_resultado_tras_enlar` checks the exit code; it turns red if behavior changes. |

### 4.9.3 Migration from the old layout and rescue

*v2 module*: `crates/ropero-cli/src/install.rs` · *Test area*: `test/install_test.sh` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-13** | `fase_migrar` decides from the state of `$DIR` (plus table in doc A). | `test/install_test.sh`<br>*v1 seed*: `install.sh:275-356`. | Test `r-inst_13_fasemigrar_decide_por_estado_dir` verifies the requirement; it turns red if behavior changes. |
| **R-INST-14** | Rescue hooks run before deleting anything: `_rescatar_engines` (for every engine binary symlink inside the old farm) and `_rescatar_venv` (the gguffer venv). | `test/install_test.sh`<br>*v1 seed*: `install.sh:191-258`. | Test `r-inst_14_los_hooks_rescate_ejecutan_antes_borrar` checks the requirement; it turns red if behavior changes. |
| **R-INST-15** | Do not rescue an engine if `command -v <name>` already resolves; do not shadow a working Homebrew or system binary. | `test/install_test.sh`<br>*v1 seed*: `install.sh:196-214` (F2). | Test `r-inst_15_rescate_motor_hace_command_nombre_resuelve` checks the prohibited behavior; it turns red if behavior changes. |
| **R-INST-16** | Rescue overwrites nothing: if destination already exists (file or symlink), or link is dangling or non-executable, do not rescue. | `test/install_test.sh`<br>*v1 seed*: `install.sh:215-223`. | Test `r-inst_16_rescate_pisa_nada_destino_existe_fichero` verifies the requirement; it turns red if behavior changes. |
| **R-INST-17** | Rescued link points to the real binary (symlinks resolved), so it still works if `$BIN_DIR` is emptied and repopulated. | `test/install_test.sh`<br>*v1 seed*: `install.sh:219-228`. | Test `r-inst_17_enlace_rescatado_apunta_binario_real_symlinks` verifies the requirement; it turns red if behavior changes. |
| **R-INST-18** | A rescued venv that collides with an existing one is moved aside as `venv.farm.$$`; nothing is overwritten. | `test/install_test.sh`<br>*v1 seed*: `install.sh:236-258`. | Test `r-inst_18_venv_rescatado_que_colisiona_con_uno` verifies the requirement; it turns red if behavior changes. |

### 4.9.4 What the installer never touches

*v2 module*: `crates/ropero-cli/src/install.rs` · *Test area*: `test/install_test.sh` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-19** | Closed list of untouchables: the repo and user-owned files. | `test/install_test.sh`<br>*v1 seed*: `install.sh:369-373`, `:404-406`, `:352`, `:324-326`, `bin/lib/cmd-install.sh:601-604`. | Test `r-inst_19_lista_intocables_repo` checks the returned list; it turns red if behavior changes. |
| **R-INST-20** | There is no `--uninstall`: uninstall only by manually removing the two symlinks. | `test/install_test.sh`<br>*v1 seed*: grep for “uninstall” in `install.sh` → no results. | Test `r-inst_20_existe_uninstall_unica_desinstalacion_borrar_los` verifies the requirement; it turns red if behavior changes. |

### 4.9.5 `--check`

*v2 module*: `crates/ropero-cli/src/install.rs` · *Test area*: `test/install_test.sh` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-21** | `--check` is read-only and performs five checks: (1) entrypoint must be a symlink resolving to an existing executable; (remaining checks are detailed in doc A). | `test/install_test.sh`<br>*v1 seed*: `install.sh:445-530`, `:501-512`. | Test `r-inst_21_check_read_only_hace_cinco_cosas` verifies the requirement; it turns red if behavior changes. |

### 4.9.6 Engines: three states that are easily confused

*v2 module*: `crates/ropero-runtime/src/preflight.rs` · *Test area*: `crates/ropero-runtime/tests/preflight.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-INST-22** | There are three distinct engine states, which doctor distinguishes (plus table in doc A). | `crates/ropero-runtime/tests/preflight.rs`<br>*v1 seed*: `bin/lib/cmd-install.sh:506-515`, `:593-613`; `bin/lib/cmd-doctor.sh:286-301`. | Test `r-inst_22_hay_tres_estados_distintos_motor_doctor` verifies the requirement; it turns red if behavior changes. |
| **R-INST-23** | An engine whose installer reports OK but is not usable is a failure, not a disguised success (`rc=1`). | Same test area<br>*v1 seed*: `bin/lib/cmd-install.sh:506-515`. | Test `r-inst_23_motor_cuyo_instalador_dice_pero_que` verifies the requirement; it turns red if behavior changes. |
| **R-INST-24** | `resolve` precedence is PATH → `$BIN_DIR` → long paths, for all eight engines. | Same test area<br>*v1 seed*: `bin/lib/engines/llama-server.sh:314-327`, `llama-server-official.sh:228-241`, `vllm.sh:73-84`, `mlx-serve.sh:76-138`, `tensorrt-llm.sh:103…` (source truncates). | Test `r-inst_24_precedencia_resolve_path_bindir_rutas_largas` verifies the requirement; it turns red if behavior changes. |
| **R-INST-25** | Link names are the `basename` of the resolved binary: `llama-server`, `llama-server-official`, `vllm`, `mlx-serve`, `trtllm-serve`, `flm`, `strata`. | Same test area<br>*v1 seed*: `bin/lib/cmd-install.sh:564-565`, `:599-600`. | Test `r-inst_25_los_nombres_los_enlaces_son_basename` verifies the requirement; it turns red if behavior changes. |
| **R-INST-26** | `ropero install engines` requires a TTY: without one, usage error (exit 2), because a menu that consumes EOF could choose on its own. | `crates/ropero-runtime/tests/preflight.rs`<br>*v1 seed*: `bin/lib/cmd-install.sh:375-448`, `:464-467`, `:495-505`. | Test `r-inst_26_ropero_install_engines_exige_tty_sin` checks the requirement; it turns red if behavior changes. |
| **R-INST-27** | A broken engine does not crash install: `_rp_engine_deps_ok` returns 1 with the missing dependency list, without `exit`. | `crates/ropero-runtime/tests/preflight.rs`<br>*v1 seed*: `bin/lib/03-engines.sh:171-184`. | Test `r-inst_27_engine_roto_tumba_install_rpenginedepsok_devuelve` verifies exit code; it turns red if behavior changes. |

### 4.10.1 TOML is the data

*v2 module*: `crates/ropero-core/src/model.rs` · *Test area*: `crates/ropero-core/tests/model.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-1** | The catalog consists of TOMLs; root is `<repo>/models`. | `crates/ropero-core/tests/model.rs`<br>*v1 seed*: `bin/python/modelconf.py:630-658`, `README.md:58-60`. | Test `r-mod_01_catalogo_son_los_toml_raiz_repo` checks the requirement against `crates/ropero-core/src/model.rs`; it turns red if behavior changes. |
| **R-MOD-2** | `bin/python/modelconf.py` is the schema source of truth (shell does not duplicate it). | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:4-6`. | Test `r-mod_02_bin_python_modelconf_fuente_verdad_del` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-3** | Shell consumes the catalog through one idempotent dump per process, with `0x1F` separator, 31 fields per record, and a format-version guard: reject if header is unexpected. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:6-7`, `:33-34`, `:159-185`. | Test `r-mod_03_shell_consume_catalogo_mediante_unico_dump` checks that invalid format yields an error; it turns red if behavior changes. |
| **R-MOD-4** | A malformed TOML does not crash the catalog: warn on stderr and skip it. | Same test file<br>*v1 seed*: `bin/python/modelconf.py:1176-1186`. | Test `r-mod_04_toml_roto_tumba_catalogo_avisa_por` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-5** | Duplicate names between active and `fired` are not an error: retain both rows and let the first win (active entries come first). | Same test file<br>*v1 seed*: `bin/python/modelconf.py:137-140`, `bin/lib/02-modelconf.sh:220-229`. | Test `r-mod_05_nombres_duplicados_entre_activo_fired_son` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-6** | The format declares where the artifact lives (plus table in doc A). | Same test file<br>*v1 seed*: `bin/lib/cmd-catalog.sh:389-410`. | Test `r-mod_06_formato_declara_donde_vive_artefacto` verifies the requirement; it turns red if behavior changes. |

### 4.10.2 Aliases

*v2 module*: `crates/ropero-core/src/catalog.rs` · *Test area*: `crates/ropero-core/tests/catalog.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-7** | Alias is declared in `[model].alias`. | `crates/ropero-core/tests/catalog.rs`<br>*v1 seed*: `models/_template.toml` (`[aliases]` section), `bin/python/modelconf.py:1549-1554`. | Test `r-mod_07_alias_declara_model_alias` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-8** | In memory, an alias becomes an extra row in the parallel arrays after real entries, copying its fields with `MODEL_ISALIAS=1` and `MODEL_REAL=<real>`. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:36-38`, `:193-214`. | Test `r-mod_08_memoria_alias_materializa_como_una_fila` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-9** | Alias collision is an error that names the conflict. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:202-206`. | Test `r-mod_09_choque_alias_error_que_nombra_conflicto` verifies the error; it turns red if behavior changes. |
| **R-MOD-10** | An alias behaves like its name: `rp_model_exists` accepts both, `rp_model_argv` translates alias → real, and `ropero path` always uses the real name for the directory. | Same test file<br>*v1 seed*: `bin/lib/02-modelconf.sh:231-235`, `:284-288`; `bin/lib/cmd-catalog.sh:364-372`. | Test `r-mod_10_alias_responde_igual_que_nombre_rpmodelexists` verifies the requirement; it turns red if behavior changes. |

### 4.10.3 stdout contract

*v2 module*: `crates/ropero-cli/src/run.rs` · *Test area*: `crates/ropero-cli/tests/cli.rs` · *Type*: cli

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-11** | stdout = data. | `crates/ropero-cli/tests/cli.rs`<br>*v1 seed*: `bin/lib/cmd-catalog.sh:11-15`, `:24-27`, `:91-92`; `bin/lib/00-utils.sh:5-8`. | Test `r-mod_11_stdout_datos` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-12** | `ropero levels` prints no headers, because any other stdout breaks completion. | Same test file<br>*v1 seed*: `bin/lib/cmd-catalog.sh:284-287`. | Test `r-mod_12_ropero_levels_imprime_cabeceras_porque_cualquier` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-13** | `ropero args` prints one argument per line and validates against TOML: undeclared level, flag, or option exits 2. | Same test file<br>*v1 seed*: `bin/lib/cmd-catalog.sh:221-282`. | Test `r-mod_13_ropero_args_imprime_argumento_por_linea` verifies the exit code; it turns red if behavior changes. |

### 4.10.4 Download: `hf` and only `hf`

*v2 module*: `crates/ropero-download/src/` · *Test area*: `crates/ropero-download/tests/download.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-14** | The only downloader is `hf` (Hugging Face CLI): no `curl` fallback, because `ROPERO_DOWNLOADER` was a test escape hatch. | `crates/ropero-download/tests/download.rs`<br>*v1 seed*: `bin/lib/05-download.sh:9-13`. | Test `r-mod_14_unico_downloader_huggingface_cli_hay_curl` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-15** | GGUF download uses its own staging directory: `mktemp -d "$dest_dir/.rp-dl.XXXXXX"` inside destination and on same filesystem, making `mv` atomic. | Same test file<br>*v1 seed*: `bin/lib/05-download.sh:95-99`, `:158-162`. | Test `r-mod_15_descarga_gguf_usa_staging_propio_mktemp` verifies atomicity; it turns red if behavior changes. |
| **R-MOD-16** | GGUF download sequence is exact: if final name exists and is nonempty, report “already downloaded” and return 0. | `crates/ropero-download/src/`<br>*Architecture change*: ignoring the empty-file detail can cost a 40 GB download.<br>*v1 seed*: `bin/lib/05-download.sh:123-199`. | Sequence matches doc A; a **zero-byte** file counts as not downloaded. |
| **R-MOD-17** | Safetensors checkpoint download deliberately does NOT use staging: `hf` resumes with fixed `--local-dir`; staging would prevent resuming a 12… (source text is truncated). | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:203-209`. | Test `r-mod_17_descarga_checkpoint_safetensors_usa_staging_proposito` verifies behavior; it turns red if behavior changes. |
| **R-MOD-18** | Verify downloads by actual content, not markers: GGUF final file exists and is nonempty (`[[ -f && -s ]]`). | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:20-22`, `:38-59`, `:126`, `:228-235`. | Test `r-mod_18_verificacion_una_descarga_por_contenido_real` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-19** | Re-verify even if `hf` exits 0. | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:285-296`. | Test `r-mod_19_aunque_salga_con_verifica` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-20** | No hash or checksum verification is performed anywhere. | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:91`, `:151`; `bin/lib/cmd-models.sh:478`, `:487`. | Test `r-mod_20_hay_verificacion_por_hash_por_checksum` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-21** | Reconstruct download state from disk: GGUF → final file presence. | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:21-22`, `:250-256`, `:286-298`. | Test `r-mod_21_estado_una_descarga_reconstruye_desde_disco` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-22** | `hf` output goes to stderr so an install with selection does not “look like it is spewing junk instead of a model.” | Same test area<br>*v1 seed*: `bin/lib/05-download.sh:277-279`. | Test `r-mod_22_salida_stderr_para_que_install_con` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-23** | `[download.aux.*]` auxiliaries download to the same directory with `use=run` filter (those referenced at launch: `${mtp}`, `${mmproj}`, `${chat_template}`). | Same test area<br>*v1 seed*: `bin/lib/cmd-models.sh:321-336`, `:515-517`. | Test `r-mod_23_los_auxiliares_download_aux_bajan_mismo` verifies the requirement; it turns red if behavior changes. |

### 4.10.5 Reuse across models

*v2 module*: `crates/ropero-download/src/reuse.rs` · *Test area*: `crates/ropero-download/tests/reuse.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-24** | If two TOMLs request the same repo and filename, the second model links to the first model's directory instead of downloading again. | `crates/ropero-download/tests/reuse.rs`<br>*v1 seed*: `bin/lib/cmd-models.sh:338-435`, `:346-350`. | Test `r-mod_24_dos_tomls_piden_mismo_repo_mismo` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-25** | Respect a hand-created symlink (do not touch it). | Same test file<br>*v1 seed*: `bin/lib/cmd-models.sh:396-426`. | Test `r-mod_25_symlink_hecho_mano_respeta_toca` verifies the requirement; it turns red if behavior changes. |

### 4.10.6 Download decision

*v2 module*: `crates/ropero-download/src/plan.rs` · *Test area*: `crates/ropero-download/tests/plan.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-MOD-26** | `ropero models` requires a TTY unless selection is explicit. | `crates/ropero-download/tests/plan.rs`<br>*v1 seed*: `bin/lib/cmd-models.sh:641-662`, `:688-690`, `:730-737`. | Test `r-mod_26_ropero_models_exige_tty_sin_seleccion` checks the requirement; it turns red if behavior changes. |
| **R-MOD-27** | Choosing a model consents to its download: set `RP_YES=1` in-process; do not repeat size prompt. | Same test file<br>*v1 seed*: `bin/lib/cmd-models.sh:19-20`, `:500`. | Test `r-mod_27_elegir_modelo_consentir_descarga_dentro_del` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-29** | A model may declare both paths (`[download]` and `[build]`). | Same test file<br>*v1 seed*: `bin/lib/cmd-models.sh:696-723`, `:730-737`. | Test `r-mod_29_modelo_puede_declarar_las_dos_vias` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-30** | In build path, if safetensors repo is already quantized, skip quantization and generate GGUF directly. | Same test file<br>*v1 seed*: `bin/lib/cmd-gguffer.sh:439-483` (quantization plan). | Test `r-mod_30_via_build_repo_safetensors_viene_cuantizado` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-31** | `ropero gguffer` is the manual equivalent of `ropero install models --prio build`: downloads safetensors repo, creates GGUF, and quantizes it. | Same test file<br>*v1 seed*: `bin/lib/cmd-gguffer.sh:1-15`, `:1271-1327`. | Test `r-mod_31_ropero_gguffer_equivalente_manual_ropero_install` verifies the requirement; it turns red if behavior changes. |
| **R-MOD-28** | Engines with their own stores (FLM, Strata) decide themselves: if engine defines optional `install_model` hook, it takes precedence and skips download/build path. | Same test file<br>*v1 seed*: `bin/lib/cmd-models.sh:437-455`, `bin/lib/engines/fastflowlm.sh:188-259`, `bin/lib/engines/strata.sh:188-237`. | Test `r-mod_28_los_motores_con_almacen_propio_flm` verifies the requirement; it turns red if behavior changes. |

### 4.10.7 The `--prio` contract

A model **may have `[download]`, `[build]`, or both**: both is a valid TOML state (§ 4.1.1). Path selection happens in `ropero install models`, and `--prio` is the way to **avoid prompting**.

This section is the normative `--prio` contract: doc A (R-MOD-29) and the CLI document (§ 9.3) refer here.

**Value**: `--prio=download` or `--prio=build`, and only those values. Any other value (e.g. `--prio=fast`) is a **usage error (exit 2)** and the message **includes the received value**.

**Scope**: `--prio` can be **global** (the whole `ropero install models` run) **or per model**.

**Global syntax**: global `--prio` comes **before** the model list:

```
ropero install models --prio=build nemotron groot
```

**Per-model syntax**: `--prio` goes **after** the model it applies to, and its scope **ends at the next model**. Canonical example:

```
ropero install models nemotron groot --prio=build qwen --prio=download
```

This means exactly:

| Model | `--prio` | Consequence |
|---|---|---|
| `nemotron` | (none) | Process **what it has**: if it declares both paths, ask the user; if only one, use it. |
| `groot` | `build` | Use its `build` path. |
| `qwen` | `download` | Use its `download` path. |

**Precedence**: if both a global and a per-model `--prio` exist, the per-model value **wins** for that model; the global value applies only to models without their own `--prio`. In

```
ropero install models --prio=build nemotron groot --prio=download
```

`nemotron` uses `build` (global), and `groot` uses `download` (its own).

**When `--prio` is ignored** (not an error; does not abort execution):

- If the model **does not have** that path (e.g. `--prio=build` for a model declaring only `[download]`), `--prio` is **ignored**.
- If the model has **only one path** and that path is specified, it is **ignored**: that is already the default, so nothing changes.

An ignored `--prio` is **not** an error and does **not** abort: the **other** models in the list are still installed and the exit code reflects the complete installation result.

**With both paths and no `--prio`**: ask the user which path to take. Without a TTY, this is a **usage error (exit 2)** handled by the commands, not the validator; the message says **what to do**: pass `--prio`, or provide a list where no model has both paths.

**What v1 did and what v2 changes** (so F5 does not copy it unchanged):

- v1 `--prio` was **global only**: one value for the whole invocation (`ropero-code/bin/lib/cmd-models.sh:603`, `:623-628`). v2 adds **per-model** scope and precedence between both.
- v1 validated `--prio` while parsing (`ropero-code/bin/lib/cmd-models.sh:623-628`), as v2 does, but its message did not include the received value.
- v1 also had `--download`/`--build` flags that forced a path and were **usage errors** if they contradicted TOML (`ropero-code/bin/lib/cmd-models.sh:730-737`): these are **different** from `--prio`, which is ignored.
- v1's “priority unavailable” branch (`ropero-code/bin/lib/cmd-models.sh:711`) was **unreachable**: with a valid value, a one-path model always followed that path (`ropero-code/bin/lib/cmd-models.sh:725-728`). v2 is unambiguous: **it is ignored**.

> **Contract pending implementation.** Current v2 does not yet have `ropero install models` (arrives in F5/T-25). This section fixes its future behavior; `r-mod_29_modelo_puede_declarar_las_dos_vias` in `crates/ropero-download/tests/plan.rs` verifies it.

### 4.11.1 What the test suite is

*v2 module*: `test/` · *Test area*: `test/` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-TST-1** | The suite is an executable specification at two levels. | `test/`<br>*v1 seed*: call-site count in `test/test-*.sh`; breakdown by file in § 5.2. | Test `r-tst_01_bateria_una_especificacion_ejecutable_dos_niveles` verifies the requirement against `test/`; it turns red if behavior changes. |
| **R-TST-2** | Wrapper exit codes distinguish only 0 (all five suites pass), 1 (one failed), and 2 (cannot start). | `test/`<br>*v1 seed*: `test/lib/test-common.sh:499-565`, `docs/juez.md`. | Test `r-tst_02_exit_code_los_wrappers_solo_distingue` checks the exit code; it turns red if behavior changes. |
| **R-TST-3** | No test automatically evaluates model response content: there is no `assert` on response quality, only on the artifact (nonempty JSON). | `test/`<br>*v1 seed*: `test/lib/test-common.sh:401-437`. | Test `r-tst_03_contenido_las_respuestas_del_modelo_evalua` verifies this; it turns red if behavior changes. |

### 4.11.2 Shared runner

*v2 module*: `test/lib/` · *Test area*: `test/lib/` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-TST-4** | Runner resolves paths relative to its own script, not `cwd` or `ROPERO_ROOT`; tests can run from anywhere. | `test/lib/`<br>*v1 seed*: `test/lib/test-common.sh:37-38`. | Test `r-tst_04_runner_resuelve_sus_rutas_desde_ubicacion` verifies the requirement; it turns red if behavior changes. |
| **R-TST-5** | Runner creates its own hermetic provider (`ropero-local`, OpenAI-compatible) at `127.0.0.1:<port>/v1` and neither reads nor touches user's global opencode config. | Same area<br>*v1 seed*: `test/lib/test-common.sh:20-27`, `:147-183`, `:288-297`. | Test `r-tst_05_runner_monta_provider_hermetico_propio_ropero` verifies the result; it turns red if behavior changes. |
| **R-TST-6** | Resolve API key after model launch, in order: `LLAMA_API_KEY` > `$STATE_DIR/api_key` > `OPENAI_API_KEY` > `sk-local-dev-key`. | Same area<br>*v1 seed*: `test/lib/test-common.sh:85-100`, `:83`, `:181`, `:459`. | Test `r-tst_06_api_key_resuelve_despues_del_arranque` verifies the requirement; it turns red if behavior changes. |
| **R-TST-7** | Test model context and `output` derive from TOML `context`: `output = context − 1024`, clamped to `[4096, 65536]`, default context 32768. | Same area<br>*v1 seed*: `test/lib/test-common.sh:114-128` (measured: groot → context 65536 / output 64512). | Test `r-tst_07_contexto_output_del_modelo_bajo_prueba` verifies the requirement; it turns red if behavior changes. |
| **R-TST-8** | Before launching opencode, runner probes engine using `GET /v1/models` + `Authorization: Bearer <key>`; if it does not respond, abort with a clear message. | Same area<br>*v1 seed*: `test/lib/test-common.sh:189-203`, `:394-398`. | Test `r-tst_08_runner_hace_probe_del_engine_con` verifies the requirement; it turns red if behavior changes. |
| **R-TST-9** | Runner invokes exactly `opencode run <prompt> --agent build --model ropero-local/<model> --format json --standalone`, stdout to JSON and stderr to its own file, never… (source truncates here). | Same area<br>*v1 seed*: `test/lib/test-common.sh:401-403` (argv verified at runtime). | Test `r-tst_09_runner_invoca_exactamente_opencode_run_prompt` verifies the prohibition; it turns red if behavior changes. |
| **R-TST-10** | A suite failure does not abort the others: record it and continue. | Same area<br>*v1 seed*: `test/lib/test-common.sh:541-565`, `:213`, `:216`. | Test `r-tst_10_fallo_una_bateria_aborta_las_demas` verifies the requirement; it turns red if behavior changes. |
| **R-TST-11** | Runner produces `recopilacion-completa.md` containing each artifact's CONTENT (engine log, JSON, exported session, stderr), not just paths. | Same area<br>*v1 seed*: `test/lib/test-common.sh:312-332`, `:445-492`. | Test `r-tst_11_runner_produce_recopilacion_completa_con_contenido` verifies the requirement; it turns red if behavior changes. |
| **R-TST-12** | Refusal to start exits 2, not 1: model missing; `jq`/`opencode`/`ropero` missing; output directory cannot be created; or isolated environment cannot be prepared. | Same area<br>*v1 seed*: `test/lib/test-common.sh:501-528`, `:530-539`. | Test `r-tst_12_negativa_empezar_exit_falta_modelo_falta` checks exit code; it turns red if behavior changes. |

### 4.11.3 Fakes

*v2 module*: `test/fakes/` · *Test area*: `test/fakes/` · *Type*: e2e

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-TST-13** | Fakes in `test/fakes/` are essential: they verify the contract without GPU, network, or touching user state. | `test/fakes/`<br>*v1 seed*: `test/fakes/{pkill,curl,ropero,opencode}`. | Test `r-tst_13_los_dobles_test_fakes_son_adorno` verifies the requirement; it turns red if behavior changes. |
| **R-TST-14** | Fake `llama-quantize` enforces the engine's real constraint: requantization fails with literal message `requantizing from type X is disabled`. | Same area<br>*v1 seed*: `test/fakes/gguffer/llama-quantize:121-129`. | Test `r-tst_14_doble_llama_quantize_impone_restriccion_real` verifies the requirement; it turns red if behavior changes. |
| **R-TST-15** | Synthetic GGUF (`gguf_fake.py`) lets real `ggufcheck`, not another fake, verify fake quantizer output; this gives credibility to the… (source truncates here). | Same area<br>*v1 seed*: `test/fakes/gguffer/gguf_fake.py`, `test/test-gguffer.sh:178`. | Test `r-tst_15_gguf_sintetico_gguffake_permite_que_resultado` verifies the requirement; it turns red if behavior changes. |
| **R-TST-16** | Offline suite with temporary `HOME` and fakes on PATH runs all 5/5 suites successfully in ~2 s, exit 0, without GPU, network, or killing anything. | Same area<br>*v1 seed*: `test/test-groot.sh` + fakes on PATH. | Test `r-tst_16_suite_offline_ejecutada_con_home_temporal` checks exit code; it turns red if behavior changes. |
| **R-TST-17** | Process termination uses an explicit closed list: `llama-server`, `llama-server-official`, `opencode`, `mcode`, `claude`. | Same area<br>*v1 seed*: `test/lib/test-common.sh:206-219`, `test/fakes/pkill:6-10`. | Test `r-tst_17_baja_procesos_una_lista_cerrada_explicita` verifies the list; it turns red if behavior changes. |

### 4.11.4 What the suite does NOT guarantee — three measured wounds

*v2 module*: `crates/ropero-core/tests/` · *Test area*: `crates/ropero-server/tests/` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-TST-18** | 🔴 Endpoint auth guard passes even when auth is broken. Empirically verified: changing `authed()` in a copy to always return `True` still made Python block report… (source truncates). | `crates/ropero-server/tests/`<br>*v1 seed*: `test/test-endpoint-auth.sh:31-106`, `:204-208`. | Test `r-tst_18_guard_auth_del_endpoint_pasa_verde` checks exit code; it turns red if behavior changes. |
| **R-TST-19** | 🟠 gguffer test is not hermetic: depends on host `transformers`, because a real pre-check runs before the fake converter. | `crates/ropero-core/tests/`<br>*Architecture change*: Wound #1, false green; reason for rewriting endpoint in Rust.<br>*v1 seed*: `bin/lib/cmd-gguffer.sh:1961-1980`, `test/test-gguffer.sh:16-17`. | Auth test calls the **same function** the server uses and asserts its result; it does not capture a subprocess `rc`. Removing key check fails the test. |
| **R-TST-20** | 🟠 No CI runner or documented command runs the full suite; 16 scripts must be launched by hand. | `crates/ropero-server/tests/`<br>*v1 seed*: `README.md:174`, `:192`, `CHANGELOG.md:205`. | Test `r-tst_20_hay_runner_comando_documentado_que_ejecute` verifies the requirement; it turns red if behavior changes. |
| **R-TST-21** | 🟠 Fake logs are written but never asserted: five root fakes record argv in `$HOME/.fake-*.log`, and no test reads them (grep for `FAKE_LOG`/`.fake-` → no matches). | Same area<br>*v1 seed*: `test/fakes/README.md:50-51`. | Test `r-tst_21_los_logs_los_dobles_escriben_pero` verifies the requirement; it turns red if behavior changes. |
| **R-TST-22** | 🟠 Level resolution (`[levels]`) has no test. Catalog defines levels per model and `rp_model_argv <model> [level]` resolves them, but no test verifies… (source truncates). | `crates/ropero-core/tests/`<br>*Architecture change*: Wound #2: level resolution had no tests.<br>*v1 seed*: `models/llama/groot.toml` (`[levels]`), `bin/lib/02-modelconf.sh:269-312`. | Test verifies requested level reaches argv, undeclared level fails with exit 2, and default level applies when none is requested. |
| **R-TST-23** | 🟠 21 source files are not touched by any test, including `cmd-catalog.sh`, `cmd-doctor.sh`, `cmd-install.sh`, `cmd-runtime.sh`, `cmd-gguffer.sh`, `router.sh`, and six… (source truncates). | `crates/ropero-core/tests/`<br>*Architecture change*: Wound #3: key readable in argv by any host user.<br>*v1 seed*: grep over `test/*.sh` → no matches. | Test verifies the key does **not** appear in engine argv by inspecting real `/proc/<pid>/cmdline`. |
| **R-TST-24** | 🟡 CHANGELOG describes an obsolete contract contradicted by test: says FLM passes `--api-key` to endpoint and “refuses to start without `LLAMA_API_KEY`”. | `crates/ropero-server/tests/`<br>*v1 seed*: `CHANGELOG.md:98-102` vs `test/test-endpoint-auth.sh:116-122`, `:199-202`. | Test `r-tst_24_changelog_describe_contrato_caducado_que_test` verifies the requirement; it turns red if behavior changes. |
| **R-TST-25** | 🟡 Two scripts depend on environment more than expected: `test-flm-decision.sh` and `test-models-symlink.sh` abort with `LLAMA_API_KEY: unbound variable` if not exported (`set…`). | `crates/ropero-server/tests/`<br>*v1 seed*: `bin/lib/01-env.sh:60`, `test/lib/test-common.sh:522-528`. | Test `r-tst_25_dos_scripts_dependent_del_entorno_mas` verifies requirement; it turns red if behavior changes. |
| **R-TST-26** | 🟡 bash 3.2 compatibility is a claim, not a verification: README and 11 script headers say so, but suite never runs bash 3.2… (source truncates). | Same area<br>*v1 seed*: `README.md:200-202`, `test/bench-motor.sh:411`. | Test `r-tst_26_compatibilidad_con_bash_una_declaracion_una` verifies the requirement; it turns red if behavior changes. |
| **R-TST-27** | 🟡 `shellcheck` is absent from the suite despite being documented in README as a development requirement. | Same area<br>*v1 seed*: `README.md:194`. | Test `r-tst_27_shellcheck_esta_bateria_pese_que_readme` verifies the requirement; it turns red if behavior changes. |
| **R-TST-28** | 🟡 Two scripts return nonstandard codes: `test-strata.sh` exits with failure count (3 failures → exit 3, not 1); `test-endpoint-auth.sh` returns 0/1 but does not propagate the failure of… (source truncates). | Same area<br>*v1 seed*: `test/test-strata.sh:218-223`, `test/test-endpoint-auth.sh:204-208`. | Test `r-tst_28_dos_scripts_devuelven_codigos_estandar_test` checks exit code; it turns red if behavior changes. |

### 4.12 opencode integration

*v2 module*: `opencode-plugin/` · *Test area*: `crates/ropero-cli/tests/opencode.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-OC-1** | Plugin is `opencode-plugin/index.ts`, a TypeScript plugin (`@opencode/plugin`) with `id: "ropero-modelo"`. | `crates/ropero-cli/tests/opencode.rs`<br>*v1 seed*: `opencode-plugin/index.ts:4-40`, `:146-147`. | Test `r-oc_01_plugin_opencode_plugin_index_plugin_typescript` verifies the requirement against `opencode-plugin/`; it turns red if behavior changes. |
| **R-OC-2** | Install plugin as a copy, not symlink—the reverse of other Ropero installs, because opencode's inotify watcher does not follow symlinks, so changes… (source truncates). | Same test file<br>*v1 seed*: `bin/lib/cmd-install.sh:688-691`, `:710-712`, `:722-732`. | Test `r-oc_02_plugin_instala_como_copia_como_symlink` verifies the requirement; it turns red if behavior changes. |
| **R-OC-3** | Installer does not register plugin: at the end, checks whether opencode config declares `"ropero-modelo"`; if not, warns and prints snippet. | Same test file<br>*v1 seed*: `bin/lib/cmd-install.sh:741-753`, `opencode-plugin/README.md:14`. | Test `r-oc_03_instalador_registra_plugin_final_comprueba_config` verifies the requirement; it turns red if behavior changes. |
| **R-OC-4** | The plugin has three options (plus table in doc A). | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:42-57`, `:151`, `:164`, `:271-279`. | Test `r-oc_04_las_tres_opciones_del_plugin` verifies the requirement; it turns red if behavior changes. |
| **R-OC-5** | The three hooks used and when (plus table in doc A). | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:215-281`, `opencode-plugin/README.md:96-103`. | Test `r-oc_05_los_tres_hooks_usados_cuando` verifies the requirement; it turns red if behavior changes. |
| **R-OC-6** | Plugin filters: queries cached `ropero list` and does nothing if `modelID` is not a Ropero model—“the server is already serving whatever it serves, and adding Ropero only…” (source truncates). | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:76-101`, `:179-191`. | Test `r-oc_06_plugin_filtra_consulta_ropero_list_cacheado` verifies the requirement; it turns red if behavior changes. |
| **R-OC-7** | If `ropero use` fails, hook throws and opencode request fails. | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:167-172`, `opencode-plugin/README.md:60-62`. | Test `r-oc_07_ropero_use_falla_hook_lanza_peticion` verifies the requirement; it turns red if behavior changes. |
| **R-OC-8** | Pass opencode `variant` as `--level` only when it exactly matches a level declared by the model (`ropero levels <model>`). | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:230-232`. | Test `r-oc_08_variant_opencode_pasa_como_nivel_solo` verifies the requirement; it turns red if behavior changes. |
| **R-OC-9** | Deduplication uses `servido` + `enCurso` to avoid launching two `ropero use` calls for consecutive requests. | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:153-160`, `:192-210`. | Test `r-oc_09_deduplicacion_usa_servido_encurso_para_lanzar` verifies the requirement; it turns red if behavior changes. |
| **R-OC-10** | Stable CLI contract has three parts: `list` (two spaces + name), `levels` (one per line), and `use` (last line `ROPERO_PORT=<n>`). | Same test file. | Test `r-oc_10_contrato_estable_con_cli_tres_cosas` verifies the requirement; it turns red if behavior changes. |
| **R-OC-11** | Plugin default `provider` is `"marichocho"`, which does not exist in the owner's current opencode config. | Same test file<br>*v1 seed*: `opencode-plugin/index.ts:151`. | Test `r-oc_11_provider_por_defecto_del_plugin_marichocho` verifies the requirement; it turns red if behavior changes. |

### 4.13.1 What Strata is, and why Ropero handles it separately

*v2 module*: `crates/ropero-runtime/src/engines/strata.rs` · *Test area*: `crates/ropero-runtime/tests/strata.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-STRATA-1** | Strata is not a model server: it is a *terminal* that downloads an engine, prepares artifacts, and leaves the model on disk. | `crates/ropero-runtime/tests/strata.rs`<br>*v1 seed*: `test/test-strata.sh:5-16`, `:102-109`. | Test `r-strata_01_strata_servidor_modelos_terminal_que_baja` verifies required writes against `crates/ropero-runtime/src/engines/strata.rs`; it turns red if behavior changes. |
| **R-STRATA-2** | A Strata model in catalog points to Strata's catalog: `[model].path = "<family>/<package>"` (e.g. …; source truncates). | Same test file<br>*v1 seed*: `bin/lib/engines/strata.sh:160-176`; `models/strata/gyro-m.toml:22-25`. | Test `r-strata_02_modelo_strata_catalogo_puntero_catalogo_strata` verifies the requirement; it turns red if behavior changes. |

### 4.13.2 Strata preparation and script behavior

The requirements in this section preserve the separately tested `groot-strata.sh` behavior and the integration gaps identified by the source specification.

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-STRATA-3** | Ropero does not download the files: Strata downloads them at launch. | `crates/ropero-runtime/tests/strata.rs`<br>*v1 seed*: `bin/lib/engines/strata.sh:188-237`; `docs/strata.md`. | Test `r-strata_03_los_ficheros_los_baja_ropero_los` verifies behavior against `crates/ropero-runtime/src/engines/strata.rs`; it turns red if behavior changes. |
| **R-STRATA-4** | Strata installation is official: download installer to a temp file (never `curl | sh`) and run `env STRATA_BIN_DIR=$BIN_DIR sh $tmp`. | Same test area<br>*v1 seed*: `bin/lib/engines/strata.sh:87-111`. | Test `r-strata_04_instalacion_strata_oficial_descarga_instalador_fichero` checks the prohibited behavior; it turns red if behavior changes. |
| **R-STRATA-5** | `resolve` precedence is PATH → `$BIN_DIR` → `~/.local/bin/strata` → `~/.local/share/strata/terminal/current/bin/strata`—the active launcher, not newest version… (source truncates). | Same test area<br>*v1 seed*: `bin/lib/engines/strata.sh:124-139`. | Test `r-strata_05_precedencia_resolve_path_bindir_local_bin` verifies the requirement; it turns red if behavior changes. |
| **R-STRATA-6** | Probe distinguishes `missing` (Strata absent), `down` (Strata present but port not serving), and `ok`. | Same test area<br>*v1 seed*: `bin/lib/engines/strata.sh:141-154`. | Test `r-strata_06_probe_distingue_tres_estados_missing_strata` verifies the requirement; it turns red if behavior changes. |
| **R-STRATA-7** | Strata does not provide auth (`native_auth` → `no`): Ropero proxy supplies key. | Same test area<br>*v1 seed*: `bin/lib/engines/strata.sh:157-163`. | Test `r-strata_07_strata_hace_auth_nativeauth_clave_pone` verifies the requirement; it turns red if behavior changes. |
| **R-STRATA-8** | Ropero launch returns PID, while supervisor is another process: `strata launch --no-tui` returns when server is serving. | Same test area<br>*v1 seed*: `test/test-strata.sh:160-161`; `bin/server/strata-run.sh:14-30`. | Test `r-strata_08_launch_ropero_devuelve_pid_proceso_supervisor` verifies result; it turns red if behavior changes. |
| **R-STRATA-9** | `strata-run.sh` (1) launches Strata and waits (download can be 70 GB), (2) stays alive while model serves, (3) on SIGTERM/SIGINT stops… (source truncates). | Same test area<br>*v1 seed*: `bin/server/strata-run.sh:14-30`, `:100-160`. | Test `r-strata_09_puente_strata_run_lanza_strata_espera` verifies behavior; it turns red if behavior changes. |
| **R-STRATA-10** | Save launch with `--save-preset <name>` so only its process can be stopped via `strata launch --preset <name> --stop`; bare `strata launch --stop` stops all… (source truncates). | Same test area<br>*v1 seed*: `bin/server/strata-run.sh:36-48`, `:100-110`. | Test `r-strata_10_para_poder_parar_solo_suyo_launch` verifies writes; it turns red if behavior changes. |
| **R-STRATA-11** | Bridge checks every 15 s that launch still runs (`strata launch --status --output json`); if Strata stopped it or engine died, wrapper exits with error so Ropero… (source truncates). | Same test area<br>*v1 seed*: `bin/server/strata-run.sh:130-160`. | Test `r-strata_11_puente_comprueba_cada_que_launch_sigue` verifies error reporting; it turns red if behavior changes. |
| **R-STRATA-12** | Ropero launch passes `--backend strata`, `--backend-bin <binary>`, `--model <family/package>`, Ropero published ID (`strata-qwen38-q2`), model context, and… (source truncates). | Same test area<br>*v1 seed*: `test/test-strata.sh:169-177`. | Test `r-strata_12_launch_ropero_pasa_backend_strata_backend` verifies the requirement; it turns red if behavior changes. |
| **R-STRATA-13** | `--set` prefix passes valued options to Strata: `ropero use strata-qwen36-q4 --vram-reserve 1344` becomes `--set vram_reserve_mib=1344`. | Same test area<br>*v1 seed*: `docs/strata.md:132-137`; `models/strata/gyro-m.toml:56-60`. | Test `r-strata_13_prefijo_set_mecanismo_con_que_pasan` verifies the requirement; it turns red if behavior changes. |
| **R-STRATA-14** 🔴 | Ropero's engine launches Strata with `strata launch`, which runs through `setup.py` again. | `crates/ropero-forge/tests/strata.rs`<br>*v1 seed*: `groot-strata.sh:15-18`, `:184-205`. | Test `r-strata_14_motor_ropero_lanza_strata_con_strata` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-15** 🔴 | Every GGUF needs its own pack, not the pack of an already installed model. | Same test area<br>*v1 seed*: `groot-strata.sh:260-271`. | Test `r-strata_15_cada_gguf_necesita_propio_pack_del` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-16** | Generate a synthetic, neutral expert profile (not activation-calibrated), with round-robin ranking by layer: layer 0 exp0, layer 1 exp0, … layer 47 exp0, layer 0 exp1, … | Same test area<br>*v1 seed*: `groot-strata.sh:184-205`, `:206-258`. | Test `r-strata_16_profile_expertos_propio_genera_sintetico_neutro` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-17** | Script does more than launch: complete cycle `prepare` → `tune` → `launch`, each for a reason; prepare creates directories, downloads and verifies (`strata models…`; source truncates). | Same test area<br>*v1 seed*: `groot-strata.sh:5-11`, `:363-399`, `:402-413`, `:449-473`. | Test `r-strata_17_script_limita_lanzar_ciclo_completo_preparar` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-18** | Invoke Strata `setup.py` directly with `STRATA_CONTROL_INSTALL=1` and a per-model private `STRATA_INSTALL_DIR`, because `setup.py` uses `CONTROL` to direct all… (source truncates). | Same test area<br>*v1 seed*: `groot-strata.sh:160-182`. | Test `r-strata_18_setup_strata_invoca_directamente_con_stratacontrolinstall` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-19** | Args patch rebuilds list preserving current tuning options and replacing only pack, profile and reserve (including duplicates). | Same test area<br>*v1 seed*: `groot-strata.sh:301-354`. | Test `r-strata_19_parche_args_reconstruye_lista_conservando_las` verifies returned list; it turns red if behavior changes. |
| **R-STRATA-20** | `tune` requires free GPU and verifies result through JSON, not exit code: require `ok is True`, `outcome` ∈ `{improved, default_confirmed}`, and present `preset.id`. | Same test area<br>*v1 seed*: `groot-strata.sh:363-399`. | Test `r-strata_20_afinar_exige_gpu_libre_verifica_resultado` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-21** | Free-GPU check is deliberately strict: tolerates trivial use (≤500 MiB per process—desktop may appear as CUDA process with a few MiB), but any… (source truncates). | Same test area<br>*v1 seed*: `groot-strata.sh:100-125`. | Test `r-strata_21_comprobacion_gpu_libre_estricta_proposito_tolera` verifies prohibited behavior; it turns red if behavior changes. |
| **R-STRATA-22** | Script `stop` refuses to stop processes: “this script does not record engine ownership…” (source truncates). | Same test area<br>*v1 seed*: `groot-strata.sh:96-98`. | Test `r-strata_22_parar_del_script_niega_parar_procesos` verifies requirement; it turns red if behavior changes. |

### 4.13.3 Requirements imposed by integration

*v2 module*: `crates/ropero-forge/src/strata.rs` · *Test area*: `crates/ropero-forge/tests/strata.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-STRATA-23** | 🔴 Strata engine needs a second launch mode: `strata serve --profile <run-config>` as well as current `strata launch --family … --model …`. | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_23_engine_strata_necesita_segundo_modo_lanzamiento` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-24** | 🔴 `[model].path` needs a second meaning for Strata: today it means `<family>/<package>` in Strata catalog. | Same test area | Test `r-strata_24_model_path_necesita_segundo_significado_para` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-25** | 🔴 Prepared artifact needs a place in Ropero catalog (plus table in doc A). | Same test area | Test `r-strata_25_artefacto_preparado_necesita_sitio_catalogo_ropero` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-26** | Public GGUF name is consequential: script renames both models to `Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` because “setup.py only finds that name”. | Same test area | Test `r-strata_26_nombre_publico_del_gguf_una_decision` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-27** | `tune` should be first-class Ropero operation: `ropero tune <model> [--mode quick|thorough]`, measuring machine and saving preset, with same verification… (source truncates). | Same test area | Test `r-strata_27_tune_debe_entrar_ropero_como_una` verifies writes; it turns red if behavior changes. |
| **R-STRATA-28** | `vram-reserve` option is declared but dead: present in `[options].list` and its `[args.option.vram-reserve]` block is commented in both Strata TOMLs. | Same test area<br>*v1 seed*: `models/strata/gyro-m.toml:29`, `:56-60`; `docs/strata.md:132-137`; `groot-strata.sh:343-344`. | Test `r-strata_28_opcion_vram_reserve_esta_declarada_pero` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-29** | VRAM reserve (1344 MiB) is a value measured on a specific machine (MSI Stealth AI+ A16, RTX 5080 Laptop). | Same test area | Test `r-strata_29_reserva_vram_1344_mib_valor_medido` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-30** | Pack preparation requires `llama.cpp` with `gguf-py` already installed in Strata tree; script does not download it automatically (“Install it first and run again…”). | Same test area | Test `r-strata_30_preparacion_del_pack_necesita_llama_cpp` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-31** | `setup.py` isolation requires private install directory not be a symlink and not match shared `INSTALL_DIR`. | Same test area<br>*v1 seed*: `groot-strata.sh:168-170`. | Test `r-strata_31_aislamiento_del_setup_exige_que_directorio` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-32** | Script state is on its own disk, not Ropero's: `$RAIZ/<id>/tune.json`, `tune.log`, `prepare-pack.log`, and run config in… (source truncates). | Same test area | Test `r-strata_32_estado_del_script_propio_disco_ropero` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-33** | Script's free-GPU check is stricter than Ropero's, and both are reasonable in context: Ropero is idempotent (if model already runs on port, it does not touch it—…). | Same test area | Test `r-strata_33_comprobacion_gpu_libre_del_script_mas` checks rejection; it turns red if behavior changes. |
| **R-STRATA-34** | Script is not portable to macOS for three concrete reasons, and Ropero has equivalent hard constraint: `readlink -f` (Ropero has `real_dir`/`real_path`/`_rp_link_real`…). | Same test area<br>*v1 seed*: `groot-strata.sh:48`, `:140`, `:288`; `install.sh:61-66`; `bin/lib/00-utils.sh`. | Test `r-strata_34_script_portable_macos_por_tres_razones` verifies requirement; it turns red if behavior changes. |
| **R-STRATA-35** | Script `status` uses `mktemp` + temp file for `strata models list --installed --output json`; measured reason: in a pipeline, if Python reader… (source truncates). | Same test area<br>*v1 seed*: `groot-strata.sh:422-428`. | Test `r-strata_35_estado_del_script_usa_mktemp_fichero` verifies requirement; it turns red if behavior changes. |

### 4.13.5 Target state

*v2 module*: `crates/ropero-forge/src/strata.rs` · *Test area*: `crates/ropero-forge/tests/strata.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-STRATA-36** 🔶 | Strata is a first-class engine. Goal: `ropero <model>` serves it like other engines without user needing to know Strata is special. | `crates/ropero-forge/src/strata.rs`<br>*Architecture change*: doc A marks this as target state. | `ropero install engines strata` makes Strata first-class: manifest declares `maturity`, its `install`, and `launch.kind = "custom"`. |
| **R-STRATA-37** 🔶 | Preparation is the missing part: three things the script does manually—GGUF-specific pack (R-STRATA-15), model-specific expert profile (R-STRATA-16), and measured tune… (source truncates). | Same module<br>*Architecture change*: closes measured gap where `strata launch` rewrites run config with wrong profile. | Strata launch generates model-specific pack and expert profile, not Flash-Next's; test verifies generated profile has Gyro-M's 48×256 shape. |
| **R-STRATA-38** 🔶 | Script is manual path, not integrated path. Its role here is to specify Ropero's future contract: profile per GGUF, pack per… (source truncates). | Same module<br>*Architecture change*: `groot-strata.sh` is manual path specifying contract. | Tests compare Ropero argv and generated files against script output for both models. |

### 4.14.1 What it is

*v2 module*: `crates/ropero-forge/src/lib.rs` · *Test area*: `crates/ropero-forge/tests/forge.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-1** | `ropero gguffer` is the only implementation for building GGUFs (download → conversion → imatrix → quantization → verification → model card). | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1-15. | Test `r-gguf_01_ropero_gguffer_unica_implementacion_fabricacion_ggufs` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/lib.rs`; it turns red if behavior changes. |
| **R-GGUF-2** | gguffer does not execute TOML: it reads fields and assembles argv like the rest of the system. | `crates/ropero-forge/tests/forge.rs` | Test `r-gguf_02_gguffer_ejecuta_toml_lee_sus_campos` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/lib.rs`; it turns red if behavior changes. |
| **R-GGUF-59** 🔶 | gguffer has a unique precondition: an available llama.cpp engine (official or fork) is needed to convert weights, quantize, and merge… (source truncates). | `crates/ropero-forge/src/lib.rs`<br>*Architecture change*: Principle: an undocumented precondition is one discovered by failing. | gguffer is the only command with an environment precondition, and the command's own help declares it (`ropero gguffer --help` says so). |

### 4.14.2 CLI contract

*v2 module*: `crates/ropero-cli/src/gguffer.rs` · *Test area*: `crates/ropero-cli/tests/cli.rs` · *Type*: cli

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-3** | Exactly one of three sources: `--path <dir>` (already cloned repo), `--repo <owner/name>` (clone), or `--gguf <file\|owner/repo>` (existing GGUF). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1418. | Test `r-gguf_03_fuente_una_tres_son_excluyentes_path` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-4** | Two distinct output options: `--out NAME\|PATH` (name/folder/path) and `--out-principal PATH` (exact path of first quant in run). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:27-32, :90-121, :1466-1485. | Test `r-gguf_04_las_opciones_salida_son_dos_distintas` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-5** | With several quants, `--out` cannot be an exact file path; it must be a name or directory. | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1466-1479. | Test `r-gguf_05_con_varios_quants_out_puede_ser` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-6** | Imatrix rules are strict and checked first: local `--imatrix` is exclusive with pair `--imatrix-repo`/`--imatrix-file`. | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:45-48, :1350-1371, test/test-gguffer.sh:191-214, :242-246. | Test `r-gguf_06_las_reglas_del_imatrix_son_duras` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-7** | `--quant` is repeatable and accepts comma-separated values: `--quant IQ2_M --quant IQ3_XS` or `--quant IQ2_M,IQ3_XS`. | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:27, :54-58, :1288-1295; test/test-gguffer.sh:152-156. | Test `r-gguf_07_quant_repetible_admite_coma_quant_iq2m` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |

### 4.14.3 The saving that shapes the design

*v2 module*: `crates/ropero-forge/src/pipeline.rs` · *Test area*: `crates/ropero-forge/tests/pipeline.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-8** | Viability check happens BEFORE downloading weights. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1619-1685; test/fakes/gguffer/llama-quantize:121-129. | Test `r-gguf_08_comprobacion_viability_ocurre_antes_descargar_los` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/pipeline.rs`; it turns red if behavior changes. |
| **R-GGUF-9** | Already-quantized (NVFP4) repo is preserved, not quantized: GGUF has `general.file_type = 39` (NVFP4), and message says so. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: test/test-gguffer.sh:144-161, :219-223. | Test `r-gguf_09_repo_cuantizado_nvfp4_conserva_cuantiza_gguf` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/pipeline.rs`; it turns red if behavior changes. |
| **R-GGUF-10** | FP8 repo can be quantized from dequantized F16; gguffer warns that dequantization loses the factory quantization advantage. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: test/test-gguffer.sh:235-240. | Test `r-gguf_10_repo_fp8_puede_cuantizarse_desde_f16` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/pipeline.rs`; it turns red if behavior changes. |

### 4.14.4 MTP head: the block imatrix does not cover

*v2 module*: `crates/ropero-forge/src/mtp.rs` · *Test area*: `crates/ropero-forge/tests/mtp.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-11** | Imatrix is calculated on base model (64 layers), but GGUF contains MTP head as another block (65). | `crates/ropero-forge/tests/mtp.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:123-140. | Test `r-gguf_11_imatrix_calcula_sobre_modelo_base_capas` in `crates/ropero-forge/tests/mtp.rs` verifies the requirement against `crates/ropero-forge/src/mtp.rs`; it turns red if behavior changes. |
| **R-GGUF-12** | Check imatrix coverage per whole block, not tensor by tensor: uncovered tensors are usually f32 norms, which are not quantized and do not require… (source truncates). | `crates/ropero-forge/tests/mtp.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:123-140. | Test `r-gguf_12_cobertura_del_imatrix_comprueba_por_bloque` in `crates/ropero-forge/tests/mtp.rs` verifies the requirement against `crates/ropero-forge/src/mtp.rs`; it turns red if behavior changes. |
| **R-GGUF-13** | Locate MTP block by metadata, not tensor name: MTP tensors are named like any other block (`blk.64.attn_k.weight`), without “nextn” in… (source truncates). | `crates/ropero-forge/tests/mtp.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:151-176. | Test `r-gguf_13_bloque_mtp_localiza_por_metadata_por` in `crates/ropero-forge/tests/mtp.rs` verifies the requirement against `crates/ropero-forge/src/mtp.rs`; it turns red if behavior changes. |
| **R-GGUF-14** | For imatrix errors, translate opaque quantizer message (“Missing importance matrix for tensor blk.64.attn_k.weight”) into the source: “this is the… (source truncates). | `crates/ropero-forge/tests/mtp.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phase 3, tail of quantization error). | Test `r-gguf_14_cuando_error_imatrix_gguffer_traduce_mensaje` in `crates/ropero-forge/tests/mtp.rs` verifies the requirement against `crates/ropero-forge/src/mtp.rs`; it turns red if behavior changes. |

### 4.14.5 Six phases

*v2 module*: `crates/ropero-forge/src/phases.rs` · *Test area*: `crates/ropero-forge/tests/pipeline.rs` · *Type*: integration

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-15** | gguffer has six phases in a fixed order: 1… (source truncates). | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phases 1–6, commented). | Test `r-gguf_15_gguffer_tiene_seis_fases_orden_fijo` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-16** | On quantization failure, delete NOTHING: retain safetensors clone and intermediate GGUF; error states exactly how to retry only quantization (the… (source truncates). | `crates/ropero-forge/src/phases.rs`<br>*Architecture change*: P-16: check preconditions/cleanup before the expensive action. <br>*v1 seed*: `bin/lib/cmd-gguffer.sh` (phase 3, retain weights in `$work`). | A quantization-phase failure leaves the clone and intermediate **on disk**, and the error says how to retry only that phase. |
| **R-GGUF-17** | Delete weights only after GGUF passes verification: “I am NOT deleting weights: they are in `$work`.” | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phases 4 and 5). | Test `r-gguf_17_los_pesos_solo_borran_gguf_supera` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-18** | Delete intermediate (F16) after all quants, not after each: subsequent quant in multi-quant run needs F16. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phase 5). | Test `r-gguf_18_intermedio_f16_borra_despues_todos_los` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |

### 4.14.6 Provenance: why it is mandatory

*v2 module*: `crates/ropero-forge/src/provenance.rs` · *Test area*: `crates/ropero-forge/tests/provenance.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-19** | Every produced GGUF leaves `<out>.fuente.json` stating model source, quant, importance matrix, and whether MTP head got a separate quant. | `crates/ropero-forge/tests/provenance.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phase 6); test/test-gguffer.sh:179-182. | Test `r-gguf_19_todo_gguf_producido_deja_out_fuente` in `crates/ropero-forge/tests/provenance.rs` verifies the requirement against `crates/ropero-forge/src/provenance.rs`; it turns red if behavior changes. |
| **R-GGUF-20** | gguffer must not misrepresent result: if weights were NVFP4 and nothing was quantized, final message says “I did NOT quantize anything: GGUF remains NVFP4”—the quant… (source truncates). | `crates/ropero-forge/tests/provenance.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (end of phase 6). | Test `r-gguf_20_gguffer_miente_sobre_resultado_los_pesos` in `crates/ropero-forge/tests/provenance.rs` verifies the requirement against `crates/ropero-forge/src/provenance.rs`; it turns red if behavior changes. |

### 4.14.7 `ggufcheck`: header-only reader

*v2 module*: `crates/ropero-forge/src/gguf.rs` · *Test area*: `crates/ropero-forge/tests/gguf.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-21** | `bin/python/ggufcheck.py` reads only the header in every mode: no `read()` may exceed 100 MiB even for a 300 MiB sparse file. | `crates/ropero-forge/tests/gguf.rs` <br>*v1 seed*: test/test-ggufcheck.sh:38-47, :51-75. | Test `r-gguf_21_bin_python_ggufcheck_lee_solo_cabecera` in `crates/ropero-forge/tests/gguf.rs` verifies the requirement against `crates/ropero-forge/src/gguf.rs`; it turns red if behavior changes. |
| **R-GGUF-54** 🔶 | gguffer requires available llama.cpp (official or fork), a precondition, not a detail: without it nothing can convert weights, quantize, or… (source truncates). | `crates/ropero-forge/src/gguf.rs`<br>*Architecture change*: S-7 / H-27. This is the highest-value fix in the document. <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1376-1392, :840-846; bin/lib/01-env.sh:33-34. | `ropero gguffer` checks the engine precondition **before phase 1**: without an engine, the error names the cause and action, and nothing is downloaded. |
| **R-GGUF-55** 🔶 | Converter is irreplaceable: without `$LLAMA_DIR/convert_hf_to_gguf.py` there is no conversion, and therefore no GGUF, quantized or not. | `crates/ropero-forge/src/gguf.rs`<br>*Architecture change*: The converter has no substitute. <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1377, :1992. | The converter `convert_hf_to_gguf.py` is searched in `$LLAMA_DIR` and PATH, and absence yields `ToolchainError::MissingTool` with searched path and action. |
| **R-GGUF-56** 🔶 | Check precondition BEFORE downloading weights, with message naming missing engine and variable pointing to it. | `crates/ropero-forge/src/gguf.rs`<br>*Architecture change*: Closes the false diagnosis that prompts deletion of 60 GB. <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1957 (the only `-f` check on the engine is for the patch), :2035-2041 (the classifier). | A precondition failure diagnostic never mentions downloading: `ToolchainError` has a variant for each cause, and none says “incomplete download”. |
| **R-GGUF-57** 🔶 | The two engines do not leave their clones in the same location; this is a gguffer seam (plus table in doc A). | `crates/ropero-forge/src/gguf.rs`<br>*Architecture change*: H-28: the official engine lives in `$LLAMA_OFFICIAL_DIR` and gguffer did not see it. <br>*v1 seed*: bin/lib/01-env.sh:33-34; bin/lib/engines/llama-server.sh:78, :155; bin/lib/engines/llama-server-official.sh:60, :77; bin/lib/cmd-g | The engines `llama-server` and `llama-server-official` record their clone directory in installation state, and gguffer queries it instead of assuming `$LLAMA_DIR`. |
| **R-GGUF-58** 🔶 | Neither `llama-quantize` nor `llama-gguf-split` is linked into `$BIN_DIR` when engine is installed; installers link only `llama-server` and `llama-server-official`. | `crates/ropero-forge/src/gguf.rs`<br>*Architecture change*: H-29: no installer activated the alternative PATH. <br>*v1 seed*: bin/lib/cmd-install.sh:599-613 (link name is the resolved binary basename), bin/lib/engines/llama-server.sh (links llama-serv | `ropero install engines` links `llama-quantize` and `llama-gguf-split` in `$BIN_DIR`, or records its path in state; gguffer finds them without manual export. |
| **R-GGUF-22** | Modes are `--meta`, `--tipo-predominante`, `--file-type`, `--nombres`, `--bloque`, `--info`, and default validation; each has an exact expected output. | `crates/ropero-forge/tests/gguf.rs` <br>*v1 seed*: test/test-ggufcheck.sh:38-47. | Test `r-gguf_22_los_modos_son_meta_tipo_predominante` in `crates/ropero-forge/tests/gguf.rs` verifies the requirement against `crates/ropero-forge/src/gguf.rs`; it turns red if behavior changes. |
| **R-GGUF-23** | Invalid GGUF fails with rc=1, clear message, and no traceback: incomplete tensor index (declares 3, has 2) reports actual count. | `crates/ropero-forge/tests/gguf.rs` <br>*v1 seed*: test/test-ggufcheck.sh:115-134. | Test `r-gguf_23_gguf_invalido_falla_con_mensaje_claro` in `crates/ropero-forge/tests/gguf.rs` verifies the requirement against `crates/ropero-forge/src/gguf.rs`; it turns red if behavior changes. |
| **R-GGUF-24** | Deprecated parameter `solo_cabecera=True` remains accepted (and ignored): backward compatibility. | `crates/ropero-forge/tests/gguf.rs` <br>*v1 seed*: test/test-ggufcheck.sh:79-93. | Test `r-gguf_24_parametro_deprecado_solocabecera_true_sigue_aceptando` in `crates/ropero-forge/tests/gguf.rs` verifies the requirement against `crates/ropero-forge/src/gguf.rs`; it turns red if behavior changes. |
| **R-GGUF-25** | `ggufcheck` can run against any GGUF, not only Ropero's: accepts `--lib <path>` for ggml library. | `crates/ropero-forge/tests/gguf.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh (phase 4); bin/python/ggufcheck.py. | Test `r-gguf_25_ggufcheck_puede_ejecutar_contra_cualquier_gguf` in `crates/ropero-forge/tests/gguf.rs` verifies the requirement against `crates/ropero-forge/src/gguf.rs`; it turns red if behavior changes. |

### 4.14.8 Checkpoint taxonomy: five classes

*v2 module*: `crates/ropero-forge/src/checkpoint.rs` · *Test area*: `crates/ropero-forge/tests/checkpoint.rs` · *Type*: unit

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-26** | Preflight classifies checkpoint before downloading anything, reading `config.json` and `hf_quant_config.json` (from repo over HTTP with `curl -fsS`, or local directory) and returning a… (source itself truncates). | `crates/ropero-forge/tests/checkpoint.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:288-378, :1647-1673. | Test `r-gguf_26_preflight_clasifica_checkpoint_antes_bajar_nada` in `crates/ropero-forge/tests/checkpoint.rs` verifies the requirement against `crates/ropero-forge/src/checkpoint.rs`; it turns red if behavior changes. |
| **R-GGUF-27** | Parsing rules resolve three traps: `quant_algo` is nested in `quantization`, not at root; … (source itself truncates). | `crates/ropero-forge/tests/checkpoint.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:223-245, :279-283, :305-306. | Test `r-gguf_27_las_reglas_parseo_tienen_tres_trampas` in `crates/ropero-forge/tests/checkpoint.rs` verifies the requirement against `crates/ropero-forge/src/checkpoint.rs`; it turns red if behavior changes. |
| **R-GGUF-28** | Quantization plan decides per requested quant in four situations: (1) GGUF already has that quant → do not quantize; … (source itself truncates). | `crates/ropero-forge/tests/checkpoint.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:426-483. | Test `r-gguf_28_plan_cuantizacion_decide_por_cada_quant` in `crates/ropero-forge/tests/checkpoint.rs` verifies the requirement against `crates/ropero-forge/src/checkpoint.rs`; it turns red if behavior changes. |
| **R-GGUF-29** | “Quantized” means every `file_type` except `""`, `0` (F32), `1` (F16), and `32` (BF16). | `crates/ropero-forge/tests/checkpoint.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:389-394. | Test `r-gguf_29_cuantizado_significa_todo_filetype_salvo_f32` in `crates/ropero-forge/tests/checkpoint.rs` verifies the requirement against `crates/ropero-forge/src/checkpoint.rs`; it turns red if behavior changes. |
| **R-GGUF-30** | Unknown quant aborts with rc 1 before download: parse list from engine `llama-quantize --help` in two versions (with alias and canonical, `grep -v "alias for"`), and… (source itself truncates). | `crates/ropero-forge/tests/checkpoint.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1440-1465. | Test `r-gguf_30_quant_desconocido_aborta_con_antes_descargar` in `crates/ropero-forge/tests/checkpoint.rs` verifies the requirement against `crates/ropero-forge/src/checkpoint.rs`; it turns red if behavior changes. |

### 4.14.9 Complete CLI surface

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-31** | `ropero gguffer` has no subcommands: one command with flags; modes include `--list-quants` (or `--quant` without value), which lists engine and published quants… (source truncates). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1324-1326, :1391-1415, :2060-2064. | Test `r-gguf_31_ropero_gguffer_tiene_subcomandos_comando_con` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-32** | Complete flags and which are required (plus table in doc A). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1271-1327. | Test `r-gguf_32_los_flags_completos_con_obligatoriedad` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-33** | Environment variables governing behavior but absent from `--help`: `GGUF_DIR` (`$HOME/models/gguf`), `GGUF_WORK_DIR` (`$HOME/.cache/ropero/gguffer`), `LLAMA_DIR`… (source truncates). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:975, :1271, :1376, :1476-1477, :1801. | Test `r-gguf_33_las_variables_entorno_que_gobiernan_comportamiento` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |
| **R-GGUF-34** | Map TOML `[build]` route to gguffer argv through explicit `modelconf.py` table: `repo→--repo`, `gguf→--gguf`, `gguf_file→--gguf-file`, `imatrix→--imatrix`,… (source truncates). | `crates/ropero-cli/tests/cli.rs` <br>*v1 seed*: bin/python/modelconf.py:1248-1263, :1326-1334; bin/lib/02-modelconf.sh:323-336; bin/quant/ACRITO.md:23-26. | Test `r-gguf_34_ruta_build_del_toml_traduce_argv` in `crates/ropero-cli/tests/cli.rs` verifies the requirement against `crates/ropero-cli/src/gguffer.rs`; it turns red if behavior changes. |

### 4.14.10 Conversion: three fixes from real cases

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-35** | Intermediate name describes contents: NVFP4 checkpoint → `-NVFP4`, MXFP4 → `-MXFP4_MOE`, otherwise `-F16`/`-BF16` per `--intermedio`. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1630-1635, :1802-1826. | Test `r-gguf_35_nombre_del_intermedio_describe_que_lleva` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-36** | Pass `--no-mtp` only if converter supports it, checked with `grep -qE "no-mtp\|no_nextn"`. | `crates/ropero-forge/src/phases.rs`<br>*Architecture change*: Provenance is mandatory (R-GGUF-18). <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1919-1925; :187-200. | A GGUF without a provenance sidecar is rejected by `ropero install models` and `ropero doctor`. |
| **R-GGUF-37** | Fix tokenizer before conversion in two steps: inject repo `chat_template.jinja` into `tokenizer_config.json`—llama.cpp converter never reads… (source truncates). | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1070-1141, :1984-1989. | Test `r-gguf_37_tokenizer_arregla_antes_convertir_dos_pasos` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-38** | Conversion has two automatic retries: (1) `Tokenizer class … does not exist` ⇒ fix tokenizer and retry; (2)… (source truncates). | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1992-2051. | Test `r-gguf_38_conversion_tiene_dos_reintentos_automaticos_tokenizer` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-39** | Quantizer flags precede positionals: `llama-quantize` parses options only while argument starts with `--`. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:2213-2219. | Test `r-gguf_39_las_flags_del_cuantizador_van_antes` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-40** | Locate MTP block by combining metadata (`block_count - nextn_predict_layers`) with block-by-block comparison against imatrix. | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:2115-2166, :878-906. | Test `r-gguf_40_bloque_mtp_localiza_por_dos_vias` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |
| **R-GGUF-41** | Never compare `general.file_type` (llama_ftype: NVFP4=39, MXFP4_MOE=38, BF16=32, F16=1) with `ggml_type` (NVFP4=40, MXFP4=39): separate number spaces and “never…” (source truncates). | `crates/ropero-forge/tests/pipeline.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1435-1438, :2072-2098; bin/quant/quant-tipos.sh:9-11. | Test `r-gguf_41_general_filetype_llamaftype_nvfp4_mxfp4moe_bf16` in `crates/ropero-forge/tests/pipeline.rs` verifies the requirement against `crates/ropero-forge/src/phases.rs`; it turns red if behavior changes. |

### 4.14.11 Engine patches

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-42** | Ropero engine is a private clone (`Lorenzo-SF/ropero-llama-turboquant`) with public-fork fallback; patches apply with `patch -p1` and require rebuild afterward… (source truncates). | `crates/ropero-runtime/tests/engines.rs` <br>*v1 seed*: bin/quant/aplicar-parche-motor.sh:103-118, :153-158, :199-203, :206-220; bin/lib/engines/llama-server.sh:77-117. | Test `r-gguf_42_motor_ropero_clon_privado_lorenzo_ropero` in `crates/ropero-runtime/tests/engines.rs` verifies the requirement against `crates/ropero-runtime/src/engines/llama.rs`; it turns red if behavior changes. |
| **R-GGUF-43** | Applying mixed-precision patch in gguffer is fail-open (`patch -p1 … \|\| true`): if patch does not apply, continue and conversion fails later with less clear error. | `crates/ropero-runtime/tests/engines.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1959. | Test `r-gguf_43_aplicacion_del_parche_mixed_precision_gguffer` in `crates/ropero-runtime/tests/engines.rs` verifies the requirement against `crates/ropero-runtime/src/engines/llama.rs`; it turns red if behavior changes. |

### 4.14.12 Script layer (`bin/quant/`)

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-44** | `bin/quant/` is a script layer over gguffer; `modelos.tsv` (7 columns: `n pesos q imr imf flags fuente`) is source of truth—the scripts do not carry the list… (source truncates). | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/cuantizar-todos.sh:5-6, :92-102, :113, :227; bin/quant/README.md:71-72, :78-81. | Test `r-gguf_44_bin_quant_una_capa_guiones_sobre` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-45** | Destination name is never misleading: if TOML route name lacks main quant name, put GGUF in its own named folder instead of… (source truncates). | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/cuantizar-todos.sh:146-186; bin/lib/cmd-gguffer.sh:105-118, :1853-1858. | Test `r-gguf_45_nombre_del_destino_nunca_mentiroso_nombre` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-46** | Failure for one model does not stop batch (unless `--parar`): “a model that cannot be built must not prevent others from being built.” | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/cuantizar-todos.sh:35-37, :204-212, :244-272. | Test `r-gguf_46_fallo_modelo_para_tirada_salvo_parar` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-47** | `bin/quant/todo.sh` is the complete script and order matters: quantize → verify → upload. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/todo.sh:47-92, :66-82. | Test `r-gguf_47_bin_quant_todo_guion_completo_orden` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-48** | `verificar-gguf.sh` verifies post-build without GPU: require ≥100 MiB, readable metadata, `arch`, >0 tensors, and `block_count`. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/verificar-gguf.sh:28-96. | Test `r-gguf_48_verificar_gguf_verificacion_posterior_sin_gpu` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-49** | HF upload (`subir-hf.sh`) creates `<HF_USER>/<model-name>-<QUANT>-GGUF` (name derived from file and actual `general.file_type`), uploads GGUF and imatrix if… (source truncates). | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/subir-hf.sh:46, :105-129, :184, :200-300. | Test `r-gguf_49_subida_subir_crea_repo_hfuser_nombre` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-50** | Model card comes from GGUF itself, not a table, and must state the model is not the quantizer's own. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/python/model_card.py:21-27, :186-188, :272, :305-309; bin/quant/subir-hf.sh:230-252. | Test `r-gguf_50_model_card_sale_del_propio_gguf` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |

### 4.14.13 Three measured gguffer discrepancies

| Requirement | What it requires | Where checked | Acceptance gate |
|---|---|---|---|
| **R-GGUF-51** | `subir-hf.sh --subir` does NOT verify uploaded GGUF: only checks `.hf-uploaded` marker. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/subir-hf.sh:203-206, :278; bin/quant/README.md:82-83. | Test `r-gguf_51_subir_subir_verifica_gguf_que_sube` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-52** | Provenance write is fail-open and return code ignored: if `_gguffer_escribe_fuente` fails, GGUF lacks `.fuente.json` and run exits 0. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/lib/cmd-gguffer.sh:1006, :2315. | Test `r-gguf_52_escritura_procedencia_fail_open_codigo_retorno` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |
| **R-GGUF-53** | `cuantizar-todos.sh` comment is stale vs code: says gguffer deletes safetensors “whether everything succeeds or something fails”. | `crates/ropero-forge/tests/forge.rs` <br>*v1 seed*: bin/quant/cuantizar-todos.sh:30-33 vs. bin/lib/cmd-gguffer.sh:2040-2047; bin/quant/patches/; bin/python/model_card.py vs. bin/quan | Test `r-gguf_53_comentario_cuantizar_todos_esta_desfasado_respecto` in `crates/ropero-forge/tests/forge.rs` verifies the requirement against `crates/ropero-forge/src/`; it turns red if behavior changes. |

## 8. Build plan

### 8.1 The golden master: preserving what already works

The Rust reimplementation risks losing details that took years to get right (the stamp, probe hierarchy, MTP handling, checkpoint taxonomy). The mechanism that prevents this:

> **Before rewriting a function, freeze its current behavior as a golden test.**

**Procedure (per task):**

1. Locate the requirement evidence in doc A (`file:line`).
2. Run the v1 behavior and **capture its output** (stdout, stderr, exit code, written files).
3. Write the Rust golden test with that output **as the expected value**.
4. Implement in Rust until it matches.
5. If the v1 output is incorrect (a bug), **do not freeze the bug**: freeze the *correct* output and document the behavior change as a `fix` in doc A, with its own PR.

Step 5 distinguishes a golden master from blindly freezing errors. Freezing a bug turns the bug into a requirement, which is exactly what a poorly made golden master does.

**Where the golden master lives**: `crates/ropero-core/tests/golden/` with `.snap` files (`insta`), one per behavior. The test compares **structured** output, not text: changing the order of two log lines does not fail the test, because log order is not a contract.

### 8.2 The ten phases

```
F0 ──► F1 ──► F2 ──► F3 ──► F4 ──► F5 ──► F6 ──► F7 ──► F8 ──► F9
base   data   engine runtime state catalog forge gateway (serv.) coexistence
```

Each phase is **deliverable, verifiable, and useful on its own**. The system is usable from F3: a Ropero v2 that starts models is worth more than a Ropero v2 with a perfect catalog and no engine.

| Phase | Name | Deliverable | Closes |
|---|---|---|---|
| **F0** | Foundations | Workspace, toolchain, CI, empty crates, architecture test | — |
| **F1** | Data model | TOML types, validator, catalog, aliases | R-DATA, R-ARQ, R-CTX |
| **F2** | Engines | Manifests, `EngineId`, compatibility, maturity | R-ENG, R-CTX |
| **F3** | Runtime | Preflight, probe, launch, stamp, foreground | R-RUN, R-ARQ |
| **F4** | State and ports | On-disk state, identity, stop, locks | R-EST, R-ARQ |
| **F5** | Catalog and downloads | `ropero list`, `install models`, `hf` | R-MOD, R-CLI |
| **F6** | Forge | Six phases, checkpoints, MTP, provenance | R-GGUF |
| **F7** | Gateway | OpenAI-compatible endpoint, auth, streaming, routing | R-API, R-ROUTE |
| **F8** | Installation and opencode | v2 `install.sh`, plugin, `doctor`, `stats` | R-INST, R-OC, R-TST |
| **F9** | Coexistence | v1 and v2 running side by side, migration, v1 retirement | — |

### 8.3 Tasks

Each task states what it does, which requirements it closes, its dependencies, and its gate. **One task = one PR.**

#### F0 · Foundations

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-01** | Workspace with six crates, `rust-toolchain.toml` (1.100.0), `rustfmt.toml`, `clippy.toml`, `deny.toml`, committed `Cargo.lock`, release profile | — | — | `cargo build` passes; dependency graph verified by test |
| **T-02** | Architecture test: `ropero-core` does not use `std::process`, `std::fs`, `tokio`, or `reqwest`; graph has no cycles; `ropero-cli` < 400 lines | R-ARQ-1 | T-01 | Test fails if a forbidden dependency is added |
| **T-03** | CI: clippy, fmt, nextest, llvm-cov, cargo-deny, macOS + Linux matrix | — | T-01 | CI passes locally and on GitHub |
| **T-04** | ADR-001: “why Rust”, ADR-002: “why extraction rather than rewrite”, and ADR-003: “`ropero-code` is a reference, not a dependency” | — | — | All three ADRs exist using their template |
| **T-05** | Port `models/_template.toml` and all 45 TOMLs to the typed schema, **without changing their content**; new validator accepts all 45 | R-DATA | T-01 | `ropero validate` accepts all 45 and rejects a TOML with an unknown key |

#### F1 · Data model

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-06** | `ModelSpec` and all its types with `serde` + `deny_unknown_fields` | R-DATA-1…16 | T-05 | Every doc A table has a type and a test |
| **T-07** | Validator: the 15 rules in § 4.1.1, each with a failing test | R-DATA-17…23 | T-06 | 15 tests, one per rule, all red beforehand |
| **T-08** | `ArgvBuilder`: base → level → flags → options, `replace`, `requires`, placeholders | R-DATA-16…21 | T-06 | Golden tests against actual argv from the TOMLs |
| **T-09** | Catalog: discover `models/`, `fired/`, unique names, aliases | R-MOD-1…8 | T-06 | Catalog of 45 (17 active + 28 fired) loads |
| **T-10** | `ropero validate`, `ropero list`, `ropero template`, `ropero args`, `ropero levels`, `ropero path` | R-CLI-6…13 | T-09 | All six commands produce expected output |

#### F2 · Engines

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-11** | `EngineId` enum + legacy aliases + canonicalization with warning | R-ENG-1, R-DATA-13 | T-06 | Test each alias and canonical ID |
| **T-12** | Engine manifest in TOML + all 8 manifests | R-ENG-2…7 | T-11 | All 8 load and validate |
| **T-13** | Compatibility evaluation: OS, NVIDIA, Metal, NPU, RAM; **accumulates all reasons** | R-CTX-1, R-ENG-3 | T-12 | Test on Linux and macOS (conditional) |
| **T-14** | `maturity` in manifest, visible in `install engines` and `doctor` | R-CTX-3 🔶 | T-12 | The six tested engines and two untested engines are distinguished |

#### F3 · Runtime

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-15** | Preflight: OS, installed engine, files present, RAM, disk space | R-RUN-1…4 | T-13 | Each precondition has a failure test |
| **T-16** | Three-level readiness probe with backoff and deadline | R-RUN-5…7 | T-15 | A real engine starts and probe passes; a fake engine does not |
| **T-17** | Stamp: hash of model + context + level + files | R-RUN-8 | T-08 | Changing context changes the stamp |
| **T-18** | Foreground and background launch; `SIGINT` propagates to group | R-RUN-9…17 | T-16 | Ctrl-C leaves process dead and state clean |
| **T-19** | State machine with all 11 transitions and exhaustive test | R-EST-1 | T-18 | Every disallowed transition panics in test |

#### F4 · State and ports

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-20** | `StateStore`: `KEY=value`, atomic write, preserves keys | R-ARQ-9, R-EST-2 | T-19 | A v1 state is read and migrated |
| **T-21** | Identity `pid + start_time + exe_digest`; `ProcIdentity` | R-EST-8 | T-20 | A recycled PID is **not** mistaken for the original process |
| **T-22** | `stop`: four-step check and SIGTERM→SIGKILL escalation | R-EST-3…14 | T-21 | **A foreign process survives** (dedicated test) |
| **T-23** | Port lock with expiration | R-RUN-17 | T-20 | Orphaned lock is removed; live lock blocks |
| **T-24** | `status`: reconstruct state from disk | R-ARQ-11, R-EST-2 | T-20 | State after restarting Ropero is unchanged |

#### F5 · Catalog and downloads

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-25** | `ropero install models`: only models from installed engines; without a TTY, require an explicit list; and the `--prio` contract in § 4.10.7 (global or per-model; ignored when a model lacks that path or it is already the default; ask when both paths exist) | R-CTX-2, R-CLI-16, R-MOD-29 | T-09 | With an installed engine, only its models are offered; `ropero install models nemotron groot --prio=build qwen --prio=download` resolves nemotron by prompt, groot to `build`, and qwen to `download` |
| **T-26** | Download with `hf`: exact R-MOD-16 sequence, resume, empty file | R-MOD-9…20 | T-25 | With fake `hf`: called with the exact arguments |
| **T-27** | Reuse across models and download decision | R-MOD-21…28 | T-26 | Two models from the same repo download once |
| **T-28** | stdout contract: `ROPERO_PORT=` last, everything else to stderr | R-OC-1…5, R-CLI-17 | T-18 | Plugin reads the three lines |

#### F6 · Forge

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-29** | **`ensure_toolchain()`: engine precondition, before phase 1** | R-GGUF-54…59 🔶 | T-26 | Without engine: correct error and **zero bytes downloaded** |
| **T-30** | Six phases as a state machine, with checkpoint per phase | R-GGUF-16…21 | T-29 | Failure in phase 5 **preserves** clone and intermediate |
| **T-31** | Five checkpoint classes and retry by phase | R-GGUF-32…36 | T-30 | Only the expensive phase is retried |
| **T-32** | MTP block: the head imatrix does not cover | R-GGUF-22…31 | T-30 | MTP is quantized using its own rule |
| **T-33** | Read GGUF header (replaces `ggufcheck.py`) | R-GGUF-37…46 | T-30 | Header is read without loading the model |
| **T-34** | Provenance: sidecar that makes the artifact auditable | R-GGUF-47…51 | T-33 | GGUF without sidecar is rejected |
| **T-35** | `ropero gguffer`: complete CLI, with six phases and their flags | R-GGUF-52…58 | T-34 | CLI matches doc A |

#### F7 · Gateway

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-36** | `ropero serve`: axum, `/v1/models`, `/v1/chat/completions`, SSE streaming | R-API-1…11, R-API-19…22 | T-16 | Stream arrives token by token, without buffering |
| **T-37** | **Auth: key via mode-0600 file, never in argv** | R-API-12…14, R-TST-23 | T-36 | `/proc/<pid>/cmdline` does not contain the key |
| **T-38** | Auth test **on the function**, not on a subprocess return code | R-TST-19 | T-37 | Breaking auth **turns the test red** (T-2) |
| **T-39** | Routing (D3): semantic selection and its degradation | R-ROUTE-1…5 | T-36 | Same input always goes to same destination |
| **T-40** | Per-client limits and CORS | R-API-23…25 🔶 | T-36 | 429 with `Retry-After` |

#### F8 · Installation and opencode

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-41** | v2 `install.sh`: symlink-only, idempotent, `--check`, old-layout migration | R-INST-1…27 | T-35 | R1–R13 rules verified by test |
| **T-42** | `ropero doctor` and `ropero check`: real diagnostics | R-INST-13…20 | T-41 | Every warning says **what to do** |
| **T-43** | `ropero stats`: CPU/RAM/GPU, with `--watch` and `--json` | R-CLI-14, R-CLI-17 🔶 | T-42 | `doctor` and `stats` share sampling layer |
| **T-44** | opencode plugin: consumes stdout contract | R-OC-1…11 | T-28 | Plugin starts a model and obtains the port |
| **T-45** | Rust integration suite, with fakes | R-TST-1…28 | T-44 | Suite runs in < 60 s without GPU |

#### F9 · Coexistence

| Task | What it does | Closes | Depends on | Gate |
|---|---|---|---|---|
| **T-46** | v1 and v2 read and write the same on-disk state | — | T-24 | State written by v1 is read by v2, and vice versa |
| **T-47** | Migration guide and v1 retirement | — | T-46 | CHANGELOG states what changed and what did not |
| **T-48** | Final report: **what was measured**, what remains pending, what debt was closed | — | T-47 | All H-1…H-29 from doc A are closed or documented |

### 8.4 Dependency graph

```
T-01 ──► T-02, T-03, T-04, T-05
T-05 ──► T-06 ──► T-07, T-08, T-09, T-11
T-09 ──► T-10
T-11 ──► T-12 ──► T-13 ──► T-15
T-12 ──► T-14
T-15 ──► T-16 ──► T-18 ──► T-19 ──► T-20 ──► T-21 ──► T-22
T-20 ──► T-23, T-24
T-08 ──► T-17
T-17 ──► T-18
T-18 ──► T-28
T-09 ──► T-25 ──► T-26 ──► T-27
T-26 ──► T-29 ──► T-30 ──► T-31, T-32, T-33 ──► T-34 ──► T-35
T-16 ──► T-36 ──► T-37 ──► T-38, T-39, T-40
T-35 ──► T-41 ──► T-42 ──► T-43
T-28 ──► T-44 ──► T-45
T-24 ──► T-46 ──► T-47 ──► T-48
```

**T-29 and T-38 are the two highest-value tasks** and can run in parallel from the start: both close measured issues and are independent of the rest.

### 8.5 What can be done in parallel

| Group | Tasks | Why they are independent |
|---|---|---|
| A | T-07, T-08, T-09, T-11 | All depend only on T-06 and touch different files |
| B | T-31, T-32, T-33 | The three forge branches (checkpoint, MTP, header) |
| C | T-37, T-39, T-40 | Auth, routing, and limits: each is independent middleware |
| D | T-22, T-23, T-24 | Stop, locks, and state: same crate, different files |

### 8.6 The sequence for a complete phase

For every phase, follow the same ritual:

```
1. Read the phase requirements in doc A (§ 7.4 has each requirement row).
2. Locate v1 evidence and freeze the golden (§ 8.1).
3. Write the tests (red).
4. Implement.
5. Verify the global gate (§ 7.2).
6. Update doc A if anything changed.
7. Open a PR with the table of closed requirements.
```

---
---

## 9. Testing strategy

### 9.1 The pyramid

| Level | What it tests | Count | When it runs | Target duration |
|---|---|---:|---|---|
| **Unit** | Pure domain functions: argv, validation, state, checksum, placeholders | ~400 | Every push | < 5 s |
| **Property** | Invariants over arbitrary inputs | ~60 | Every push | < 20 s |
| **Integration** | Real processes: launch, probe, stop; HTTP server | ~120 | Every push | < 60 s |
| **Golden** | Frozen v1 outputs | ~80 | Every push | < 10 s |
| **End-to-end** | Full CLI against a real engine | ~25 | Daily + before release | < 10 min |
| **Manual** | Real hardware: GPU, NPU, Metal | script | Before each release | hours |

**88% of the work is checked in under a minute.** A slow verification loop is one that does not get run.

### 9.2 Property tests: invariants that must not break

A property test does not test one example: it tests a **family** of inputs. Ropero's invariants:

```rust
// The assembled argv NEVER contains an unresolved placeholder.
proptest! {
    #[test]
    fn argv_no_tiene_placeholders_sin_resolver(spec in arbitraria_spec()) {
        let argv = build_argv(&spec, &req, &ctx).unwrap();
        for a in &argv {
            prop_assert!(!a.contains("${"), "unresolved placeholder: {a}");
        }
    }
}

// argv order is ALWAYS base → level → flags → options.
proptest! {
    #[test]
    fn argv_respeta_el_orden_fijo(spec in arbitraria_spec(), req in arbitraria_request()) {
        let argv = build_argv(&spec, &req, &ctx).unwrap();
        // the user's last flag is the last flag in argv
        if let Some(last) = req.flags.last() {
            let pos = argv.iter().rposition(|a| a.contains(last)).unwrap();
            prop_assert!(pos > 0);
        }
    }
}

// build_argv is pure: identical inputs produce identical argv.
proptest! {
    #[test]
    fn argv_es_determinista(spec in arbitraria_spec(), req in arbitraria_request()) {
        let a = build_argv(&spec, &req, &ctx).unwrap();
        let b = build_argv(&spec, &req, &ctx).unwrap();
        prop_assert_eq!(a, b);
    }
}

// An undeclared level is never accepted.
proptest! {
    #[test]
    fn nivel_no_declarado_siempre_falla(spec in arbitraria_spec()) {
        let req = LaunchRequest { level: Some("nivel-inexistente-xyz".into()), .. };
        prop_assert!(build_argv(&spec, &req, &ctx).is_err());
    }
}

// On-disk state round-trips.
proptest! {
    #[test]
    fn estado_hace_roundtrip(st in arbitraria_estado()) {
        let dir = tempdir().unwrap();
        StateStore::new(dir).write(9999, &st).unwrap();
        let leido = StateStore::new(dir).read(9999).unwrap();
        prop_assert_eq!(st, leido);
    }
}
```

**The last test has the most value**: on-disk state is the source of truth (P-4), and a broken round-trip means Ropero “forgets” a model in plain sight. An example test will not find it; a property test will.

### 9.3 Fakes

The v1 repo already has fake `hf`, `curl`, `llama-quantize`, `convert_hf_to_gguf.py`, and `strata` in `test/fakes/`. In v2 they become **Rust binaries** in a `ropero-fakes` crate, with one important difference:

| In v1 (bash) | In v2 (Rust) |
|---|---|
| Fake records to a text file | Fake **is** a process that responds to a protocol |
| Test greps the file | Test asks the fake a **question** |
| If fake is not invoked, file does not exist | If fake is not invoked, the test fails when asking |

Avoid the pattern from wound #1: a fake that answers “ok” and a test that **does not inspect the answer**. In Rust, the fake is queried through an API that returns a typed result, and the test asserts on that result.

**The engine fake** is most important: a process that opens a socket on a port, responds to `/health` with 200, and returns a fixed inference response. It tests launch, probe, stamp, and stop without a GPU.

### 9.4 Coverage

| Crate | Line threshold | Branch threshold | Reason |
|---|---:|---:|---|
| `ropero-core` | **95%** | 90% | It is pure and everything is testable. No excuse here. |
| `ropero-runtime` | 80% | 70% | Some code only runs with a real engine. |
| `ropero-download` | 85% | 75% | Resume logic is critical. |
| `ropero-forge` | 80% | 70% | Phases are tested with fakes. |
| `ropero-server` | 85% | 80% | Auth and streaming are critical. |
| `ropero-cli` | 60% | 50% | It is wiring; contract tests matter most. |

**Coverage is not the goal; it is a smoke detector.** A high threshold forces code to be reachable, which makes unit tests possible. The real goal is P-14: tests **must be able to fail**.

### 9.5 Tests missing today (and to be written)

Doc A measures three wounds. Each is closed with a **concrete** test:

| Wound | Test that closes it | Why the current test did not catch it |
|---|---|---|
| False-green auth (R-TST-19) | `auth_rechaza_clave_incorrecta` on the function, not the return code | v1 test captured `rc=$?` and never inspected it again; `exit 0` on line 206 made the check unreachable; count used hardcoded `+12` |
| Levels without tests (R-TST-22) | `nivel_llega_al_argv`, `nivel_no_declarado_es_error_2`, `nivel_por_defecto_se_aplica` | Catalog defines levels and `rp_model_argv` resolves them, but no test verifies this |
| Key in argv (R-TST-23) | `la_clave_no_aparece_en_el_cmdline`, reading `/proc/<pid>/cmdline` | Key was in engine argv, readable by any host user |

And a fourth, not listed as a wound in doc A but revealed by this document's analysis:

| Fourth risk | Test that closes it |
|---|---|
| **gguffer precondition is not checked** (R-GGUF-54) | `gguffer_sin_motor_no_descarga_nada`: without an engine, error is `NoEngineInstalled` and fake `hf` **records no call** |

### 9.6 CI

```yaml
# .github/workflows/ci.yml — skeleton
name: ci
on: [push, pull_request]

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.100.0", components: "rustfmt, clippy" }
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo nextest run --all-features --no-fail-fast
      - run: cargo llvm-cov --all-features --lcov --output-path coverage.lcov
      - run: cargo deny check

  macos:
    runs-on: macos-latest   # arm64: reference hardware
    steps: [same as above]

  e2e:
    needs: quality
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --test e2e -- --ignored   # those requiring a GPU
```

**The macOS job is not optional.** Half the engines exist only on macOS (Metal, MLX), and the bash 3.2 constraint comes from there. Linux-only CI detects neither issue.

---

## 10. CI/CD and publication

### 10.1 Publication

| Aspect | Decision |
|---|---|
| Versioning | SemVer. `0.x` while the CLI changes; `1.0.0` when the stdout contract and CLI are declared stable. |
| Binaries | 4 targets (§ 2.2). Linux with `musl` (static), native macOS. |
| Signing | `cosign` over the tarball, with public key in the repo. |
| Checksums | `SHA256SUMS` alongside the tarball. |
| SBOM | `cargo cyclonedx` on every release. |
| Channel | `releases/latest` and `releases/canary` (every merge to `main`). |
| Installation | `install.sh` downloads the binary from the channel chosen by the user. |

### 10.2 v2 `install.sh`

The v1 installer has 13 hard rules (R1–R13) and 580 lines. v2:

| v1 rule | In v2 |
|---|---|
| R1: symlinks only | The principle remains (S-4), but the symlink points to a **single binary**, not a tree of scripts. |
| R2: `~/.local/share/<name>` → `<repo>/bin` | Removed: there is no tree to link. The link is to `~/.local/bin/ropero`. |
| R3: idempotence | Remains, and is **checked** with `--check` after installation. |
| R4: do not touch foreign files | Remains **unchanged**: a real file at `~/.local/bin/ropero` is not overwritten. |
| R5: migrate old layout | Remains: detect the v1 layout and replace it **only after** verifying that it belongs to us. |
| R6: offline | **Changes**: the v2 installer downloads the binary. The ADR must explain why and what integrity check is used. |
| R7: flags | `--local`, `--install`, `--check`. Add `--version`, `--uninstall`, `--dir`, `--bin-dir`. |
| R8: output | `[✓]/[!]/[✗]/[-]` markers to stderr, in Spanish, with a summary. |
| R9: post-install verification | `ropero --version` and `ropero doctor`. |
| R10: bash 3.2 | Remains: the installer is still bash 3.2 because it must run on a machine without Rust. |
| R11: keep the repo clean | Remains. |
| R12: read-only `--check` | Remains. |
| R13: PATH warning | Remains. |

**R6 is the only fundamental change**, which is why it needs an ADR. Reason: without published binaries, installing Ropero v2 would require a Rust toolchain on the user's machine, a barrier to entry that a model manager cannot impose. The ADR must record the alternative (keep the current `install.sh` and compile from the repo) and why it is rejected.

### 10.3 The CHANGELOG

Keep a Changelog format, in Spanish, with the categories from § 6.3. Entries for **behavior changes** are mandatory and include the requirement that caused them:

```markdown
## [2.0.0] — 2026-11-12

### Cambiado
- On-disk state adds `STARTTIME` and `EXEDIGEST`. A Ropero v1 can read v2 state (ignores unknown keys); Ropero v2 reads v1 state but treats it as “foreign” until identity is verified. (R-EST-8, R-ARQ-9)
- `ropero gguffer` checks the engine precondition **before** downloading. Without an installed engine, it now fails before download with the actual reason. (R-GGUF-54…59)
```

---

## 11. Security

### 11.1 Attack surface

Ropero runs on the user's machine with their permissions and launches processes that consume the GPU. The real threats are:

| Threat | Impact | Mitigation |
|---|---|---|
| **API key theft** | Access to the model over the local network | Key in mode-0600 file, never argv (§ 3.6) |
| **Killing a foreign process** | Loss of the user's work | Identity `pid + start_time + digest` (§ 3.3.3) |
| **Foreign models on the GPU** | Model does not start, or runs slowly | Detect foreign engines (H-26, see § 11.4) |
| **Downloading a tampered artifact** | Arbitrary GGUF on disk | Checksum + provenance verification (§ 11.3) |
| **Path traversal in TOML** | Write outside model directory | Validate paths: all paths relative, normalize and verify they do not escape (§ 11.2) |
| **Command injected through a field** | Arbitrary execution | **TOML is data and is never evaluated** (P-6). Build argv as a vector, without a shell. |
| **Secrets in logs** | Disclosure | `tracing` filter that redacts; test verifies the key does not appear in logs |

### 11.2 Paths

**Rule**: no path in TOML is absolute. `${gguf}` is always resolved inside `$GGUF_DIR/<model>/`. If a TOML contains an absolute path, the validator **rejects** it (does not normalize it): the error is clearer than “Ropero cannot write to /etc”.

If a path contains `..`, reject it during validation, not during writing.

### 11.3 Provenance and verification

A GGUF downloaded or built by Ropero carries a **provenance sidecar**:

```toml
# Groot-65B-A6B-GyroM-Q2_0.gguf.provenance.toml
model = "groot"
source_repo = "BoxyFit/Groot-65B-A6B-GyroM"
source_files = ["Groot-65B-A6B-GyroM-Q2_0.gguf"]
quant = "Q2_0"
imatrix = "imatrix.dat"
sha256 = "…"
built_at = "2026-11-12T18:45:12Z"
built_by = "ropero 2.0.0"
engine = "llama-server @ <commit>"
```

**A GGUF without a sidecar is not considered to belong to Ropero.** `ropero install models` warns about it and `ropero doctor` flags it. This makes an artifact **auditable**: one can find where it came from and what was used to build it without asking anyone.

The `sha256` is for the final file and is checked **before** use: if the file changed on disk, it is a different file.

### 11.4 Open question: foreign engines

Doc A explicitly leaves this open (R-EST-15 🔶, H-26), and this document **does not decree it closed**. It does establish the framework:

- **Reporting and stopping are different things.** Detecting a foreign engine is cheap; stopping it is not, and will not be done.
- **On Linux** there is direct data: `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader`. Cross-reference GPU PIDs with Ropero-managed PIDs; the difference is foreign processes.
- **On macOS** there is no direct equivalent. `ioreg` gives system information, not compute-process information. Detection on macOS remains **open** and is documented as unresolved.

**What F4 does implement**, because it addresses the most annoying symptom (“Ropero does not start and does not say why”): when the port is occupied by a process that is **not** managed by Ropero, the error says so explicitly:

```
ERROR: port 9999 is occupied by a foreign process (pid 42137, llama-server)
       Ropero will not stop it: it does not own it.
       Options: use another port (--port 9997), or stop that process yourself.
```

That applies P-2 and P-13, turning an unexplained “does not start” into a named problem.

---

## 12. Observability

### 12.1 Traces

`tracing` with two formats: `pretty` for a human, `json` for a machine. Set the level with `ROPERO_LOG=debug|trace|info|warn|error`.

Each long operation emits a span with its duration:

```
INFO ropero::runtime::supervisor: starting engine
  ├─ model=groot engine=llama-server port=9999 level=high ctx=65536
  ├─ 04-launch.sh:216 elapsed=42.1s
  └─ probe=inference elapsed=3.4s status=ready
```

**Probe time belongs in the trace.** When a model takes 90 seconds to start, the question is always “how long did the engine take, and how long did Ropero take to notice?” Without that distinction, there is no diagnosis.

### 12.2 `ropero doctor`

The v2 `doctor` is the diagnostic instrument, and its output is **actionable** by construction (P-13). Each line has one of these forms:

| Form | Meaning |
|---|---|
| `[✓] <check>` | Passes |
| `[✗] <check> — <reason> — <action>` | Fails, with reason and action |
| `[!] <check> — <reason>` | Warning: not blocking, but useful to know |
| `[-] <check>` | Could not be checked (and why) |

**A `[✗]` without an action is a `doctor` bug.** If `doctor` says something is wrong but not what to do, the user must investigate something Ropero already knows.

The v2 `doctor` checks:

1. The binary and its version.
2. Model directory and number of TOMLs (active and `fired/`).
3. TOMLs that **do not validate** (with file and line).
4. Installed engines and engines compatible with this machine.
5. Engines **installed but incompatible** (with reason).
6. Engines **compatible but not installed**.
7. Managed ports and their actual status (probe).
8. Ports occupied by foreign processes.
9. Models whose file is missing or empty.
10. Models whose GGUF has no provenance sidecar.
11. Free disk space compared with model requirements.
12. RAM compared with `[limits].min_ram_gb`.
13. API key: present, absent, or permissions too open.
14. opencode plugin: installed and consistent with the version.

### 12.3 `ropero stats`

Samples CPU, RAM, and GPU for the engine process and system. Without a GPU, reports process and system information; with a GPU, also GPU information (via `nvidia-smi` on Linux, `ioreg`/`powermetrics` on macOS when feasible).

**`doctor` and `stats` share the sampling layer** (R-CLI-17 🔶): their figures cannot diverge because they come from the same code.

---

## 13. What this document leaves open

A document that pretends to close everything is hiding gaps. These are the real ones:

| # | Gap | Why it is not closed here | What is needed |
|---|---|---|---|
| H-26 | Detecting foreign engines on macOS | No direct API | A reliable method, or accept it as unresolved and say so (§ 11.4) |
| H-a | Distribution outside the repo (Homebrew, AUR) | Owner decision | A statement from the owner |
| H-b | Dependency update policy | Depends on maintainer | Agreed cadence (weekly/monthly) |
| H-c | Support for more than two operating systems | Out of scope | Owner decision |
| H-d | Message internationalization | Current repo is Spanish and does not request it | Owner decision |
| H-e | Persistent metrics (usage history) | Nobody requested them | Format and storage location |

**All five are recorded as open debt in doc A** (or as items not covered by doc A). None blocks the plan: the ten phases can be executed without closing them.

---

## 14. Glossary

| Term | Meaning |
|---|---|
| **engine** | An external program that serves a model: llama.cpp, MLX, TensorRT-LLM, vLLM, AirLLM, FLM, Strata. |
| **model** | A data file (TOML) declaring what to serve, with which engine and arguments. |
| **artifact** | The file being served: a GGUF, a checkpoint, a Strata package. |
| **probe** | Check whether an engine is ready. Three levels: TCP, HTTP, inference. |
| **stamp** | Hash of model + context + level + files. Detects whether configuration changed. |
| **state** | File at `$STATE_DIR/ports/<port>` that says what is running. |
| **identity** | Tuple `pid + start_time + binary digest` proving a process is ours. |
| **provenance** | Sidecar stating where a GGUF came from and how it was built. |
| **checkpoint** | Record of how far a forge phase progressed, allowing retry of only the expensive step. |
| **gate** | Definition of Done: what is checked and where. |
| **golden master** | v1 behavior frozen as a test's expected value. |
| **ADR** | Architecture Decision Record: why a decision was made. |
| **wound** | Measured defect: a v1 repo defect the test suite does not detect. |

---

## 15. Provenance of this document

**Date**: 2026-11-12.
**Primary source**: `lorenzo-sf/ropero` repo at commit `f21b442`, 147 files, 14,839 lines of bash and 4,803 of Python, 8 engines, 45 TOMLs (17 active + 28 retired), 16 test files.
**Secondary source**: `ropero-especificacion.md`, with 317 requirements, 281 of them with `file:line` evidence.
**Versions**: verified against the crates.io index and `static.rust-lang.org` on 2026-11-12 (§ 2.2, § 2.3).
**Deliverable**: this document and `ropero-especificacion.md`, read together.

### 15.1 What this document did **not** verify

- **That v1 code meets every requirement.** Doc A describes what the code does; where code and doc A disagree, doc A records the debt. This document did not rerun the v1 suite.
- **That crate versions work together.** The **existing** versions were verified; the specific combination is validated in F0.
- **That the phase plan is optimal.** It is an **executable** plan, not necessarily the shortest. It can be reordered if new evidence appears.
