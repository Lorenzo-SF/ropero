//! Tests del motor real: `SysEngine` (doc B §3.3.4).
//!
//! ## El zombi
//!
//! Un proceso que ha muerto pero que su padre todavía no ha recogido
//! sigue en la tabla de procesos: la señal 0 la acepta y `kill(pid, 0)`
//! dice que está vivo. El motor que muere al hacer bind («address
//! already in use») es exactamente eso durante el arranque, y si
//! pareciera vivo el arranque no podría reintentar con otro puerto.
//!
//! ## El que se recoge
//!
//! Un zombi no es sólo un problema de cuentas: ocupa una entrada de la
//! tabla de procesos. Por eso el motor que muere se **recoge** en cuanto
//! se ve muerto, y el reintento de puerto no deja zombis detrás.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use ropero_core::{EngineId, PortRange};
use ropero_runtime::launch::{Engine, LaunchSpec};
use ropero_runtime::process::estado_del_proceso;
use ropero_runtime::{OsStopper, Stopper, SysEngine};

/// Un directorio temporal que se borra al soltarlo.
struct TempDir(PathBuf);

impl TempDir {
    fn nuevo() -> Self {
        let base = std::env::temp_dir().join(format!(
            "ropero-sys-engine-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).expect("crear el temporal");
        Self(base)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un proceso que termina y se queda sin recoger: un zombi.
fn zombi() -> (u32, std::process::Child) {
    let hijo = Command::new("/bin/sh")
        .arg("-c")
        .arg("exit 1")
        .spawn()
        .expect("arrancar el proceso de prueba");
    (hijo.id(), hijo)
}

#[test]
fn un_proceso_muerto_sin_recoger_no_esta_vivo() {
    let (pid, mut hijo) = zombi();

    // Se le da tiempo a que termine (y a que se quede sin recoger).
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(
        estado_del_proceso(pid),
        Some('Z'),
        "el proceso tiene que estar en zombi"
    );
    assert!(
        !SysEngine::new().alive(pid),
        "un zombi no está vivo: si lo pareciera, el arranque no podría reintentar"
    );

    // Se recoge, para no dejar el zombi detrás.
    let _ = hijo.wait();
}

#[test]
fn un_proceso_vivo_esta_vivo() {
    let mut hijo = Command::new("/bin/sh")
        .arg("-c")
        .arg("sleep 30")
        .spawn()
        .expect("arrancar el proceso de prueba");
    let pid = hijo.id();

    assert!(SysEngine::new().alive(pid), "un proceso vivo está vivo");
    assert_ne!(estado_del_proceso(pid), Some('Z'), "no puede ser un zombi");

    let _ = hijo.kill();
    let _ = hijo.wait();
}

#[test]
fn un_proceso_que_no_existe_no_tiene_estado() {
    // Un PID que no existe (o que ya se recogió): no hay estado que leer.
    let (pid, mut hijo) = zombi();
    let _ = hijo.wait();
    assert_eq!(estado_del_proceso(pid), None, "el proceso ya no existe");
}

/// La parada da por parado a un zombi.
///
/// Un zombi no está sirviendo: la parada no tiene que esperar a su
/// deadline para descubrirlo, y no manda ninguna señal a un proceso que
/// ya no existe.
#[test]
fn la_parada_da_por_parado_a_un_zombi() {
    let (pid, mut hijo) = zombi();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(estado_del_proceso(pid), Some('Z'), "el proceso es un zombi");

    assert!(
        !OsStopper.alive(pid),
        "un zombi no está vivo: la parada lo da por parado"
    );

    let _ = hijo.wait();
}

/// El motor que muere se recoge: no queda zombi detrás.
///
/// El reintento de puerto lanza un motor que muere al hacer bind. Si su
/// padre (ropero) no lo recoge, se queda en la tabla de procesos como
/// zombi: `kill(pid, 0)` lo daría por vivo y el arranque no podría
/// distinguir «carrera» de «motor sano».
#[test]
fn un_motor_muerto_se_recoge_y_no_deja_zombi() {
    let dir = TempDir::nuevo();
    let spec = LaunchSpec {
        model: "groot".to_string(),
        engine: EngineId::LlamaServer,
        engine_name: "llama-server".to_string(),
        server_api_key: None,
        program: PathBuf::from("/bin/sh"),
        argv: vec!["-c".to_string(), "exit 1".to_string()],
        level: "think".to_string(),
        context: 32768,
        log: dir.path().join("motor.log"),
        port_range: PortRange::new(8100, 8199),
    };
    let engine = SysEngine::new();
    let lanzado = engine
        .spawn(&spec, 8100)
        .expect("arrancar el motor de prueba");

    // El motor muere al instante; en cuanto se ve muerto, se recoge.
    for _ in 0..100 {
        if !engine.alive(lanzado.pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!engine.alive(lanzado.pid), "un motor muerto no está vivo");
    // Y no queda zombi: el proceso se recogió y ya no está en /proc.
    assert_eq!(
        estado_del_proceso(lanzado.pid),
        None,
        "el motor tiene que estar recogido, no ser un zombi"
    );
}
