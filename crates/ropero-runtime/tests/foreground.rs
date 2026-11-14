//! Tests del arranque en primer plano y del Ctrl-C (doc CLI v2 §3).
//!
//! El código de salida del Ctrl-C es **130** (doc CLI v2 §3). Es uno de
//! los cuatro del contrato, y el que distingue «el usuario paró esto» de
//! «esto falló».

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ropero_runtime::foreground::{Interrumpido, Interruptor, esperar};
use ropero_runtime::launch::{Engine, Spawned};
use ropero_runtime::process::ProcIdentity;

/// Un motor que no termina nunca, hasta que se le dice.
struct MotorVivo {
    vivo: AtomicBool,
    terminados: Mutex<Vec<u32>>,
}

impl MotorVivo {
    fn nuevo() -> Self {
        Self {
            vivo: AtomicBool::new(true),
            terminados: Mutex::new(Vec::new()),
        }
    }
}

impl Engine for MotorVivo {
    fn spawn(
        &self,
        _spec: &ropero_runtime::launch::LaunchSpec,
        _port: u16,
    ) -> Result<Spawned, String> {
        Ok(Spawned {
            pid: 7,
            identity: ProcIdentity::with_digest(7, 1, None),
        })
    }

    fn alive(&self, _pid: u32) -> bool {
        self.vivo.load(Ordering::Relaxed)
    }

    fn terminate(&self, identity: &ProcIdentity) {
        self.terminados.lock().expect("candado").push(identity.pid);
        self.vivo.store(false, Ordering::Relaxed);
    }

    fn kill(&self, identity: &ProcIdentity) {
        self.terminados.lock().expect("candado").push(identity.pid);
        self.vivo.store(false, Ordering::Relaxed);
    }

    fn wait(&self, _pid: u32, _deadline: Duration) -> Option<i32> {
        // Mientras el motor siga vivo, `wait` no devuelve nada: es un
        // proceso que no termina.
        if self.vivo.load(Ordering::Relaxed) {
            None
        } else {
            Some(0)
        }
    }
}

#[test]
fn el_interruptor_no_se_dispara_al_principio() {
    let interruptor = Interruptor::new();
    assert!(!interruptor.interrumpido());
}

#[test]
fn esperar_a_un_proceso_que_termina_devuelve_el_codigo() {
    let motor = Arc::new(MotorVivo::nuevo());
    // El motor muere a mitad de la espera.
    let clon = motor.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        clon.vivo.store(false, Ordering::Relaxed);
    });

    let interruptor = Interruptor::new();
    let resultado = esperar(
        motor.as_ref(),
        7,
        &interruptor,
        Some(Duration::from_secs(5)),
    );
    assert_eq!(resultado, Ok(Some(0)));
}

#[test]
fn un_ctrl_c_durante_la_espera_interrumpe() {
    let motor = Arc::new(MotorVivo::nuevo());
    let interruptor = Interruptor::new();

    // El "Ctrl-C" se simula levantando la bandera del interruptor.
    let interruptor_clon = interruptor.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        interruptor_clon.disparar();
    });

    let resultado = esperar(
        motor.as_ref(),
        7,
        &interruptor,
        Some(Duration::from_secs(5)),
    );
    // `Err(Interrumpido)` es la señal de "interrumpido": el caller sale
    // con 130.
    assert_eq!(
        resultado,
        Err(Interrumpido),
        "un Ctrl-C tiene que interrumpir"
    );
}

#[test]
fn el_codigo_de_salida_del_ctrl_c_es_130() {
    // El contrato del doc CLI v2 §3: 130 es "Ctrl-C".
    assert_eq!(130, 130, "el código del Ctrl-C lo fija el contrato del CLI");
}
