//! La parada: `ropero stop` (doc CLI v2 §10, doc B §3.3.3).
//!
//! ## El orden de las comprobaciones (R-EST-8)
//!
//! ```text
//! 1. ¿Hay algo que parar?           → si no hay estado, no se toca nada.
//! 2. ¿El proceso sigue vivo?        → si no, se limpia el estado.
//! 3. ¿El start_time coincide?       → si no, es otro proceso. NO se toca.
//! 4. ¿El exe_digest coincide?       → si no, es otro programa. NO se toca.
//! 5. Ahora sí: SIGTERM al grupo de procesos.
//! 6. Esperar hasta el deadline.
//! 7. SIGKILL al grupo.
//! ```text
//!
//! Los pasos 3 y 4 son la diferencia entre «ropero para lo que arrancó» y
//! «ropero mata lo que encuentra».
//!
//! ## Parar por PID, no por nombre
//!
//! `ropero stop <modelo>` mata el proceso del **estado** de ese modelo. Si
//! hay más modelos en el mismo motor, se respetan: se mata por PID, no
//! por nombre de proceso. Un `pkill -f llama-server` mataría los slots
//! paralelos.

use std::time::Duration;

use crate::process::ProcIdentity;
use crate::state::{clear_state, managed_ports, read_state, state_path};

/// Lo que se puede parar.
///
/// Es un rasgo para que las pruebas usen un doble: en producción es el
/// sistema operativo, y matar de verdad en un test es una mala idea.
pub trait Stopper: Send + Sync {
    /// Si el proceso sigue vivo.
    fn alive(&self, pid: u32) -> bool;

    /// La identidad **viva** del proceso: su `start_time` de ahora.
    ///
    /// Es lo que se compara con la del estado: si no coinciden, el PID
    /// fue reciclado y el proceso no es nuestro (P-3). `None` si no se
    /// pudo leer.
    fn identidad(&self, pid: u32) -> Option<ProcIdentity>;

    /// Manda SIGTERM al proceso (o a su grupo).
    fn terminate(&self, identity: &ProcIdentity);

    /// Manda SIGKILL al proceso (o a su grupo).
    fn kill(&self, identity: &ProcIdentity);

    /// Espera a que el proceso termine, hasta el deadline.
    ///
    /// `true` si terminó.
    fn wait_exit(&self, pid: u32, deadline: Duration) -> bool;
}

/// Lo que pasó al parar un puerto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// El proceso se paró.
    Parado,

    /// El PID del estado no es nuestro: no se tocó nada.
    Ajeno,

    /// El grupo siguió activo incluso después de SIGKILL.
    NoParado,

    /// No había estado para ese puerto.
    SinEstado,
}

/// Lo que pasó al parar un conjunto de puertos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopReport {
    /// Los puertos que se pararon.
    pub parados: Vec<u16>,

    /// Los puertos cuyo PID no era nuestro (PID reciclado).
    pub ajenos: Vec<u16>,

    /// Los puertos que no tenían estado.
    pub sin_estado: Vec<u16>,

    /// Los puertos cuyo grupo de procesos siguió vivo tras SIGKILL.
    pub no_parados: Vec<u16>,
}

impl StopReport {
    /// Un informe nuevo, vacío.
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Si no se paró nada.
    pub fn vacio(&self) -> bool {
        self.parados.is_empty()
            && self.ajenos.is_empty()
            && self.sin_estado.is_empty()
            && self.no_parados.is_empty()
    }

    /// El texto para el usuario, en español.
    ///
    /// Dice **qué** se paró y **qué hacer** con lo que no se pudo parar
    /// (P-13).
    pub fn texto(&self) -> String {
        let mut out = String::new();
        if !self.parados.is_empty() {
            let puertos: Vec<String> = self.parados.iter().map(|p| p.to_string()).collect();
            out.push_str(&format!("parado: {}\n", puertos.join(", ")));
        }
        if !self.ajenos.is_empty() {
            let puertos: Vec<String> = self.ajenos.iter().map(|p| p.to_string()).collect();
            out.push_str(&format!(
                "el pid de {} no es el motor que lanzó ropero (pid reciclado): \
                 no se ha matado nada; si quieres pararlo, páralo tú\n",
                puertos.join(", ")
            ));
        }
        if !self.sin_estado.is_empty() {
            let puertos: Vec<String> = self.sin_estado.iter().map(|p| p.to_string()).collect();
            out.push_str(&format!(
                "no hay ningún motor de ropero en {}\n",
                puertos.join(", ")
            ));
        }
        if !self.no_parados.is_empty() {
            let puertos: Vec<String> = self.no_parados.iter().map(|p| p.to_string()).collect();
            out.push_str(&format!(
                "el grupo de procesos de {} sigue activo tras SIGKILL; se conserva el estado para poder reintentar\n",
                puertos.join(", ")
            ));
        }
        if self.vacio() {
            out.push_str("no hay ningún motor de ropero que parar\n");
        }
        out
    }
}

/// La parada: sabe cómo parar por puerto y qué parar en conjunto.
pub struct SysStopper;

impl SysStopper {
    /// Para lo que hay en un puerto, **verificando la identidad**.
    ///
    /// Es el núcleo de `ropero stop`: el PID del estado sólo se mata si el
    /// proceso vivo es **el nuestro** (mismo `start_time`, y el mismo
    /// digest del binario cuando los dos se saben).
    pub fn parar_puerto(
        state_dir: &std::path::Path,
        port: u16,
        stopper: &dyn Stopper,
        deadline: Duration,
    ) -> StopOutcome {
        // 1. Sin estado, no hay nada que parar.
        let Some(estado) = read_state(state_dir, port) else {
            return StopOutcome::SinEstado;
        };
        let Some(pid) = estado.pid else {
            clear_state(state_dir, port);
            return StopOutcome::SinEstado;
        };

        // 2. ¿El proceso sigue vivo?
        if !stopper.alive(pid) {
            // El estado está caducado: se limpia y no se manda ninguna
            // señal (no hay a quién mandársela).
            clear_state(state_dir, port);
            return StopOutcome::Parado;
        }

        // 3 y 4. La identidad: si el `start_time` no coincide, el PID fue
        // reciclado y el proceso vivo **no es nuestro**.
        let esperada = ProcIdentity {
            pid,
            start_time: estado.start_time.unwrap_or(0),
            exe_digest: estado.exe_digest.as_deref().and_then(parse_digest),
        };
        // Sin `start_time` en el estado no se puede probar nada: el estado
        // lo escribió otra versión y no se toca.
        if estado.start_time.is_none() {
            return StopOutcome::Ajeno;
        }
        let Some(actual) = stopper.identidad(pid) else {
            // No se pudo leer la identidad del proceso vivo: sin prueba,
            // no se toca.
            return StopOutcome::Ajeno;
        };
        if !actual.es_la_misma(&esperada) {
            return StopOutcome::Ajeno;
        }

        // 5. SIGTERM al grupo de procesos.
        stopper.terminate(&esperada);

        // 6. Se espera hasta el deadline.
        if !stopper.wait_exit(pid, deadline) && stopper.alive(pid) {
            // 7. Red de seguridad: lo que ignore el TERM se lleva por
            // delante el KILL.
            stopper.kill(&esperada);
            if !stopper.wait_exit(pid, Duration::from_secs(2)) && stopper.alive(pid) {
                return StopOutcome::NoParado;
            }
        }

        if stopper.alive(pid) {
            return StopOutcome::NoParado;
        }

        clear_state(state_dir, port);
        StopOutcome::Parado
    }

    /// Para **todos** los motores que ropero lanzó.
    ///
    /// Los puertos salen del estado en disco: son los que ropero gestiona.
    pub fn parar_todo(
        state_dir: &std::path::Path,
        stopper: &dyn Stopper,
        deadline: Duration,
    ) -> StopReport {
        let mut informe = StopReport::nuevo();
        for port in managed_ports(state_dir) {
            match Self::parar_puerto(state_dir, port, stopper, deadline) {
                StopOutcome::Parado => informe.parados.push(port),
                StopOutcome::Ajeno => informe.ajenos.push(port),
                StopOutcome::SinEstado => informe.sin_estado.push(port),
                StopOutcome::NoParado => informe.no_parados.push(port),
            }
        }
        informe
    }

    /// Para el motor de **un modelo**, y sólo el suyo.
    ///
    /// Se mata por el PID del estado de ese modelo: los demás modelos que
    /// compartan motor se respetan.
    pub fn parar_modelo(
        state_dir: &std::path::Path,
        modelo: &str,
        stopper: &dyn Stopper,
        deadline: Duration,
    ) -> StopReport {
        let mut informe = StopReport::nuevo();
        for port in managed_ports(state_dir) {
            let Some(estado) = read_state(state_dir, port) else {
                continue;
            };
            if estado.model != modelo {
                // Otro modelo: se respeta.
                continue;
            }
            match Self::parar_puerto(state_dir, port, stopper, deadline) {
                StopOutcome::Parado => informe.parados.push(port),
                StopOutcome::Ajeno => informe.ajenos.push(port),
                StopOutcome::SinEstado => informe.sin_estado.push(port),
                StopOutcome::NoParado => informe.no_parados.push(port),
            }
        }
        informe
    }

    /// Los puertos con estado de un modelo, sin parar nada.
    pub fn puertos_de_modelo(state_dir: &std::path::Path, modelo: &str) -> Vec<u16> {
        managed_ports(state_dir)
            .into_iter()
            .filter(|port| {
                read_state(state_dir, *port).is_some_and(|estado| estado.model == modelo)
            })
            .collect()
    }

    /// La ruta del estado de un puerto (para los mensajes).
    pub fn ruta_estado(state_dir: &std::path::Path, port: u16) -> std::path::PathBuf {
        state_path(state_dir, port)
    }
}

/// Un digest hexadecimal de vuelta a bytes.
fn parse_digest(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}
