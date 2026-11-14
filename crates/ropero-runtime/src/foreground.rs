//! El arranque en primer plano: esperar al proceso y propagar el Ctrl-C.
//!
//! ## El Ctrl-C
//!
//! En primer plano, ropero **espera** al motor. El Ctrl-C (SIGINT) tiene
//! que:
//!
//! 1. pararlo (el motor y su grupo de procesos),
//! 2. limpiar el estado,
//! 3. salir con **130** (doc CLI v2 §3).
//!
//! ## Por qué hace falta una señal explícita
//!
//! El motor se lanza con su **propio grupo** de procesos (`setpgid`), así
//! que el SIGINT del terminal —que va al grupo de la terminal— **no**
//! llega al motor. Por eso ropero tiene que coger el SIGINT y mandárselo
//! él al grupo del motor. Sin esto, el Ctrl-C deja el motor sirviendo y
//! ropero muere: el puerto sigue ocupado y el estado en disco sigue
//! diciendo que ropero lo tiene vivo.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;

/// La bandera que se levanta cuando llega un Ctrl-C.
///
/// Es un `AtomicBool` en un `Arc`: el manejador de la señal sólo puede
/// tocar memoria segura entre hilos (se comparte estáticamente), y el
/// hilo principal la lee.
#[derive(Debug, Clone)]
pub struct Interruptor {
    flag: Arc<AtomicBool>,
}

impl Interruptor {
    /// Un interruptor nuevo, sin señal todavía.
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Instala el manejador de SIGINT y SIGTERM.
    ///
    /// Devuelve `Err` si no se puede instalar: en ese caso el arranque
    /// sigue, pero sin Ctrl-C propagado (y el motor queda en su grupo, así
    /// que el Ctrl-C del terminal no lo mata: es mejor eso que no
    /// arrancar).
    pub fn install(&self) -> Result<(), String> {
        flag::register(SIGINT, self.flag.clone()).map_err(|e| format!("SIGINT: {e}"))?;
        flag::register(SIGTERM, self.flag.clone()).map_err(|e| format!("SIGTERM: {e}"))?;
        Ok(())
    }

    /// Si se ha pedido parar.
    pub fn interrumpido(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Levanta la bandera a mano.
    ///
    /// Lo usa el manejador de la señal, y también las pruebas: el Ctrl-C
    /// de verdad no se puede mandar en un test sin tumbar al runner.
    pub fn disparar(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
}

impl Default for Interruptor {
    fn default() -> Self {
        Self::new()
    }
}

/// El error de la espera: el usuario interrumpió.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interrumpido;

impl std::fmt::Display for Interrumpido {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "interrumpido por el usuario")
    }
}

impl std::error::Error for Interrumpido {}

/// Espera al proceso, con el interruptor a la vista.
///
/// Devuelve `Ok(codigo)` si el proceso terminó solo, `Err(Interrumpido)`
/// si el usuario lo interrumpió.
///
/// `deadline` es el margen de la espera: `None` significa «esperar
/// indefinidamente», que es lo que hace el arranque en primer plano (el
/// motor sirve hasta que el usuario lo para).
///
/// La espera **no** es un `wait` bloqueante del sistema: se sondea con
/// un intervalo corto para poder mirar el interruptor. Es lo que hace que
/// el Ctrl-C se note al instante.
pub fn esperar(
    engine: &dyn crate::launch::Engine,
    pid: u32,
    interruptor: &Interruptor,
    deadline: Option<Duration>,
) -> Result<Option<i32>, Interrumpido> {
    let limite = deadline.map(|d| std::time::Instant::now() + d);
    loop {
        if interruptor.interrumpido() {
            return Err(Interrumpido);
        }
        if let Some(codigo) = engine.wait(pid, Duration::from_millis(50)) {
            return Ok(Some(codigo));
        }
        if let Some(limite) = limite
            && std::time::Instant::now() >= limite
        {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
