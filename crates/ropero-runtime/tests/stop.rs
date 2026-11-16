//! Tests de la parada: `ropero stop` (doc CLI v2 §10, doc B §3.3.3).
//!
//! ## Lo que NO se puede hacer
//!
//! Matar **procesos ajenos**. Un `pkill -f llama-server` mataría los
//! llama-server de toda la máquina, incluidos los de otro proyecto. Y un
//! `kill <pid>` a secas mataría a un inocente cuando el PID del estado
//! fue **reciclado** por el SO (P-3, R-EST-8).
//!
//! Por eso la parada:
//!
//! 1. lee el estado en disco (el PID y su `start_time`),
//! 2. comprueba que el proceso vivo tiene **ese** `start_time`,
//! 3. y sólo entonces lo para.
//!
//! ## Lo que sí se puede hacer
//!
//! Parar **por PID**, no por nombre: `ropero stop <modelo>` mata el
//! proceso del estado de ese modelo y respeta los demás modelos que
//! compartan motor.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use ropero_runtime::process::ProcIdentity;
use ropero_runtime::state::{PortState, write_state};
use ropero_runtime::stop::{StopOutcome, StopReport, Stopper, SysStopper};

/// Un directorio temporal que se borra al soltarlo.
struct TempDir(PathBuf);

impl TempDir {
    fn nuevo() -> Self {
        let base = std::env::temp_dir().join(format!(
            "ropero-stop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).expect("crear el temporal");
        Self(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// El stopper doble: no mata nada, sólo registra lo que se le pide.
///
/// La identidad que devuelve es la del **estado**: el proceso vivo es
/// «el nuestro».
struct FakeStopper {
    /// Los PIDs que se han terminado.
    terminados: Mutex<Vec<u32>>,
    /// Los PIDs que se han matado a la fuerza.
    matados: Mutex<Vec<u32>>,
    /// La identidad que devuelve para cualquier PID: el `start_time` es lo
    /// que importa, el PID es el que se le pida.
    start_time: Option<u64>,

    /// Si el proceso sigue vivo tras el TERM.
    vivo_despues: bool,

    /// Si el proceso estaba vivo **antes** de la parada.
    vivo: bool,

    /// Si SIGKILL logra terminar el grupo en este doble.
    kill_efectivo: bool,
}

impl FakeStopper {
    /// Un stopper cuyo proceso vivo es el nuestro (`start_time` 100).
    fn nuestro() -> Self {
        Self {
            terminados: Mutex::new(Vec::new()),
            matados: Mutex::new(Vec::new()),
            start_time: Some(100),
            vivo_despues: false,
            vivo: true,
            kill_efectivo: true,
        }
    }

    /// Un stopper cuyo proceso vivo es **otro** (`start_time` 999): es el
    /// caso del PID reciclado.
    fn ajeno() -> Self {
        Self {
            terminados: Mutex::new(Vec::new()),
            matados: Mutex::new(Vec::new()),
            start_time: Some(999),
            vivo_despues: false,
            vivo: true,
            kill_efectivo: true,
        }
    }

    /// Un stopper cuyo proceso no se puede identificar.
    fn sin_identidad() -> Self {
        Self {
            terminados: Mutex::new(Vec::new()),
            matados: Mutex::new(Vec::new()),
            start_time: None,
            vivo_despues: false,
            vivo: true,
            kill_efectivo: true,
        }
    }

    /// Un stopper donde el proceso sigue vivo tras el TERM.
    fn que_sigue_vivo() -> Self {
        Self {
            terminados: Mutex::new(Vec::new()),
            matados: Mutex::new(Vec::new()),
            start_time: Some(100),
            vivo_despues: true,
            vivo: true,
            kill_efectivo: true,
        }
    }

    fn que_no_muere() -> Self {
        Self {
            kill_efectivo: false,
            ..Self::que_sigue_vivo()
        }
    }
}

impl Stopper for FakeStopper {
    fn alive(&self, pid: u32) -> bool {
        if self.kill_efectivo && self.matados.lock().expect("candado").contains(&pid) {
            return false;
        }
        let terminados = self.terminados.lock().expect("candado");
        if terminados.contains(&pid) {
            // Recibe el TERM y muere: salvo que siga vivo tras el TERM.
            return self.vivo_despues;
        }
        self.vivo
    }

    fn identidad(&self, pid: u32) -> Option<ProcIdentity> {
        self.start_time
            .map(|start_time| ProcIdentity::with_digest(pid, start_time, None))
    }

    fn terminate(&self, identity: &ProcIdentity) {
        self.terminados.lock().expect("candado").push(identity.pid);
    }

    fn kill(&self, identity: &ProcIdentity) {
        self.matados.lock().expect("candado").push(identity.pid);
    }

    fn wait_exit(&self, _pid: u32, _deadline: Duration) -> bool {
        false
    }
}

/// Un estado de prueba para el puerto dado.
fn estado(modelo: &str, motor: &str, pid: u32, start_time: u64, port: u16) -> PortState {
    PortState {
        model: modelo.to_string(),
        engine: motor.to_string(),
        pid: Some(pid),
        port: Some(port),
        level: "think".to_string(),
        ts: Some(1_762_362_312),
        log: "/tmp/x.log".to_string(),
        stamp: "abc".to_string(),
        start_time: Some(start_time),
        exe_digest: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parar un puerto
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn se_mata_el_pid_del_estado() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();
    let stopper = FakeStopper::nuestro();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::Parado);
    assert_eq!(*stopper.terminados.lock().unwrap(), vec![4242]);
}

#[test]
fn un_pid_reciclado_no_se_mata() {
    let dir = TempDir::nuevo();
    // El estado dice que el proceso empezó en el instante 100.
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();

    // Pero el proceso vivo **no** es el nuestro: su `start_time` es otro.
    // Es el caso del PID reciclado: matarlo sería matar a un inocente.
    let stopper = FakeStopper::ajeno();
    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::Ajeno, "no se puede matar un ajeno");
    assert!(
        stopper.terminados.lock().unwrap().is_empty(),
        "no se manda ninguna señal a un proceso ajeno"
    );
    // Y el estado **no** se borra: el estado sigue siendo cierto (el
    // proceso es otro), y borrarlo haría que ropero no supiera que hay
    // algo en ese puerto.
    assert!(
        dir.path().join("ports").join("9999").exists(),
        "el estado de un ajeno se conserva"
    );
}

#[test]
fn un_proceso_que_no_se_puede_identificar_no_se_toca() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();
    let stopper = FakeStopper::sin_identidad();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::Ajeno);
    assert!(stopper.terminados.lock().unwrap().is_empty());
}

#[test]
fn un_estado_sin_start_time_no_se_toca() {
    // Un estado escrito por la v1 no tiene `start_time`: sin esa prueba no
    // se puede saber si el PID es el nuestro, así que no se toca.
    let dir = TempDir::nuevo();
    let mut e = estado("groot", "llama-server", 4242, 100, 9999);
    e.start_time = None;
    write_state(dir.path(), 9999, &e).unwrap();
    let stopper = FakeStopper::nuestro();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::Ajeno);
    assert!(stopper.terminados.lock().unwrap().is_empty());
}

#[test]
fn sin_estado_no_se_mata_nada() {
    let dir = TempDir::nuevo();
    let stopper = FakeStopper::nuestro();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::SinEstado);
    assert!(stopper.terminados.lock().unwrap().is_empty());
}

#[test]
fn un_proceso_que_no_esta_vivo_se_limpia_el_estado() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();
    // El stopper dice que el PID no está vivo: el estado está caducado.
    let stopper = Muerto;

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_secs(1));

    assert_eq!(resultado, StopOutcome::Parado);
    // Y el estado se borra: ya no describe nada.
    assert!(
        !dir.path().join("ports").join("9999").exists(),
        "el estado de un proceso muerto tiene que desaparecer"
    );
}

/// Un stopper donde ningún proceso está vivo.
struct Muerto;

impl Stopper for Muerto {
    fn alive(&self, _pid: u32) -> bool {
        false
    }

    fn identidad(&self, _pid: u32) -> Option<ProcIdentity> {
        None
    }

    fn terminate(&self, _identity: &ProcIdentity) {}

    fn kill(&self, _identity: &ProcIdentity) {}

    fn wait_exit(&self, _pid: u32, _deadline: Duration) -> bool {
        true
    }
}

#[test]
fn un_proceso_que_ignora_el_term_muere_con_el_kill() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();
    // El proceso sigue vivo tras el TERM: hay que mandarle el KILL.
    let stopper = FakeStopper::que_sigue_vivo();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_millis(50));

    assert_eq!(resultado, StopOutcome::Parado);
    assert_eq!(*stopper.terminados.lock().unwrap(), vec![4242]);
    assert_eq!(
        *stopper.matados.lock().unwrap(),
        vec![4242],
        "lo que ignora el TERM se lleva por delante el KILL"
    );
}

#[test]
fn si_el_grupo_sigue_vivo_no_afirma_que_se_paro_y_conserva_estado() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        9999,
        &estado("groot", "llama-server", 4242, 100, 9999),
    )
    .unwrap();
    let stopper = FakeStopper::que_no_muere();

    let resultado = SysStopper::parar_puerto(dir.path(), 9999, &stopper, Duration::from_millis(1));

    assert_eq!(resultado, StopOutcome::NoParado);
    assert!(dir.path().join("ports").join("9999").exists());
    assert!(stopper.matados.lock().unwrap().contains(&4242));
}

// ─────────────────────────────────────────────────────────────────────────────
// Parar en conjunto
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parar_todo_mata_todos_los_que_hay() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        8100,
        &estado("groot", "llama-server", 111, 100, 8100),
    )
    .unwrap();
    write_state(
        dir.path(),
        8200,
        &estado("nemotron", "turbo-llama", 222, 100, 8200),
    )
    .unwrap();
    let stopper = FakeStopper::nuestro();

    let informe = SysStopper::parar_todo(dir.path(), &stopper, Duration::from_secs(1));

    assert_eq!(informe.parados.len(), 2, "informe={informe:?}");
    assert!(informe.parados.contains(&8100));
    assert!(informe.parados.contains(&8200));
    // Los dos PIDs se han terminado.
    let terminados = stopper.terminados.lock().unwrap().clone();
    assert!(terminados.contains(&111), "terminados={terminados:?}");
    assert!(terminados.contains(&222), "terminados={terminados:?}");
}

#[test]
fn parar_un_modelo_respeta_los_demas_del_mismo_motor() {
    let dir = TempDir::nuevo();
    // Dos modelos en el **mismo** motor, en puertos distintos.
    write_state(
        dir.path(),
        8100,
        &estado("groot", "llama-server", 111, 100, 8100),
    )
    .unwrap();
    write_state(
        dir.path(),
        8101,
        &estado("nemotron", "llama-server", 222, 100, 8101),
    )
    .unwrap();
    let stopper = FakeStopper::nuestro();

    let informe = SysStopper::parar_modelo(dir.path(), "groot", &stopper, Duration::from_secs(1));

    // Sólo se para el de `groot`.
    assert_eq!(informe.parados, vec![8100], "informe={informe:?}");
    let terminados = stopper.terminados.lock().unwrap().clone();
    assert_eq!(terminados, vec![111], "terminados={terminados:?}");
    // Y el estado del otro modelo sigue ahí.
    assert!(
        dir.path().join("ports").join("8101").exists(),
        "el estado del otro modelo tiene que seguir"
    );
}

#[test]
fn parar_un_modelo_que_no_esta_no_hace_nada() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        8100,
        &estado("groot", "llama-server", 111, 100, 8100),
    )
    .unwrap();
    let stopper = FakeStopper::nuestro();

    let informe =
        SysStopper::parar_modelo(dir.path(), "no-existe", &stopper, Duration::from_secs(1));

    assert!(informe.vacio(), "informe={informe:?}");
    assert!(stopper.terminados.lock().unwrap().is_empty());
    assert!(
        dir.path().join("ports").join("8100").exists(),
        "el estado del otro modelo tiene que seguir"
    );
}

#[test]
fn un_modelo_servido_en_varios_puertos_se_paran_todos() {
    let dir = TempDir::nuevo();
    write_state(
        dir.path(),
        8100,
        &estado("groot", "llama-server", 111, 100, 8100),
    )
    .unwrap();
    write_state(
        dir.path(),
        8101,
        &estado("groot", "llama-server", 222, 100, 8101),
    )
    .unwrap();
    let stopper = FakeStopper::nuestro();

    let informe = SysStopper::parar_modelo(dir.path(), "groot", &stopper, Duration::from_secs(1));

    assert_eq!(informe.parados.len(), 2, "informe={informe:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// El informe
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn un_informe_vacio_dice_que_no_hay_nada_que_parar() {
    let informe = StopReport::nuevo();
    assert!(informe.vacio());
    let texto = informe.texto();
    assert!(texto.contains("no hay ningún motor"), "texto={texto}");
}

#[test]
fn el_informe_dice_que_se_ha_parado() {
    let informe = StopReport {
        parados: vec![8100, 8200],
        ajenos: vec![],
        sin_estado: vec![],
        no_parados: vec![],
    };
    let texto = informe.texto();
    assert!(texto.contains("8100"), "texto={texto}");
    assert!(texto.contains("8200"), "texto={texto}");
}

#[test]
fn el_informe_dice_que_no_se_ha_matado_un_ajeno() {
    let informe = StopReport {
        parados: vec![],
        ajenos: vec![8100],
        sin_estado: vec![],
        no_parados: vec![],
    };
    let texto = informe.texto();
    assert!(texto.contains("reciclado"), "texto={texto}");
    // Y dice qué hacer.
    assert!(texto.contains("páralo tú"), "texto={texto}");
}

#[test]
fn el_informe_explica_que_el_grupo_sigue_activo() {
    let informe = StopReport {
        no_parados: vec![9999],
        ..StopReport::default()
    };
    assert!(!informe.vacio());
    assert!(informe.texto().contains("sigue activo"));
}
