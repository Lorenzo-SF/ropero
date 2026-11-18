//! El arranque de un modelo, de principio a fin (doc CLI v2 §8).
//!
//! Es la pieza que la CLI llama: recibe lo que el usuario pidió y devuelve
//! el código de salida. La CLI **no decide nada** (R-ARQ-1): despacha e
//! imprime.
//!
//! ## El camino
//!
//! ```text
//! 1. El modelo tiene que estar en el catálogo.      → si no, uso (2)
//! 2. El engine del modelo tiene que estar instalado. → si no, error (1)
//! 3. El argv se monta con `build_argv`.              → si no, uso (2)
//! 4. El preflight: recursos.                         → si no, error (1)
//! 5. El arranque: spawn, sonda, estado.              → si no, error (1)
//! 6. En primer plano: esperar, con Ctrl-C → 130.
//! ```
//!
//! ## Los códigos de salida (doc CLI v2 §3)
//!
//! | Código | Significado |
//! |---:|---|
//! | `0` | Ok |
//! | `1` | Error |
//! | `2` | Uso (argumento o comando inválido) |
//! | `130` | Ctrl-C |

use std::path::PathBuf;
use std::time::Duration;

use ropero_core::{
    Catalog, EngineId, EngineManifest, Environment, LaunchRequest, ModelSpec, Preflight,
    build_argv, resolve_context, resolve_paths,
};

use crate::launch::{LaunchContext, LaunchOptions, LaunchSpec, ProbePlan};
use crate::state::PortState;

/// Lo que el usuario pidió en la línea de órdenes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunRequest {
    /// El modelo a arrancar.
    pub model: String,

    /// `--force`: mata los procesos de ropero que ocupan el device.
    pub force: bool,

    /// `--cpu`: fuerza el offload 100 % a CPU/RAM.
    pub cpu: bool,

    /// `--port`: el puerto pedido.
    pub port: Option<u16>,

    /// `--api-key`: la clave de API del gateway.
    pub api_key: Option<String>,

    /// `--no-api-key`: arranca sin clave de API.
    pub no_api_key: bool,

    /// `--ctx`: el contexto pedido.
    pub context: Option<u32>,

    /// El nivel de razonamiento pedido.
    pub level: Option<String>,

    /// Flags declarados por el modelo en `[flags]`.
    pub model_flags: Vec<String>,

    /// Opciones declaradas por el modelo en `[options]`.
    pub model_options: Vec<(String, String)>,

    /// Nivel común de logs solicitado por el usuario.
    pub log_level: Option<String>,

    /// `--background`: arranca desacoplado y sin salida a la terminal.
    pub background: bool,
}

impl RunRequest {
    /// Si el usuario ha pedido la clave de API de alguna manera.
    ///
    /// `--no-api-key` **manda**: sin clave, no se genera ninguna y no se
    /// manda ninguna (doc CLI v2 §8.2).
    pub fn quiere_api_key(&self) -> bool {
        !self.no_api_key
    }
}

/// Lo que pasó en el arranque: lo que la CLI tiene que imprimir.
pub struct RunOutcome {
    /// El código de salida.
    pub code: i32,

    /// El puerto en el que sirve (si arrancó).
    pub port: Option<u16>,

    /// El PID del motor (si arrancó).
    pub pid: Option<u32>,

    /// El mensaje de error (si lo hubo).
    pub error: Option<String>,

    /// Si el usuario lo interrumpió con Ctrl-C.
    pub interrupted: bool,
}

impl RunOutcome {
    /// Un arranque correcto.
    pub fn ok(port: u16, pid: u32) -> Self {
        Self {
            code: 0,
            port: Some(port),
            pid: Some(pid),
            error: None,
            interrupted: false,
        }
    }

    /// Un error de ejecución (código 1).
    pub fn error(mensaje: impl Into<String>) -> Self {
        Self {
            code: 1,
            port: None,
            pid: None,
            error: Some(mensaje.into()),
            interrupted: false,
        }
    }

    /// Un error de uso (código 2).
    pub fn uso(mensaje: impl Into<String>) -> Self {
        Self {
            code: 2,
            port: None,
            pid: None,
            error: Some(mensaje.into()),
            interrupted: false,
        }
    }

    /// Una interrupción por Ctrl-C (código 130).
    pub fn interrumpido() -> Self {
        Self {
            code: 130,
            port: None,
            pid: None,
            error: None,
            interrupted: true,
        }
    }
}

/// Las piezas del arranque que ya están resueltas.
pub struct RunPlan {
    /// El modelo del catálogo.
    pub spec: ModelSpec,

    /// El manifiesto del motor del modelo.
    pub engine: EngineManifest,

    /// El argv del modelo, ya montado.
    pub argv: Vec<String>,

    /// El nivel de razonamiento con el que arranca.
    pub level: String,

    /// El contexto que se sirve.
    pub context: u32,

    /// Nivel común de logs pedido (si el motor ofrece control nativo).
    pub log_level: Option<String>,
}

/// Prepara el arranque: busca el modelo, su motor y monta el argv.
///
/// `Err` con el código 2 si el modelo no está o el argv no se puede
/// montar (uso); con el código 1 si el motor no está instalado (ejecución).
pub fn preparar(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
    peticion: &RunRequest,
) -> Result<RunPlan, RunOutcome> {
    // 1. El modelo tiene que estar en el catálogo.
    let Some(spec) = ropero_core::find_spec(catalogo, &peticion.model) else {
        return Err(RunOutcome::error(format!(
            "modelo desconocido: '{}' (ropero models)",
            peticion.model
        )));
    };

    // 2. El motor del modelo.
    let Some(engine) = motores.iter().find(|m| m.id == spec.model.engine).cloned() else {
        return Err(RunOutcome::error(format!(
            "el motor '{}' del modelo '{}' no tiene manifiesto: \
             no se puede arrancar (ropero install engines)",
            spec.model.engine.as_str(),
            peticion.model
        )));
    };
    if engine.id == EngineId::Ignis && peticion.cpu {
        return Err(RunOutcome::error(
            "Ignis requiere una GPU NVIDIA compatible y no admite '--cpu'",
        ));
    }

    // 3. El argv del modelo. El `--ctx` sobrescribe el contexto del TOML.
    let mut ctx = resolve_context(spec, env);
    if let Some(contexto) = peticion.context {
        ctx.context = contexto;
    }
    let peticion_argv = LaunchRequest {
        level: peticion.level.clone(),
        flags: peticion.model_flags.clone(),
        options: peticion.model_options.clone(),
    };
    let model_argv = build_argv(spec, &peticion_argv, &ctx)
        .map_err(|e| RunOutcome::error(ropero_core::surface::argv_error_message(&e.to_string())))?;
    let mut argv = if engine.id == EngineId::Strata {
        let profile = crate::strata::profile_for(&engine, spec, env).map_err(RunOutcome::error)?;
        let mut strata_argv = crate::strata::serve_argv(&profile);
        strata_argv.extend(model_argv);
        strata_argv
    } else if ropero_core::flm::is_flm_model(spec)
        && (engine.serve.is_some() || engine.install.is_some())
    {
        crate::flm::serve_argv(&engine, spec, &model_argv, Some(ctx.context))
            .map_err(RunOutcome::error)?
    } else {
        model_argv
    };
    if engine.id == EngineId::MlxServe {
        configura_mlx_argv(
            &mut argv,
            spec,
            &ctx.files.model_path,
            ctx.files.gguf.as_deref(),
        )
        .map_err(RunOutcome::error)?;
    }
    // Ignis exposes its model through /v1/models using model-id. Keep the
    // catalog name for Ropero's paths/state, but honor the engine-facing alias.
    if engine.id == EngineId::Ignis
        && let Some(alias) = spec.model.alias.as_deref()
    {
        set_flag_value(&mut argv, "--model-id", alias);
    }
    // llama.cpp otherwise publishes the GGUF path as `/v1/models` id.
    // Connect uses the public alias, so make the serving id deterministic.
    if matches!(engine.id, EngineId::LlamaServer | EngineId::TurboLlama) {
        let alias = spec.model.alias.as_deref().unwrap_or(&spec.model.name);
        set_flag_value(&mut argv, "--alias", alias);
        if !argv.iter().any(|arg| arg == "--jinja") {
            argv.push("--jinja".into());
        }
    }
    if let Some(alias) = spec.model.alias.as_deref() {
        match engine.id {
            EngineId::Vllm => set_flag_value(&mut argv, "--served-model-name", alias),
            EngineId::TensorRtLlm => set_flag_value(&mut argv, "--served_model_name", alias),
            _ => {}
        }
    }
    // llama-server y turbo-llama no reciben el GGUF como argv implícito:
    // sin `-m`, versiones recientes arrancan un router vacío y el servidor
    // rechaza `model=<alias>` aunque Ropero haya encontrado el artefacto.
    if matches!(engine.id, EngineId::LlamaServer | EngineId::TurboLlama)
        && spec.model.format == ropero_core::ModelFormat::Gguf
        && !argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "-m" | "--model"))
        && let Some(gguf) = ctx.files.gguf.as_deref()
    {
        argv.extend(["-m".into(), gguf.into()]);
    }
    if let Some(level) = peticion.log_level.as_deref() {
        match engine.id {
            EngineId::LlamaServer | EngineId::TurboLlama => {
                let verbosity = match level {
                    "error" => "1",
                    "warn" => "2",
                    "info" => "3",
                    "trace" => "4",
                    "debug" => "5",
                    _ => unreachable!("log level validated in peticion_de"),
                };
                argv.extend(["-lv".into(), verbosity.into()]);
            }
            EngineId::Vllm => {
                let level = match level {
                    "error" => "ERROR",
                    "warn" => "WARNING",
                    "info" => "INFO",
                    "debug" | "trace" => "DEBUG",
                    _ => unreachable!("log level validated in peticion_de"),
                };
                argv.extend(["--log-level".into(), level.into()]);
            }
            EngineId::MlxServe => {
                let level = if level == "warn" { "warn" } else { level };
                argv.extend(["--log-level".into(), level.into()]);
            }
            other => {
                return Err(RunOutcome::error(format!(
                    "el motor '{}' no ofrece un nivel de log configurable por CLI",
                    other.as_str()
                )));
            }
        }
    }

    // El nivel con el que arranca: el pedido, o el defecto del TOML.
    let level = peticion
        .level
        .clone()
        .or_else(|| spec.levels.default.clone())
        .unwrap_or_default();

    Ok(RunPlan {
        spec: spec.clone(),
        engine,
        argv,
        level,
        context: ctx.context,
        log_level: peticion.log_level.clone(),
    })
}

fn set_flag_value(argv: &mut Vec<String>, flag: &str, value: &str) {
    if let Some(index) = argv.iter().position(|arg| arg == flag) {
        if let Some(argument) = argv.get_mut(index + 1) {
            *argument = value.to_string();
        } else {
            argv.push(value.to_string());
        }
    } else if let Some(argument) = argv
        .iter_mut()
        .find(|argument| argument.starts_with(&format!("{flag}=")))
    {
        *argument = format!("{flag}={value}");
    } else {
        argv.extend([flag.to_string(), value.to_string()]);
    }
}

/// MLX-Serve necesita que el checkpoint y el bind local formen parte del
/// comando; su CLI escucha en todas las interfaces si no se fija `--host`.
fn configura_mlx_argv(
    argv: &mut Vec<String>,
    spec: &ropero_core::ModelSpec,
    model_path: &str,
    gguf_path: Option<&str>,
) -> Result<(), String> {
    let path = match spec.model.format {
        ropero_core::ModelFormat::Safetensors => model_path,
        ropero_core::ModelFormat::Gguf => gguf_path.unwrap_or_default(),
        _ => "",
    };
    if path.is_empty() {
        return Err(format!(
            "MLX-Serve necesita una ruta local para el modelo '{}'",
            spec.model.name
        ));
    }
    set_flag_value(argv, "--model", path);
    set_flag_value(argv, "--host", "127.0.0.1");
    Ok(())
}

/// El preflight de un plan: lo que hay que comprobar antes de lanzar.
pub fn preflight_de(
    plan: &RunPlan,
    env: &Environment,
    engine_installed: bool,
    model_installed: bool,
    cpu: bool,
) -> Preflight {
    Preflight {
        engine: plan.engine.id.as_str().to_string(),
        engine_installed,
        model_installed,
        // La VRAM y la RAM: lo que se pudo leer. Lo que no se pudo leer se
        // queda `None` y no bloquea (doc B §3.2.3).
        vram_available_bytes: (plan.engine.id != EngineId::Ignis)
            .then(vram_libre)
            .flatten(),
        ram_available_bytes: ram_libre(),
        context_requested: plan.context,
        // El modelo no declara cuánta VRAM necesita: se usa el tamaño del
        // artefacto, que es lo que hay.
        // Ignis has its own load-time VRAM/host plan (Flash-Next streams
        // weights from host RAM); artifact size is not a VRAM estimate.
        vram_required_bytes: (plan.engine.id != EngineId::Ignis)
            .then(|| tamano_modelo(plan, env))
            .flatten(),
        ram_required_bytes: tamano_modelo(plan, env),
        cpu,
    }
}

/// El tamaño del artefacto del modelo, en bytes.
///
/// Viene del `[download].size` del TOML, que es una cadena como "~4 GB":
/// se parsea el número y la unidad. Si no se puede, `None`: lo que no se
/// sabe no bloquea.
pub fn tamano_modelo(plan: &RunPlan, env: &Environment) -> Option<u64> {
    let _ = env;
    let descarga = plan.spec.download.as_ref()?;
    let texto = descarga.size.trim();
    let numero: String = texto
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let numero: f64 = numero.parse().ok()?;
    let superior = texto.to_uppercase();
    let bytes = if superior.contains("TIB") || superior.contains("TB") {
        numero * 1024.0 * 1024.0 * 1024.0 * 1024.0
    } else if superior.contains("GIB") || superior.contains("GB") {
        numero * 1024.0 * 1024.0 * 1024.0
    } else if superior.contains("MIB") || superior.contains("MB") {
        numero * 1024.0 * 1024.0
    } else if superior.contains("KIB") || superior.contains("KB") {
        numero * 1024.0
    } else if superior.contains('B') {
        numero
    } else {
        return None;
    };
    Some(bytes as u64)
}

/// La VRAM libre, en bytes.
pub fn vram_libre() -> Option<u64> {
    let salida = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    let texto = String::from_utf8_lossy(&salida.stdout);
    let primera = texto.lines().next()?;
    let campos: Vec<&str> = primera.split(',').map(|c| c.trim()).collect();
    let total: f64 = campos.first()?.parse().ok()?;
    let usado: f64 = campos.get(1)?.parse().ok()?;
    let libre = total - usado;
    if libre <= 0.0 {
        return None;
    }
    Some((libre * 1024.0 * 1024.0 * 1024.0) as u64)
}

/// La RAM libre, en bytes.
pub fn ram_libre() -> Option<u64> {
    let texto = std::fs::read_to_string("/proc/meminfo").ok()?;
    let disponible = ropero_core::sysinfo::meminfo_mb(&texto).1?;
    Some((disponible * 1024.0 * 1024.0) as u64)
}

/// El log del motor: `$LOG_DIR/<modelo>.log`.
pub fn log_de(spec: &ModelSpec, env: &Environment) -> PathBuf {
    PathBuf::from(resolve_paths(spec, env).log)
}

/// El estado en disco de un puerto: para consultarlo sin escribir.
pub fn estado_de(state_dir: &std::path::Path, port: u16) -> Option<PortState> {
    crate::state::read_state(state_dir, port)
}

/// Estados de procesos de Ropero cuya identidad sigue viva. Nunca se usa
/// sólo el PID: también debe coincidir `start_time` para evitar PIDs reciclados.
fn procesos_gestionados(
    state_dir: &std::path::Path,
) -> Vec<(crate::process::ProcIdentity, PortState)> {
    use crate::launch::Device;
    let vivos = crate::device::StateDevice::new(state_dir).ropero_processes();
    let mut resultado = Vec::new();
    for puerto in crate::state::managed_ports(state_dir) {
        let Some(estado) = crate::state::read_state(state_dir, puerto) else {
            continue;
        };
        let Some(pid) = estado.pid else { continue };
        if let Some(identidad) = vivos.iter().find(|p| p.pid == pid) {
            resultado.push((*identidad, estado));
        }
    }
    resultado
}

/// Los puertos del rango que **están ocupados** (alguien escucha en ellos).
///
/// La pregunta se hace al disco: se intenta **conectar** a cada puerto
/// del rango. Si alguien acepta la conexión, el puerto está ocupado.
///
/// Es O(rango) conexiones, pero el rango de un motor son **cien** puertos
/// como mucho (doc CLI v2 §5.1) y la conexión a un puerto libre falla
/// enseguida: no cuesta nada.
pub fn puertos_ocupados(rango: &ropero_core::PortRange) -> Vec<u16> {
    let mut ocupados = Vec::new();
    for puerto in rango.min..=rango.max {
        if crate::probe::tcp_ready(puerto, std::time::Duration::from_millis(200)) {
            ocupados.push(puerto);
        }
    }
    ocupados
}

fn puerto_ocupado(puerto: u16) -> bool {
    crate::probe::tcp_ready(puerto, Duration::from_millis(200))
}

/// El plan de sonda por defecto: dos minutos de margen.
pub fn plan_de_sonda() -> ProbePlan {
    ProbePlan {
        backoff: Duration::from_millis(500),
        deadline: Duration::from_secs(120),
    }
}

/// Las opciones de arranque para una petición.
pub fn opciones_de(peticion: &RunRequest, foreground: bool) -> LaunchOptions {
    LaunchOptions {
        probe: plan_de_sonda(),
        foreground,
        force: peticion.force,
        cpu: peticion.cpu,
    }
}

/// La petición de arranque a partir de los valores crudos de la línea de
/// órdenes.
///
/// Los valores mal formados (`--port 0`, `--ctx no-es-un-número`) son
/// **errores de uso** (código 2): el uso es incorrecto, no el arranque.
#[allow(clippy::too_many_arguments)]
pub fn peticion_de(
    modelo: &str,
    force: bool,
    cpu: bool,
    port: Option<&str>,
    api_key: Option<&str>,
    no_api_key: bool,
    ctx: Option<&str>,
    background: bool,
    log_level: Option<&str>,
) -> Result<RunRequest, RunOutcome> {
    let log_level = log_level.map(str::to_ascii_lowercase);
    if log_level
        .as_deref()
        .is_some_and(|level| !["error", "warn", "info", "debug", "trace"].contains(&level))
    {
        return Err(RunOutcome::uso(
            "'--log-level' debe ser error, warn, info, debug o trace",
        ));
    }
    let mut peticion = RunRequest {
        model: modelo.to_string(),
        force,
        cpu,
        port: None,
        api_key: api_key.map(|c| c.to_string()),
        no_api_key,
        context: None,
        level: None,
        model_flags: Vec::new(),
        model_options: Vec::new(),
        log_level,
        background,
    };
    if let Some(valor) = port {
        match valor.parse::<u16>() {
            Ok(puerto) if puerto > 0 => peticion.port = Some(puerto),
            _ => {
                return Err(RunOutcome::uso(format!(
                    "'--port {valor}' no es un puerto válido (1-65535)"
                )));
            }
        }
    }
    if let Some(valor) = ctx {
        match valor.parse::<u32>() {
            Ok(contexto) => peticion.context = Some(contexto),
            Err(_) => {
                return Err(RunOutcome::uso(format!(
                    "'--ctx {valor}' no es un número de contexto válido"
                )));
            }
        }
    }
    Ok(peticion)
}

/// El arranque completo, **de principio a fin**, como lo ve el usuario.
///
/// Es lo que llama la CLI, y es la pieza que se lleva el pegamento: la
/// CLI sólo despacha (R-ARQ-1) y el umbral de líneas del CLI (ADR-005)
/// obliga a que la orquestación viva aquí.
///
/// En primer plano escribe `ROPERO_PORT=<n>` en stdout cuando el motor está
/// listo; en background no escribe nada en la terminal.
pub fn arranca_todo(
    peticion: &RunRequest,
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
) -> i32 {
    let plan = match preparar(catalogo, motores, env, peticion) {
        Ok(p) => p,
        Err(outcome) => return informa(outcome),
    };
    let state_dir = crate::state::state_dir(&env.home, env.get("ROPERO_STATE_DIR"));
    let foreground = !peticion.background;
    let mut callback = |puerto: u16, _pid: u32| {
        if foreground {
            ropero_core::out_line(&format!("ROPERO_PORT={puerto}"));
        }
    };
    let outcome = arranca(&plan, peticion, env, &state_dir, foreground, &mut callback);
    if outcome.code == 0 {
        return 0;
    }
    informa(outcome)
}

/// La parada de toda la vida, como la ve el usuario.
///
/// Sin modelo para **todos**; con modelo, sólo el de ese modelo. Escribe
/// el informe y devuelve el código de salida.
pub fn para_todo(modelo: &str, env: &Environment) -> i32 {
    let state_dir = crate::state::state_dir(&env.home, env.get("ROPERO_STATE_DIR"));
    let stopper = crate::sys_engine::OsStopper;
    let deadline = Duration::from_secs(10);
    let informe = if modelo.is_empty() {
        crate::stop::SysStopper::parar_todo(&state_dir, &stopper, deadline)
    } else {
        crate::stop::SysStopper::parar_modelo(&state_dir, modelo, &stopper, deadline)
    };
    let texto = informe.texto();
    let texto = texto.trim_end();
    if informe.vacio() {
        ropero_core::err_line(texto);
    } else {
        ropero_core::out_line(texto);
    }
    if informe.no_parados.is_empty() { 0 } else { 1 }
}

/// Registra el uso de un arranque. No guarda secretos.
fn registra_uso(env: &Environment, plan: &RunPlan, peticion: &RunRequest, outcome: &RunOutcome) {
    let resultado = if outcome.interrupted {
        "interrupted"
    } else if outcome.code == 0 {
        "ok"
    } else if outcome.code == 2 {
        "uso"
    } else {
        "error"
    };
    let modo = if peticion.background {
        "background"
    } else {
        "foreground"
    };
    let record = crate::usage::UsageRecord {
        timestamp: crate::usage::now_ts(),
        model: plan.spec.model.name.clone(),
        engine: plan.engine.id.as_str().to_string(),
        port: outcome.port,
        result: resultado.to_string(),
        mode: modo.to_string(),
    };
    if let Err(e) = crate::usage::append_usage(&crate::usage::logs_dir(env), &record) {
        ropero_core::err_line(&format!("error: no se pudo escribir el log de uso: {e}"));
    }
}

/// Imprime el error de un arranque fallido y devuelve su código.
fn informa(outcome: RunOutcome) -> i32 {
    if let Some(mensaje) = &outcome.error {
        ropero_core::err_line(mensaje.trim_end());
    }
    outcome.code
}

/// El arranque completo, con las piezas de producción.
///
/// `al_arrancar` se llama en cuanto el motor está listo, **antes** de
/// esperar. En foreground el puerto se comunica al terminal; en background
/// no hay salida a la terminal.
pub fn arranca(
    plan: &RunPlan,
    peticion: &RunRequest,
    env: &Environment,
    state_dir: &std::path::Path,
    foreground: bool,
    mut al_arrancar: impl FnMut(u16, u32),
) -> RunOutcome {
    // El motor tiene que estar instalado: sin binario no hay nada que
    // lanzar.
    let binario = crate::sys_engine::binary_of(&plan.engine, env);
    if binario.is_none() {
        let outcome = RunOutcome::error(format!(
            "el motor '{}' no está instalado: instálalo con 'ropero install engines'",
            plan.engine.id.as_str()
        ));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }
    let binario = binario.expect("el binario existe");

    // El modelo tiene que estar disponible. Para FLM delegado, el tag declarado
    // es suficiente para intentar servir (el almacén es de FLM).
    let model_ok = if plan.engine.id == EngineId::Strata {
        plan.argv
            .windows(2)
            .find(|pair| pair[0] == "--profile")
            .is_some_and(|pair| std::path::Path::new(&pair[1]).is_file())
    } else if ropero_core::flm::is_flm_model(&plan.spec)
        && ropero_core::flm::primary_tag(&plan.spec).is_some()
        && crate::flm::delegated(&plan.engine)
    {
        true
    } else {
        crate::catalog_scan::model_downloaded(&plan.spec, env)
    };
    if !model_ok {
        let outcome = RunOutcome::error(format!(
            "el modelo '{}' no está descargado: instálalo con 'ropero install models'",
            peticion.model
        ));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    if plan.engine.id == EngineId::MlxServe
        && let Err(error) = crate::mlxserve::aplica_alias(&plan.spec, env)
    {
        let outcome = RunOutcome::error(error);
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // FLM consulta la NPU a través de XRT (no basta con que `flm validate`
    // vea `/dev/accel`). Comprueba runtime y plugin antes de reservar puertos
    // o ejecutar `--force`, para que una pila incompatible falle sin efectos.
    if plan.engine.id == EngineId::FastFlowLm
        && let Err(error) = crate::install::valida_runtime_flm(env)
    {
        let outcome = RunOutcome::error(error);
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // Detecta primero los bloqueos que `--force` no puede resolver. Así no
    // se detienen modelos gestionados cuando el puerto explícito pertenece a
    // un proceso externo, o cuando todos los puertos sólo los ocupa software
    // que Ropero no gestiona.
    let managed_before_force = procesos_gestionados(state_dir);
    let occupied_before_force = peticion.port.map_or_else(
        || puertos_ocupados(&plan.engine.port_range),
        |p| {
            if puerto_ocupado(p) {
                vec![p]
            } else {
                Vec::new()
            }
        },
    );
    let requested_port_external = peticion.port.is_some_and(|p| {
        occupied_before_force.contains(&p)
            && !managed_before_force.iter().any(|(_, s)| s.port == Some(p))
    });
    let range_full_external = peticion.port.is_none()
        && occupied_before_force.len()
            == (plan.engine.port_range.max - plan.engine.port_range.min + 1) as usize
        && !occupied_before_force
            .iter()
            .any(|p| managed_before_force.iter().any(|(_, s)| s.port == Some(*p)));
    if requested_port_external || range_full_external {
        let ports = occupied_before_force
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let outcome = RunOutcome::error(format!(
            "puerto(s) {ports} ocupado(s) por procesos externos o no gestionados por Ropero; no se detuvo ningún proceso. Libera el puerto o elige otro con '--port'"
        ));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // `--force` sólo actúa después de comprobar que el nuevo modelo puede
    // intentarse. Se enumeran procesos vivos del estado de Ropero, se pide
    // confirmación y luego se vuelve a medir memoria en el preflight.
    if peticion.force {
        let afectados = procesos_gestionados(state_dir);
        if !afectados.is_empty() {
            if !crate::install::hay_tty() {
                let nombres = afectados
                    .iter()
                    .map(|(_, s)| {
                        format!(
                            "{} ({}, puerto {})",
                            s.model,
                            s.engine,
                            s.port.unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let outcome = RunOutcome::error(format!(
                    "--force necesita confirmación interactiva para detener: {nombres}; vuelve a ejecutarlo desde un terminal"
                ));
                registra_uso(env, plan, peticion, &outcome);
                return outcome;
            }
            ropero_core::out_line("Procesos de Ropero que se detendrán:");
            for (_, estado) in &afectados {
                ropero_core::out_line(&format!(
                    "  - {} ({}, puerto {})",
                    estado.model,
                    estado.engine,
                    estado.port.unwrap_or_default()
                ));
            }
            let respuesta = crate::install::pregunta("¿Detenerlos y continuar? [s/N]: ");
            if !respuesta.is_some_and(|r| {
                matches!(r.trim().to_ascii_lowercase().as_str(), "s" | "si" | "sí")
            }) {
                let outcome =
                    RunOutcome::error("arranque cancelado; no se detuvo ningún modelo".to_string());
                registra_uso(env, plan, peticion, &outcome);
                return outcome;
            }
            crate::stop::SysStopper::parar_todo(
                state_dir,
                &crate::sys_engine::OsStopper,
                Duration::from_secs(10),
            );
        }
    }

    // El preflight: los recursos.
    let preflight = preflight_de(
        plan,
        env,
        crate::catalog_scan::engine_installed_with_env(&plan.engine, env),
        model_ok,
        peticion.cpu,
    );
    if let Err(e) = preflight.check() {
        let detalle = if !peticion.force {
            let gestionados = procesos_gestionados(state_dir);
            if gestionados.is_empty() {
                format!(
                    "{e}; no hay modelos activos gestionados por Ropero que pueda detener. Libera VRAM/RAM de otros procesos o reduce el modelo/contexto"
                )
            } else {
                let modelos = gestionados
                    .iter()
                    .map(|(_, s)| format!("{} ({})", s.model, s.engine))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{e}; hay modelos gestionados por Ropero que podrían liberar recursos: {modelos}. Si quieres detenerlos, vuelve a ejecutar con '--force' y confirma"
                )
            }
        } else {
            format!(
                "{e}; Ropero ya detuvo los modelos confirmados. Puede faltar capacidad propia del modelo o estar ocupada por procesos externos que Ropero no gestiona"
            )
        };
        let outcome = RunOutcome::error(detalle);
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // 2. Los puertos ocupados del rango del motor.
    let ports = if peticion.port.is_some() {
        Vec::new()
    } else {
        puertos_ocupados(&plan.engine.port_range)
    };
    let ocupados_requeridos = peticion.port.map_or_else(
        || ports.len() == (plan.engine.port_range.max - plan.engine.port_range.min + 1) as usize,
        puerto_ocupado,
    );
    if ocupados_requeridos {
        let ocupados = peticion.port.map_or_else(|| ports.clone(), |p| vec![p]);
        let gestionados = procesos_gestionados(state_dir);
        let detalle = ocupados
            .iter()
            .map(|p| {
                gestionados
                    .iter()
                    .find(|(_, s)| s.port == Some(*p))
                    .map(|(_, s)| format!("{p} ({} / {})", s.model, s.engine))
                    .unwrap_or_else(|| format!("{p} (proceso externo o no gestionado por Ropero)"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let outcome = RunOutcome::error(format!(
            "puerto(s) ocupado(s): {detalle}. Libera un puerto o elige otro con '--port'; Ropero sólo puede detener procesos que gestiona y confirma antes de hacerlo"
        ));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    let log = log_de(&plan.spec, env);
    // El flujo normal siempre genera fichero de log para fallos/status.
    if let Some(dir) = log.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        let outcome = RunOutcome::error(format!("no se pudo preparar el directorio de logs: {e}"));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // Resuelve la clave justo antes de lanzar el proceso. Así no se cambia
    // el estado persistente si el preflight o la selección del puerto fallan.
    let clave = match crate::api_key::launch_key(
        plan.engine.id,
        &env.config_resuelta(),
        env,
        state_dir,
        peticion.api_key.as_deref(),
        peticion.no_api_key,
    ) {
        Ok(key) => key,
        Err(error) => {
            let outcome = RunOutcome::error(error);
            registra_uso(env, plan, peticion, &outcome);
            return outcome;
        }
    };
    let server_api_key = clave.clone();
    let probe = crate::probe::NetProbe {
        timeout: Duration::from_secs(3),
        api_key: clave.clone(),
    };

    let spec = LaunchSpec {
        model: plan.spec.model.name.clone(),
        engine: plan.engine.id,
        engine_name: plan.engine.id.as_str().to_string(),
        server_api_key,
        runtime_env: if plan.engine.id == EngineId::FastFlowLm {
            crate::flm::xrt_environment(&env.vars)
        } else {
            std::collections::BTreeMap::new()
        },
        program: binario,
        argv: plan.argv.clone(),
        level: plan.level.clone(),
        context: plan.context,
        log: log.clone(),
        port_range: plan.engine.port_range,
    };

    let engine = crate::sys_engine::SysEngine::new();
    let device = crate::device::StateDevice::new(state_dir.to_path_buf());
    let ctx = LaunchContext {
        engine: &engine,
        probe: &probe,
        device: &device,
        preflight: &preflight,
        port: peticion.port,
        occupied_ports: ports,
        state_dir,
        // `--force` ya se confirmó y se ejecutó antes de medir recursos.
        options: LaunchOptions {
            force: false,
            ..opciones_de(peticion, foreground)
        },
    };

    // En primer plano se muestra en vivo la salida del motor (colorizada) y
    // el motor escribe además en su log. En background no hay salida a la
    // terminal.
    let follower = if foreground {
        let desde = crate::colorize::line_count(&log);
        let engine_color = ropero_core::palette::engine_hex(plan.engine.color);
        crate::colorize::LogFollower::start(
            log.clone(),
            desde,
            plan.spec.model.engine,
            engine_color,
        )
    } else {
        crate::colorize::LogFollower::none()
    };

    let resultado = match crate::launch::launch(&spec, &ctx) {
        Ok(r) => r,
        Err(e) => {
            follower.stop();
            let outcome = RunOutcome::error(e.to_string());
            registra_uso(env, plan, peticion, &outcome);
            return outcome;
        }
    };

    // El motor está listo: se comunica el puerto (sólo en foreground).
    al_arrancar(resultado.port, resultado.pid);

    if foreground {
        let interruptor = crate::foreground::Interruptor::new();
        if let Err(e) = interruptor.install() {
            use crate::launch::Engine as _;
            engine.terminate(&resultado.identity);
            let _ = engine.wait(resultado.pid, Duration::from_secs(5));
            if engine.alive(resultado.pid) {
                engine.kill(&resultado.identity);
                let _ = engine.wait(resultado.pid, Duration::from_secs(2));
            }
            crate::state::clear_state(state_dir, resultado.port);
            follower.stop();
            let outcome = RunOutcome::error(format!(
                "no se pudo preparar el Ctrl-C: {e}; se intentó detener el motor y limpiar su estado"
            ));
            registra_uso(env, plan, peticion, &outcome);
            return outcome;
        }
        let outcome = match crate::foreground::esperar(&engine, resultado.pid, &interruptor, None) {
            Ok(Some(codigo)) => {
                crate::state::clear_state(state_dir, resultado.port);
                if codigo == 0 {
                    RunOutcome::ok(resultado.port, resultado.pid)
                } else {
                    RunOutcome::error(format!(
                        "el motor de '{}' terminó con el código {codigo}",
                        peticion.model
                    ))
                }
            }
            Ok(None) => {
                crate::state::clear_state(state_dir, resultado.port);
                RunOutcome::ok(resultado.port, resultado.pid)
            }
            Err(_) => {
                use crate::launch::Engine as _;
                engine.terminate(&resultado.identity);
                crate::state::clear_state(state_dir, resultado.port);
                RunOutcome::interrumpido()
            }
        };
        follower.stop();
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }

    // Background: desacoplado, sin espera ni salida a la terminal.
    use crate::launch::Engine as _;
    if let Err(e) = engine.detach(resultado.pid) {
        engine.terminate(&resultado.identity);
        let _ = engine.wait(resultado.pid, Duration::from_secs(5));
        if engine.alive(resultado.pid) {
            engine.kill(&resultado.identity);
            let _ = engine.wait(resultado.pid, Duration::from_secs(2));
        }
        crate::state::clear_state(state_dir, resultado.port);
        follower.stop();
        let outcome = RunOutcome::error(format!(
            "no se pudo desacoplar el proceso en background: {e}"
        ));
        registra_uso(env, plan, peticion, &outcome);
        return outcome;
    }
    follower.stop();
    let outcome = RunOutcome::ok(resultado.port, resultado.pid);
    registra_uso(env, plan, peticion, &outcome);
    outcome
}

#[cfg(test)]
mod tests {
    use super::{configura_mlx_argv, set_flag_value};

    #[test]
    fn ignis_qwen27b_toml_usa_el_alias_expuesto_por_el_engine() {
        let source = include_str!("../../../models/ignis/qwen3.8-27b-16g.toml");
        let model = ropero_core::parse_model(source, std::path::Path::new("qwen3.8-27b-16g.toml"))
            .expect("el TOML de Qwen de Ignis debe seguir siendo válido");

        let base = &model.args.base;
        let model_id = base
            .windows(2)
            .find(|pair| pair[0] == "--model-id")
            .map(|pair| pair[1].as_str());
        assert_eq!(model_id, model.model.alias.as_deref());
        assert_ne!(model_id, Some(model.model.name.as_str()));
    }

    #[test]
    fn alias_served_model_replaces_or_adds_exactly_one_flag() {
        let mut args = vec!["--alias=old".to_string(), "--ctx-size".to_string()];
        set_flag_value(&mut args, "--alias", "new-public-name");
        assert_eq!(args, ["--alias=new-public-name", "--ctx-size"]);

        let mut args = Vec::new();
        set_flag_value(&mut args, "--alias", "catalog-name");
        assert_eq!(args, ["--alias", "catalog-name"]);
    }

    #[test]
    fn los_flags_de_alias_de_vllm_y_tensorrt_sustituyen_el_modelo_servido() {
        let mut vllm = vec!["--served-model-name".into(), "internal".into()];
        set_flag_value(&mut vllm, "--served-model-name", "public");
        assert_eq!(vllm, ["--served-model-name", "public"]);

        let mut tensorrt = Vec::new();
        set_flag_value(&mut tensorrt, "--served_model_name", "public");
        assert_eq!(tensorrt, ["--served_model_name", "public"]);
    }

    #[test]
    fn mlx_usa_la_ruta_del_modelo_y_solo_escucha_en_loopback() {
        let spec = ropero_core::parse_model(
            include_str!("../../../models/mlx/metalqwen.toml"),
            std::path::Path::new("metalqwen.toml"),
        )
        .unwrap();
        let mut args = vec!["--host".into(), "0.0.0.0".into()];
        configura_mlx_argv(&mut args, &spec, "/models/qwen", None).unwrap();
        assert_eq!(args, ["--host", "127.0.0.1", "--model", "/models/qwen"]);
    }
}
