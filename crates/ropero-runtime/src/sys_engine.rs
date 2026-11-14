//! El motor real: el que lanza procesos de verdad.
//!
//! Es el **doble de producción**: en las pruebas, `launch.rs` se prueba
//! con dobles; aquí está el que habla con el sistema operativo.
//!
//! ## El grupo de procesos
//!
//! El motor se lanza en su **propio grupo de procesos** (`setpgid`), y las
//! señales se mandan al **grupo** (`killpg`), no al proceso suelto: un
//! `llama-server` tiene hijos, y si sólo se mata al padre los hijos
//! quedan sirviendo el puerto (doc B §3.3.3, paso 5: «SIGTERM al grupo de
//! procesos»).

use std::process::{Child, Command, Stdio};

use ropero_core::PortRange;

use crate::launch::{Engine, LaunchSpec, Spawned};
use crate::process::ProcIdentity;

/// El motor del sistema operativo.
pub struct SysEngine {
    /// El último proceso lanzado, para poder esperarlo.
    ultimo: std::sync::Mutex<Option<Child>>,
}

impl SysEngine {
    /// Un motor nuevo.
    pub fn new() -> Self {
        Self {
            ultimo: std::sync::Mutex::new(None),
        }
    }

    /// Recoge al motor que ya ha muerto, si es el nuestro.
    ///
    /// Un proceso que ha muerto sigue en la tabla de procesos hasta que
    /// su padre lo recoge: es un **zombi**. Recogerlo aquí evita que el
    /// reintento de puerto deje zombis detrás, y deja el código de
    /// salida en el sitio donde `wait` lo puede leer.
    ///
    /// `try_wait` no bloquea: si el motor sigue vivo, no hace nada.
    fn recoge(&self, pid: u32) -> bool {
        let Ok(mut guard) = self.ultimo.lock() else {
            return false;
        };
        let Some(motor) = guard.as_mut() else {
            return false;
        };
        if motor.id() != pid {
            return false;
        }
        matches!(motor.try_wait(), Ok(Some(_)))
    }
}

impl Default for SysEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for SysEngine {
    fn spawn(&self, spec: &LaunchSpec, port: u16) -> Result<Spawned, String> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.argv);
        command.arg("--port").arg(port.to_string());
        command.stdin(Stdio::null());
        // La salida del motor va al **log**: es lo que el usuario mira
        // cuando el arranque falla («mira su log para ver qué pasó»). Sin
        // esto, un motor que aborta se queda sin explicar por qué.
        if let Some(log) = abrir_log(&spec.log) {
            if let Ok(clon) = log.try_clone() {
                command.stdout(clon);
            }
            command.stderr(log);
        } else {
            command.stdout(Stdio::null());
            command.stderr(Stdio::null());
        }
        // Su propio grupo: las señales van al grupo entero.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("{}: {e}", spec.program.display()))?;
        let pid = child.id();
        // La identidad se lee **del proceso vivo**, no del binario que se
        // lanzó: así el `exe_digest` es el del binario que realmente
        // ejecuta (que para un script es su intérprete) y coincide con
        // el que se leerá al parar (P-3).
        let identity = ProcIdentity::of(pid).ok_or_else(|| {
            let _ = child.kill();
            format!("no se pudo leer la identidad del proceso {pid}")
        })?;
        if let Ok(mut guard) = self.ultimo.lock() {
            *guard = Some(child);
        }
        Ok(Spawned { pid, identity })
    }

    fn alive(&self, pid: u32) -> bool {
        // Si el motor ya ha muerto, se recoge: mientras su padre (ropero)
        // no lo recoja, sigue en la tabla de procesos como **zombi**, y
        // parece vivo.
        if self.recoge(pid) {
            return false;
        }
        vivo_de_verdad(pid)
    }

    fn terminate(&self, identity: &ProcIdentity) {
        // Al grupo: el motor tiene hijos.
        signal_group(identity.pid, Signal::TERM);
    }

    fn kill(&self, identity: &ProcIdentity) {
        signal_group(identity.pid, Signal::KILL);
    }

    fn wait(&self, _pid: u32, deadline: std::time::Duration) -> Option<i32> {
        let mut guard = self.ultimo.lock().ok()?;
        let child = guard.as_mut()?;
        let limite = std::time::Instant::now() + deadline;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) => {}
                Err(_) => return None,
            }
            if std::time::Instant::now() >= limite {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// La señal a mandar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Terminación suave: el proceso apaga a sus hijos y suelta puertos.
    TERM,
    /// Terminación forzada: la red de seguridad.
    KILL,
}

impl Signal {
    /// El número de la señal.
    fn numero(self) -> i32 {
        match self {
            Self::TERM => 15,
            Self::KILL => 9,
        }
    }
}

/// Manda la señal al **grupo** de procesos del PID.
///
/// Si el grupo no existe (el proceso ya murió, o no se Lanzó con su propio
/// grupo), se manda al proceso suelto: es lo mismo para un motor que no
/// tiene grupo propio.
fn signal_group(pid: u32, signal: Signal) {
    // `killpg` con el PID como PGID: el motor se lanzó con `setpgid(0,0)`,
    // así que su PGID es su PID.
    if libc_killpg(pid as i32, signal.numero()) != 0 {
        libc_kill(pid as i32, signal.numero());
    }
}

/// Si el proceso está vivo **de verdad**.
///
/// `kill(pid, 0)` pregunta si el proceso existe, y un **zombi** —un
/// proceso muerto que su padre todavía no ha recogido— existe: la señal
/// 0 la acepta. Por eso, antes de preguntar, se mira el estado que dice
/// el kernel: un zombi no está vivo.
///
/// El motor que muere al hacer bind («address already in use») es
/// exactamente eso durante el arranque: sin esta comprobación parecería
/// un motor sano, y el arranque no podría reintentar con otro puerto.
///
/// Lo usan los dos «¿sigue vivo?»: el del arranque (`SysEngine`) y el de
/// la parada (`OsStopper`), para que los dos digan lo mismo.
fn vivo_de_verdad(pid: u32) -> bool {
    if crate::process::estado_del_proceso(pid) == Some('Z') {
        return false;
    }
    libc_kill(pid as i32, 0) == 0
}

/// `kill(pid, 0)`: la señal 0 no hace nada, sólo pregunta si el proceso
/// existe y si le podemos mandar señales.
///
/// El `unsafe` está justificado: es una llamada al sistema POSIX
/// documentada (`kill(2)`), sin referencias ni memoria: sólo números.
fn libc_kill(pid: i32, sig: i32) -> i32 {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid, sig) }
}

/// `killpg(pgrp, sig)`: la señal al grupo de procesos.
///
/// El `unsafe` está justificado: es una llamada al sistema POSIX
/// documentada (`kill(2)`), sin referencias ni memoria.
fn libc_killpg(pgrp: i32, sig: i32) -> i32 {
    unsafe extern "C" {
        fn killpg(pgrp: i32, sig: i32) -> i32;
    }
    unsafe { killpg(pgrp, sig) }
}

/// Abre el log del motor para anexar la salida.
///
/// Si no se puede abrir (el directorio no existe, no hay permisos), se
/// devuelve `None` y el motor arranca sin log: es mejor un arranque sin
/// log que un arranque que no arranca.
fn abrir_log(ruta: &std::path::Path) -> Option<std::fs::File> {
    if let Some(dir) = ruta.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta)
        .ok()
}

/// El stopper real: el que manda las señales del sistema operativo.
///
/// Se llama `OsStopper` y no `SysStopper` porque `SysStopper` es el
/// **orquestador** de la parada (el de `stop.rs`), y este es su doble de
/// producción: los dos están en el mismo crate y el nombre repetido sólo
/// genera ambigüedad.
pub struct OsStopper;

impl crate::stop::Stopper for OsStopper {
    fn alive(&self, pid: u32) -> bool {
        vivo_de_verdad(pid)
    }

    fn identidad(&self, pid: u32) -> Option<crate::process::ProcIdentity> {
        crate::process::ProcIdentity::of(pid)
    }

    fn terminate(&self, identity: &crate::process::ProcIdentity) {
        // Al grupo: el motor tiene hijos, y si sólo se mata al padre los
        // hijos quedan sirviendo el puerto.
        signal_group(identity.pid, Signal::TERM);
    }

    fn kill(&self, identity: &crate::process::ProcIdentity) {
        signal_group(identity.pid, Signal::KILL);
    }

    fn wait_exit(&self, pid: u32, deadline: std::time::Duration) -> bool {
        let limite = std::time::Instant::now() + deadline;
        loop {
            if !self.alive(pid) {
                return true;
            }
            if std::time::Instant::now() >= limite {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// El rango de puertos de un motor, para la CLI.
pub fn port_range_of(engine: &ropero_core::EngineManifest) -> PortRange {
    engine.port_range
}

/// El binario del motor: el primero de `detect.binaries` que está en el
/// `PATH`.
///
/// Es la misma pregunta que `engine_installed` (que mira *algún* binario),
/// pero con el **concreto**: el motor que se lanza es un binario
/// concreto, no "uno de los que可能会".
pub fn binary_of(engine: &ropero_core::EngineManifest) -> Option<std::path::PathBuf> {
    engine
        .detect
        .binaries
        .iter()
        .find_map(|binario| crate::catalog_scan::which(binario))
        .or_else(|| {
            engine
                .detect
                .paths
                .iter()
                .map(std::path::PathBuf::from)
                .find(|ruta| ruta.is_file())
        })
}
