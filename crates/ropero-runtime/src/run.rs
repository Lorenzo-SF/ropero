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
    Catalog, EngineManifest, Environment, LaunchRequest, ModelSpec, Preflight, build_argv,
    resolve_context, resolve_paths,
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
            "modelo desconocido: '{}' (ropero list)",
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

    // 3. El argv del modelo. El `--ctx` sobrescribe el contexto del TOML.
    let mut ctx = resolve_context(spec, env);
    if let Some(contexto) = peticion.context {
        ctx.context = contexto;
    }
    let peticion_argv = LaunchRequest {
        level: peticion.level.clone(),
        flags: Vec::new(),
        options: Vec::new(),
    };
    let argv = build_argv(spec, &peticion_argv, &ctx)
        .map_err(|e| RunOutcome::error(ropero_core::surface::argv_error_message(&e.to_string())))?;

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
    })
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
        vram_available_bytes: vram_libre(),
        ram_available_bytes: ram_libre(),
        context_requested: plan.context,
        // El modelo no declara cuánta VRAM necesita: se usa el tamaño del
        // artefacto, que es lo que hay.
        vram_required_bytes: tamano_modelo(plan, env),
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
    let bytes = if superior.contains("TB") {
        numero * 1024.0 * 1024.0 * 1024.0 * 1024.0
    } else if superior.contains("GB") {
        numero * 1024.0 * 1024.0 * 1024.0
    } else if superior.contains("MB") {
        numero * 1024.0 * 1024.0
    } else if superior.contains("KB") {
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
pub fn peticion_de(
    modelo: &str,
    force: bool,
    cpu: bool,
    port: Option<&str>,
    api_key: Option<&str>,
    no_api_key: bool,
    ctx: Option<&str>,
) -> Result<RunRequest, RunOutcome> {
    let mut peticion = RunRequest {
        model: modelo.to_string(),
        force,
        cpu,
        port: None,
        api_key: api_key.map(|c| c.to_string()),
        no_api_key,
        context: None,
        level: None,
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
/// Devuelve el código de salida y **escribe** en stdout el
/// `ROPERO_PORT=<n>` (el contrato R-OC): es la última línea de stdout, y
/// sale en cuanto el motor está listo, aunque el proceso siga vivo.
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
    let outcome = arranca(&plan, peticion, env, &state_dir, true, |puerto, _| {
        ropero_core::out_line(&format!("ROPERO_PORT={puerto}"));
    });
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
    0
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
/// Es lo que llama la CLI. Todo lo que sale de aquí es **dato**: el
/// código de salida, el puerto y el PID. La CLI los imprime.
///
/// `al_arrancar` se llama en cuanto el motor está listo, **antes** de
/// esperar: es el momento en el que el puerto ya se sabe y hay que
/// comunicarlo (`ROPERO_PORT=<n>`), aunque el proceso siga vivo.
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
    let binario = crate::sys_engine::binary_of(&plan.engine);
    if binario.is_none() {
        return RunOutcome::error(format!(
            "el motor '{}' no está instalado: instálalo con 'ropero install engines'",
            plan.engine.id.as_str()
        ));
    }
    let binario = binario.expect("el binario existe");

    // El modelo tiene que estar descargado.
    if !crate::catalog_scan::model_downloaded(&plan.spec, env) {
        return RunOutcome::error(format!(
            "el modelo '{}' no está descargado: instálalo con 'ropero install models'",
            peticion.model
        ));
    }

    // El preflight: los recursos.
    let preflight = preflight_de(
        plan,
        env,
        crate::catalog_scan::engine_installed(&plan.engine),
        crate::catalog_scan::model_downloaded(&plan.spec, env),
        peticion.cpu,
    );
    if let Err(e) = preflight.check() {
        return RunOutcome::error(e.to_string());
    }

    // La sonda: con clave de API si el usuario la pidió.
    let clave = peticion
        .api_key
        .clone()
        .filter(|_| peticion.quiere_api_key());
    let probe = crate::probe::NetProbe {
        timeout: Duration::from_secs(3),
        api_key: clave,
    };

    // 2. Los puertos ocupados del rango del motor.
    //
    //    "Libre" quiere decir que nadie escucha en él: si no, el motor
    //    muere con «address already in use» y el arranque falla con un
    //    error que no explica nada.
    let ports = if peticion.port.is_some() {
        Vec::new()
    } else {
        puertos_ocupados(&plan.engine.port_range)
    };

    let spec = LaunchSpec {
        model: plan.spec.model.name.clone(),
        engine: plan.engine.id,
        engine_name: plan.engine.id.as_str().to_string(),
        program: binario,
        argv: plan.argv.clone(),
        level: plan.level.clone(),
        context: plan.context,
        log: log_de(&plan.spec, env),
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
        options: opciones_de(peticion, foreground),
    };

    let resultado = match crate::launch::launch(&spec, &ctx) {
        Ok(r) => r,
        Err(e) => return RunOutcome::error(e.to_string()),
    };

    // El motor está listo: se comunica el puerto. Es lo que leen los
    // scripts (`ROPERO_PORT=<n>`), y tiene que salir **antes** de que
    // ropero se quede esperando al proceso.
    al_arrancar(resultado.port, resultado.pid);

    // En primer plano: se espera al proceso, y el Ctrl-C lo interrumpe.
    if foreground {
        let interruptor = crate::foreground::Interruptor::new();
        if let Err(e) = interruptor.install() {
            // Sin manejador de SIGINT el Ctrl-C mataría a ropero y dejaría
            // al motor sirviendo: es mejor avisar y no esperar.
            return RunOutcome::error(format!(
                "no se pudo preparar el Ctrl-C: {e}\n  \
                 (el motor queda parado; páralo con 'ropero stop')"
            ));
        }
        match crate::foreground::esperar(&engine, resultado.pid, &interruptor, None) {
            Ok(Some(codigo)) => {
                // El motor terminó solo: se limpia el estado y se devuelve
                // su código de salida.
                crate::state::clear_state(state_dir, resultado.port);
                if codigo == 0 {
                    return RunOutcome::ok(resultado.port, resultado.pid);
                }
                return RunOutcome::error(format!(
                    "el motor de '{}' terminó con el código {codigo}",
                    peticion.model
                ));
            }
            Ok(None) => {
                crate::state::clear_state(state_dir, resultado.port);
                return RunOutcome::ok(resultado.port, resultado.pid);
            }
            Err(_) => {
                // El Ctrl-C: se para el motor y se sale con 130.
                use crate::launch::Engine as _;
                engine.terminate(&resultado.identity);
                crate::state::clear_state(state_dir, resultado.port);
                return RunOutcome::interrumpido();
            }
        }
    }

    RunOutcome::ok(resultado.port, resultado.pid)
}
