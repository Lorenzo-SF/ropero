//! El arranque de un modelo: preflight, sonda, sello y estado (doc B §3.3).
//!
//! ## La forma del arranque
//!
//! ```text
//! 1. Preflight: engine instalado, modelo instalado, recursos.
//! 2. Puerto: el del modelo, el pedido con --port, o el primero libre del
//!    rango del motor.
//! 3. --force: si hay procesos de ropero en ese device, pararlos (sólo los
//!    nuestros: PID + start_time).
//! 4. Spawn del motor con el argv montado.
//! 5. Sonda de readiness, con backoff y deadline.
//! 6. Estado en disco (tmp + mv, atómico).
//! 7. En primer plano: esperar al proceso. Ctrl-C propaga el SIGINT y
//!    sale con 130.
//! ```text
//!
//! ## Por qué hay una capa de dobles
//!
//! El arranque real lanza procesos de verdad y espera a que respondan: no
//! se puede probar en un test sin encender un servidor. Por eso el
//! arranque habla con dos rasgos —el **motor** y la **sonda**— que en las
//! pruebas son dobles y en producción son los de verdad. El resto del
//! código no sabe cuál de los dos tiene delante.

use std::path::PathBuf;
use std::time::Duration;

use ropero_core::{EngineId, Preflight, PreflightError};

use crate::process::ProcIdentity;
use crate::state::PortState;

/// Lo que el motor necesita para arrancar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    /// El modelo que se sirve.
    pub model: String,

    /// El motor que lo sirve.
    pub engine: EngineId,

    /// El nombre del motor, tal y como se escribe en el estado.
    pub engine_name: String,

    /// El binario del motor (o el comando con sus argumentos).
    pub program: PathBuf,

    /// El argv del modelo, ya montado por `build_argv`.
    pub argv: Vec<String>,

    /// El nivel de razonamiento con el que arranca.
    pub level: String,

    /// El contexto que se sirve.
    pub context: u32,

    /// La ruta del log del arranque.
    pub log: PathBuf,

    /// El rango de puertos del motor (doc CLI v2 §5.1).
    ///
    /// Es de donde sale el puerto cuando no se pide uno con `--port` y el
    /// modelo no lo tiene pineado.
    pub port_range: ropero_core::PortRange,
}

/// El resultado de arrancar un modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchResult {
    /// El puerto en el que sirve.
    pub port: u16,

    /// El proceso que lo sirve.
    pub pid: u32,

    /// La identidad del proceso (con su `start_time`).
    pub identity: ProcIdentity,

    /// El estado que se escribió en disco.
    pub state: PortState,
}

/// Los errores del arranque.
#[derive(Debug)]
pub enum LaunchError {
    /// El preflight no se cumple: el motor no está instalado, el modelo no
    /// está, o faltan recursos.
    Preflight(PreflightError),

    /// No hay ningún puerto libre en el rango del motor.
    NoPort(u16, u16),

    /// El motor no se pudo lanzar.
    Spawn(String),

    /// El motor no respondió a la sonda a tiempo.
    NotReady {
        /// El puerto donde se esperaba la respuesta.
        port: u16,
        /// El modelo que se esperaba.
        model: String,
        /// Cuánto se esperó.
        waited: Duration,
    },

    /// El motor murió durante el arranque.
    Died {
        /// El puerto donde se esperaba.
        port: u16,
        /// El modelo que se esperaba.
        model: String,
    },

    /// No se pudo escribir el estado en disco.
    State(String),

    /// El usuario interrumpió con Ctrl-C.
    Interrupted,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preflight(e) => write!(f, "{e}"),
            Self::NoPort(min, max) => write!(
                f,
                "no hay ningún puerto libre en el rango {min}-{max} del motor; \
                 libéralo con 'ropero stop' o usa '--port' con otro puerto"
            ),
            Self::Spawn(e) => write!(f, "no se pudo lanzar el motor: {e}"),
            Self::NotReady {
                port,
                model,
                waited,
            } => write!(
                f,
                "'{model}' no responde en el puerto {port} tras {}s; \
                 mira su log para ver qué pasó",
                waited.as_secs()
            ),
            Self::Died { port, model } => write!(
                f,
                "el proceso que sirve '{model}' murió durante el arranque en el puerto {port}"
            ),
            Self::State(e) => write!(f, "no se pudo escribir el estado en disco: {e}"),
            Self::Interrupted => write!(f, "interrumpido"),
        }
    }
}

impl std::error::Error for LaunchError {}

impl From<PreflightError> for LaunchError {
    fn from(e: PreflightError) -> Self {
        Self::Preflight(e)
    }
}

/// El motor: lo que se lanza y se para.
///
/// En producción es el sistema operativo; en las pruebas, un doble.
pub trait Engine: Send + Sync {
    /// Lanza el motor con el argv dado.
    ///
    /// Devuelve el proceso lanzado y su identidad.
    fn spawn(&self, spec: &LaunchSpec, port: u16) -> Result<Spawned, String>;

    /// Si el proceso sigue vivo.
    fn alive(&self, pid: u32) -> bool;

    /// Manda SIGTERM al proceso (o a su grupo).
    fn terminate(&self, identity: &ProcIdentity);

    /// Manda SIGKILL al proceso (o a su grupo).
    fn kill(&self, identity: &ProcIdentity);

    /// Espera a que el proceso termine, hasta el deadline.
    ///
    /// Devuelve el código de salida, si lo hubo.
    fn wait(&self, pid: u32, deadline: Duration) -> Option<i32>;
}

/// Un proceso lanzado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawned {
    /// El PID del proceso.
    pub pid: u32,
    /// La identidad completa (con `start_time`).
    pub identity: ProcIdentity,
}

/// La sonda de readiness.
///
/// La jerarquía de niveles (doc B §3.3.6): se prueban en orden y el
/// primero que pasa es suficiente.
pub trait Probe: Send + Sync {
    /// Si el motor ya responde en el puerto.
    fn ready(&self, port: u16, model: &str) -> bool;
}

/// El plan de la sonda: los intentos y el backoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbePlan {
    /// El tiempo que se espera entre un intento y el siguiente.
    pub backoff: Duration,

    /// El tiempo total que se espera antes de rendirse.
    pub deadline: Duration,
}

impl Default for ProbePlan {
    fn default() -> Self {
        Self {
            backoff: Duration::from_millis(200),
            deadline: Duration::from_secs(120),
        }
    }
}

/// Cómo se lanza: el plan de la sonda y si se espera en primer plano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOptions {
    /// El plan de la sonda.
    pub probe: ProbePlan,

    /// Si se espera al proceso en primer plano.
    pub foreground: bool,

    /// Si hay que parar lo que haya de ropero en el device.
    pub force: bool,

    /// Si el arranque es sólo por CPU.
    pub cpu: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            probe: ProbePlan::default(),
            foreground: true,
            force: false,
            cpu: false,
        }
    }
}

/// Lo que hay que saber del **dispositivo** antes de arrancar.
///
/// Sin esto, el `--force` no puede distinguir «un proceso de ropero» de
/// «un proceso cualquiera»: mata lo que encuentra.
pub trait Device: Send + Sync {
    /// Los procesos de ropero que están vivos en el device.
    ///
    /// La identidad es la que permite **verificar** que un PID es nuestro
    /// antes de matarlo.
    fn ropero_processes(&self) -> Vec<ProcIdentity>;
}

/// El contexto del arranque: lo que hay alrededor del motor.
///
/// Se agrupa en una estructura porque el arranque tiene muchas piezas
/// (motor, sonda, device, preflight, puerto, estado, opciones) y una
/// lista de argumentos de ocho posiciones es ilegible: es fácil
/// intercambiarlas sin que el compilador avise.
pub struct LaunchContext<'a> {
    /// El motor que se lanza.
    pub engine: &'a dyn Engine,

    /// La sonda de readiness.
    pub probe: &'a dyn Probe,

    /// El device: los procesos de ropero que están vivos.
    pub device: &'a dyn Device,

    /// El preflight: lo que se comprueba antes de tocar nada.
    pub preflight: &'a Preflight,

    /// El puerto pedido (`--port`), si lo hay.
    pub port: Option<u16>,

    /// El directorio de estado.
    pub state_dir: &'a std::path::Path,

    /// Las opciones del arranque.
    pub options: LaunchOptions,
}

/// El arranque completo.
///
/// La firma es la del doc B §3.3 con las piezas que la hacen probable:
/// el motor, la sonda y el device entran por parámetro, así que un test
/// puede poner dobles donde quiera.
pub fn launch(spec: &LaunchSpec, ctx: &LaunchContext<'_>) -> Result<LaunchResult, LaunchError> {
    let engine = ctx.engine;
    let probe = ctx.probe;
    let device = ctx.device;
    let preflight = ctx.preflight;
    let port = ctx.port;
    let state_dir = ctx.state_dir;
    let options = ctx.options;

    // 1. Preflight: antes de tocar nada. Un lanzamiento condenado no
    //    mata lo que corre.
    preflight.check()?;

    // 2. Puerto.
    let puerto = match port {
        Some(p) => p,
        None => ropero_core::pick_port(spec.port_range, &[]).ok_or(LaunchError::NoPort(
            spec.port_range.min,
            spec.port_range.max,
        ))?,
    };

    // 3. --force: parar lo que haya de ropero en el device.
    if options.force {
        for identidad in device.ropero_processes() {
            engine.terminate(&identidad);
        }
    }

    // 4. Lanzar.
    let lanzado = engine.spawn(spec, puerto).map_err(LaunchError::Spawn)?;

    // 5. Estado en disco: se escribe **antes** de esperar, para que otro
    //    ropero vea el arranque en curso y para que un `stop` en mitad del
    //    arranque sepa a quién matar.
    let stamp = crate::state::stamp_de(&[
        &spec.model,
        &spec.level,
        &spec.context.to_string(),
        &spec.argv.join(" "),
    ]);
    let estado = PortState {
        model: spec.model.clone(),
        engine: spec.engine_name.clone(),
        pid: Some(lanzado.pid),
        port: Some(puerto),
        level: spec.level.clone(),
        ts: Some(now_epoch()),
        log: spec.log.to_string_lossy().to_string(),
        stamp,
        start_time: Some(lanzado.identity.start_time),
        exe_digest: lanzado
            .identity
            .exe_digest
            .map(|d| d.iter().map(|b| format!("{b:02x}")).collect()),
    };
    crate::state::write_state(state_dir, puerto, &estado).map_err(LaunchError::State)?;

    // 6. Sonda de readiness.
    if !espera_ready(
        engine,
        probe,
        puerto,
        &spec.model,
        lanzado.pid,
        options.probe,
    ) {
        // El proceso murió durante el arranque: el estado ya no describe
        // nada.
        if !engine.alive(lanzado.pid) {
            crate::state::clear_state(state_dir, puerto);
            return Err(LaunchError::Died {
                port: puerto,
                model: spec.model.clone(),
            });
        }
        // Sigue vivo pero no responde: se le para para no dejar un zombi.
        engine.terminate(&lanzado.identity);
        crate::state::clear_state(state_dir, puerto);
        return Err(LaunchError::NotReady {
            port: puerto,
            model: spec.model.clone(),
            waited: options.probe.deadline,
        });
    }

    Ok(LaunchResult {
        port: puerto,
        pid: lanzado.pid,
        identity: lanzado.identity,
        state: estado,
    })
}

/// La espera de readiness, con backoff y deadline.
///
/// La sonda se intenta hasta que pasa o hasta que se acaba el plazo. Si
/// el proceso muere, no se espera al deadline: el arranque ya está
/// condenado y esperar sólo hace perder el tiempo del usuario.
fn espera_ready(
    engine: &dyn Engine,
    probe: &dyn Probe,
    puerto: u16,
    modelo: &str,
    pid: u32,
    plan: ProbePlan,
) -> bool {
    let mut esperado = Duration::ZERO;
    loop {
        if probe.ready(puerto, modelo) {
            return true;
        }
        // El proceso ya no está: la sonda no va a pasar nunca.
        if !engine.alive(pid) {
            return false;
        }
        if esperado >= plan.deadline {
            return false;
        }
        std::thread::sleep(plan.backoff);
        esperado += plan.backoff;
    }
}

/// Los segundos desde epoch de ahora.
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
