//! Tests del arranque con dobles (doc B §3.3).
//!
//! El arranque real lanza procesos y espera a que respondan: no se puede
//! probar en un test sin encender un servidor. Por eso `launch.rs` habla
//! con dos rasgos —el **motor** y la **sonda**— y aquí los dobles
//! responden a lo que cada test necesita.
//!
//! Lo que se prueba es **la orquestación**: que el preflight impide el
//! spawn, que la sonda decide cuándo se está listo, que el estado se
//! escribe antes de esperar, y que un motor que no responde es un error
//! (no un arranque a medias).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use ropero_core::{EngineId, PortRange, Preflight};
use ropero_runtime::launch::{
    Engine, LaunchContext, LaunchError, LaunchOptions, LaunchSpec, Probe, ProbePlan,
};
use ropero_runtime::process::ProcIdentity;

/// Un directorio temporal que se borra al soltarlo.
struct TempDir(PathBuf);

impl TempDir {
    fn nuevo() -> Self {
        let base = std::env::temp_dir().join(format!(
            "ropero-test-{}-{}",
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

/// El motor doble: no lanza nada, sólo cuenta y responde lo que se le dice.
struct FakeEngine {
    /// Lo que pasó: los launches, los TERM, los KILL.
    acciones: Mutex<Vec<String>>,
    /// Si el motor está vivo.
    vivo: Mutex<bool>,
    /// Si el spawn falla.
    falla_spawn: bool,
    /// El próximo PID que se simula: cada arranque tiene el suyo.
    siguiente_pid: Mutex<u32>,
    /// El puerto en el que el motor muere al arrancar.
    ///
    /// Es la carrera del *bind*: otro proceso cogió el puerto y el motor
    /// no pudo hacer bind («address already in use»).
    ///
    /// Con `muere_siempre` da igual el puerto: el motor muere en todos.
    muere_en: Option<u16>,
    /// Si el motor muere al arrancar en **cualquier** puerto.
    ///
    /// Es el caso en el que el rango entero está cogido: el arranque tiene
    /// que rendirse cuando ya no queda ningún puerto que probar.
    muere_siempre: bool,
    /// Los PIDs de los motores que murieron al arrancar.
    muertos: Mutex<Vec<u32>>,
}

impl FakeEngine {
    fn nuevo() -> Self {
        Self {
            acciones: Mutex::new(Vec::new()),
            vivo: Mutex::new(true),
            falla_spawn: false,
            siguiente_pid: Mutex::new(4242),
            muere_en: None,
            muere_siempre: false,
            muertos: Mutex::new(Vec::new()),
        }
    }

    fn acciones(&self) -> MutexGuard<'_, Vec<String>> {
        self.acciones.lock().expect("el candado del motor")
    }

    fn muerto(self) -> Self {
        *self.vivo.lock().expect("el candado del motor") = false;
        self
    }

    fn que_falla_el_spawn() -> Self {
        Self {
            acciones: Mutex::new(Vec::new()),
            vivo: Mutex::new(true),
            falla_spawn: true,
            siguiente_pid: Mutex::new(4242),
            muere_en: None,
            muere_siempre: false,
            muertos: Mutex::new(Vec::new()),
        }
    }

    /// El motor muere al arrancar en ese puerto: no pudo hacer bind.
    fn que_muere_en(mut self, puerto: u16) -> Self {
        self.muere_en = Some(puerto);
        self
    }

    /// El motor muere al arrancar en **cualquier** puerto.
    fn que_muere_siempre(mut self) -> Self {
        self.muere_siempre = true;
        self
    }
}

impl Engine for FakeEngine {
    fn spawn(
        &self,
        spec: &LaunchSpec,
        port: u16,
    ) -> Result<ropero_runtime::launch::Spawned, String> {
        if self.falla_spawn {
            return Err("no such file or directory".to_string());
        }
        self.acciones().push(format!(
            "spawn {} {} --port {port}",
            spec.program.display(),
            spec.argv.join(" ")
        ));
        // Cada arranque es un motor distinto, con su PID.
        let pid = {
            let mut siguiente = self.siguiente_pid.lock().expect("el candado del motor");
            let pid = *siguiente;
            *siguiente += 1;
            pid
        };
        // Si el motor no pudo hacer bind, muere al instante.
        if self.muere_siempre || self.muere_en == Some(port) {
            self.muertos.lock().expect("el candado del motor").push(pid);
        }
        Ok(ropero_runtime::launch::Spawned {
            pid,
            identity: ProcIdentity::with_digest(pid, 1_000_000, None),
        })
    }

    fn alive(&self, pid: u32) -> bool {
        if self
            .muertos
            .lock()
            .expect("el candado del motor")
            .contains(&pid)
        {
            return false;
        }
        *self.vivo.lock().expect("el candado del motor")
    }

    fn terminate(&self, identity: &ProcIdentity) {
        self.acciones().push(format!("term {}", identity.pid));
    }

    fn kill(&self, identity: &ProcIdentity) {
        self.acciones().push(format!("kill {}", identity.pid));
    }

    fn wait(&self, _pid: u32, _deadline: Duration) -> Option<i32> {
        Some(0)
    }

    fn detach(&self, pid: u32) -> Result<(), String> {
        self.acciones().push(format!("detach {pid}"));
        Ok(())
    }
}

/// La sonda doble: responde lo que se le dice.
struct FakeProbe {
    /// Cuántas veces se ha llamado.
    llamadas: Mutex<u32>,
    /// Si responde a partir de la llamada número N.
    responde_a_partir_de: u32,
}

impl FakeProbe {
    fn siempre() -> Self {
        Self {
            llamadas: Mutex::new(0),
            responde_a_partir_de: 0,
        }
    }

    fn nunca() -> Self {
        Self {
            llamadas: Mutex::new(0),
            responde_a_partir_de: u32::MAX,
        }
    }
}

impl Probe for FakeProbe {
    fn ready(&self, _port: u16, _model: &str) -> bool {
        let mut llamadas = self.llamadas.lock().expect("el candado de la sonda");
        *llamadas += 1;
        *llamadas > self.responde_a_partir_de
    }
}

/// El device doble: no hay ningún proceso de ropero, salvo que se diga.
struct FakeDevice {
    procesos: Vec<ProcIdentity>,
}

impl FakeDevice {
    fn vacio() -> Self {
        Self {
            procesos: Vec::new(),
        }
    }

    fn con(procesos: Vec<ProcIdentity>) -> Self {
        Self { procesos }
    }
}

impl ropero_runtime::launch::Device for FakeDevice {
    fn ropero_processes(&self) -> Vec<ProcIdentity> {
        self.procesos.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Los helpers
// ─────────────────────────────────────────────────────────────────────────────

/// El spec de un arranque de prueba.
fn spec() -> LaunchSpec {
    LaunchSpec {
        model: "groot".to_string(),
        engine: EngineId::LlamaServer,
        engine_name: "llama-server".to_string(),
        server_api_key: None,
        program: PathBuf::from("/usr/bin/llama-server"),
        argv: vec!["--model".to_string(), "/models/groot.gguf".to_string()],
        level: "think".to_string(),
        context: 32768,
        log: PathBuf::from("/tmp/groot.log"),
        port_range: PortRange::new(8100, 8199),
    }
}

/// Un preflight que se cumple.
fn preflight_ok() -> Preflight {
    Preflight {
        engine: "llama-server".to_string(),
        engine_installed: true,
        model_installed: true,
        vram_available_bytes: Some(24 * 1024 * 1024 * 1024),
        ram_available_bytes: Some(64 * 1024 * 1024 * 1024),
        context_requested: 32768,
        vram_required_bytes: Some(8 * 1024 * 1024 * 1024),
        ram_required_bytes: Some(16 * 1024 * 1024 * 1024),
        cpu: false,
    }
}

/// Un plan de sonda rápido: los tests no esperan.
fn plan() -> ProbePlan {
    ProbePlan {
        backoff: Duration::from_millis(1),
        deadline: Duration::from_millis(50),
    }
}

/// Las opciones de un arranque de prueba.
fn opciones() -> LaunchOptions {
    LaunchOptions {
        probe: plan(),
        foreground: false,
        force: false,
        cpu: false,
    }
}

/// Un device vacío.
fn device_vacio() -> FakeDevice {
    FakeDevice::vacio()
}

/// El contexto de un arranque de prueba.
///
/// El directorio de estado es el del temporal de cada test, así que
/// entra por parámetro.
fn contexto<'a>(
    engine: &'a FakeEngine,
    probe: &'a FakeProbe,
    device: &'a FakeDevice,
    preflight: &'a Preflight,
    state_dir: &'a Path,
) -> LaunchContext<'a> {
    LaunchContext {
        engine,
        probe,
        device,
        preflight,
        port: None,
        occupied_ports: Vec::new(),
        state_dir,
        options: opciones(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Los tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_arranque_escriba_el_estado_en_disco() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());
    let resultado =
        ropero_runtime::launch::launch(&spec(), &ctx).expect("el arranque tiene que funcionar");

    assert_eq!(resultado.port, 8100);
    assert_eq!(resultado.pid, 4242);

    // El estado se escribió en $STATE_DIR/ports/<puerto>.
    let ruta = dir.path().join("ports").join("8100");
    let texto = std::fs::read_to_string(&ruta).expect("el fichero de estado");
    assert!(texto.contains("MODEL=groot"), "estado={texto}");
    assert!(texto.contains("ENGINE=llama-server"), "estado={texto}");
    assert!(texto.contains("PID=4242"), "estado={texto}");
    assert!(texto.contains("PORT=8100"), "estado={texto}");
    assert!(texto.contains("LEVEL=think"), "estado={texto}");
    assert!(texto.contains("LOG=/tmp/groot.log"), "estado={texto}");
    assert!(texto.contains("STAMP="), "estado={texto}");
    assert!(texto.contains("TS="), "estado={texto}");
    assert!(texto.contains("VERSION=2"), "estado={texto}");
    assert!(texto.contains("STARTTIME=1000000"), "estado={texto}");
}

#[test]
fn el_arranque_pasa_el_puerto_al_motor() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    ropero_runtime::launch::launch(
        &spec(),
        &contexto(&engine, &probe, &device, &preflight, dir.path()),
    )
    .expect("el arranque tiene que funcionar");

    let acciones = engine.acciones();
    let spawn = acciones
        .iter()
        .find(|a| a.starts_with("spawn"))
        .expect("hubo spawn");
    assert!(spawn.contains("--port 8100"), "spawn={spawn}");
    // El argv del modelo va delante del puerto.
    assert!(
        spawn.contains("--model /models/groot.gguf"),
        "spawn={spawn}"
    );
}

#[test]
fn un_puerto_pedido_se_usa_tal_cual() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = LaunchContext {
        port: Some(9999),
        ..contexto(&engine, &probe, &device, &preflight, dir.path())
    };
    let resultado =
        ropero_runtime::launch::launch(&spec(), &ctx).expect("el arranque tiene que funcionar");
    assert_eq!(resultado.port, 9999);
    assert!(
        dir.path().join("ports").join("9999").exists(),
        "el estado del puerto pedido tiene que estar"
    );
}

#[test]
fn un_preflight_que_falla_no_lanza_nada() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let mut preflight = preflight_ok();
    preflight.engine_installed = false;
    let device = device_vacio();

    let ctx = LaunchContext {
        preflight: &preflight,
        ..contexto(&engine, &probe, &device, &preflight, dir.path())
    };
    let error =
        ropero_runtime::launch::launch(&spec(), &ctx).expect_err("el preflight tenía que fallar");

    assert!(matches!(error, LaunchError::Preflight(_)), "error={error}");
    // Y lo importante: **no** se lanzó nada.
    assert!(
        engine.acciones().is_empty(),
        "acciones={:?}",
        engine.acciones()
    );
    // Tampoco se escribió estado.
    assert!(!dir.path().join("ports").exists(), "no debe haber estado");
}

#[test]
fn un_motor_que_no_responde_a_la_sonda_es_un_error() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::nunca();

    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());
    let error = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect_err("la sonda no pasó: tiene que ser error");

    assert!(
        matches!(error, LaunchError::NotReady { .. }),
        "error={error}"
    );
    // El proceso se para para no dejar un zombi.
    assert!(
        engine.acciones().iter().any(|a| a == "term 4242"),
        "acciones={:?}",
        engine.acciones()
    );
    // Y el estado se borra: ya no describe nada.
    assert!(
        !dir.path().join("ports").join("8100").exists(),
        "el estado de un arranque fallido no debe quedar"
    );
}

#[test]
fn un_motor_que_muere_durante_el_arranque_es_un_error_distinto() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo().muerto();
    let probe = FakeProbe::nunca();

    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());
    let error = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect_err("el motor murió: tiene que ser error");

    assert!(matches!(error, LaunchError::Died { .. }), "error={error}");
}

/// Si el motor muere al hacer bind, se prueba con el siguiente puerto.
///
/// La carrera: dos procesos que eligen el mismo rango de puertos ven el
/// mismo puerto libre y los dos lo cogen. El que pierde muere al hacer
/// bind («address already in use»). Si el arranque no se reintentara, se
/// quedaría sin motor y sin estado en disco.
#[test]
fn si_el_motor_muere_al_hacer_bind_se_prueba_el_siguiente_puerto() {
    let dir = TempDir::nuevo();
    // El motor pierde el puerto 8100 (otro proceso lo cogió) y vive en
    // el siguiente.
    let engine = FakeEngine::nuevo().que_muere_en(8100);
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());

    let resultado = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect("el arranque tiene que reintentar con otro puerto");

    // El arranque acaba en el segundo puerto, no en el que se perdió.
    assert_eq!(
        resultado.port, 8101,
        "el arranque tiene que acabar en el puerto que el motor cogió"
    );
    // Hubo dos spawns: el que se perdió y el que salió bien.
    let acciones = engine.acciones();
    let spawns: Vec<&String> = acciones.iter().filter(|a| a.starts_with("spawn")).collect();
    assert_eq!(spawns.len(), 2, "acciones={acciones:?}");
    assert!(spawns[0].contains("--port 8100"), "spawn={}", spawns[0]);
    assert!(spawns[1].contains("--port 8101"), "spawn={}", spawns[1]);
    // Y el estado está en el puerto del motor vivo: en el perdido no
    // puede haber nada, porque ese motor nunca sirvió.
    assert!(
        dir.path().join("ports").join("8101").exists(),
        "el estado tiene que estar en el puerto bueno"
    );
    assert!(
        !dir.path().join("ports").join("8100").exists(),
        "no puede haber estado en el puerto que se perdió"
    );
}

/// Un puerto pedido por el usuario no se reintenta con otro.
///
/// Si el usuario pide un puerto, se usa ese: si el motor no puede
/// hacer bind, es un error, no una señal para coger otro puerto.
#[test]
fn un_puerto_pedido_que_el_motor_no_puede_coger_no_se_reintenta() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo().que_muere_en(9999);
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = LaunchContext {
        port: Some(9999),
        ..contexto(&engine, &probe, &device, &preflight, dir.path())
    };

    let error = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect_err("el motor no pudo hacer bind: tiene que ser error");

    match error {
        LaunchError::Spawn(mensaje) => {
            assert!(mensaje.contains("9999"), "mensaje={mensaje}");
        }
        otra => panic!("el error tiene que ser de spawn: {otra}"),
    }
    // Y sólo se intentó una vez: el puerto pedido no se cambia.
    let acciones = engine.acciones();
    let spawns: Vec<&String> = acciones.iter().filter(|a| a.starts_with("spawn")).collect();
    assert_eq!(spawns.len(), 1, "acciones={acciones:?}");
}

/// El reintento está acotado por el rango: no se prueba eternamente.
///
/// Si el motor muere al hacer bind en **todos** los puertos del rango, el
/// arranque se rinde cuando ya no queda ninguno. El bucle no puede dar
/// vueltas: cada intento descarta un puerto.
#[test]
fn el_reintento_se_acota_con_el_rango_de_puertos() {
    let dir = TempDir::nuevo();
    // El motor no puede hacer bind en ningún puerto: el rango entero
    // está cogido.
    let engine = FakeEngine::nuevo().que_muere_siempre();
    let probe = FakeProbe::siempre();
    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());

    let error = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect_err("el motor no pudo hacer bind en ningún puerto: tiene que ser error");

    assert!(matches!(error, LaunchError::NoPort(..)), "error={error}");
    // Y se probó cada puerto del rango, y sólo cada uno: 8100-8199.
    let acciones = engine.acciones();
    let spawns: Vec<&String> = acciones.iter().filter(|a| a.starts_with("spawn")).collect();
    assert_eq!(spawns.len(), 100, "acciones={:?}", acciones.len());
    assert!(spawns[0].contains("--port 8100"), "spawn={}", spawns[0]);
    assert!(spawns[99].contains("--port 8199"), "spawn={}", spawns[99]);
}

#[test]
fn un_spawn_que_falla_es_un_error() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::que_falla_el_spawn();
    let probe = FakeProbe::siempre();

    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());
    let error = ropero_runtime::launch::launch(&spec(), &ctx)
        .expect_err("el spawn falló: tiene que ser error");

    assert!(matches!(error, LaunchError::Spawn(_)), "error={error}");
}

#[test]
fn con_force_se_paran_los_procesos_de_ropero_del_device() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let device = FakeDevice::con(vec![ProcIdentity::with_digest(111, 100, None)]);

    let preflight = preflight_ok();
    let vacio = device_vacio();
    let ctx = LaunchContext {
        device: &device,
        options: LaunchOptions {
            force: true,
            ..opciones()
        },
        ..contexto(&engine, &probe, &vacio, &preflight, dir.path())
    };
    ropero_runtime::launch::launch(&spec(), &ctx).expect("el arranque tiene que funcionar");

    assert!(
        engine.acciones().iter().any(|a| a == "term 111"),
        "acciones={:?}",
        engine.acciones()
    );
}

#[test]
fn sin_force_no_se_toca_nada_del_device() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();
    let device = FakeDevice::con(vec![ProcIdentity::with_digest(111, 100, None)]);

    let preflight = preflight_ok();
    let vacio = device_vacio();
    let ctx = LaunchContext {
        device: &device,
        ..contexto(&engine, &probe, &vacio, &preflight, dir.path())
    };
    ropero_runtime::launch::launch(&spec(), &ctx).expect("el arranque tiene que funcionar");

    assert!(
        !engine.acciones().iter().any(|a| a == "term 111"),
        "acciones={:?}",
        engine.acciones()
    );
}

#[test]
fn la_sonda_se_reintenta_hasta_que_pasa() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe {
        llamadas: Mutex::new(0),
        responde_a_partir_de: 3,
    };

    let preflight = preflight_ok();
    let device = device_vacio();
    let ctx = contexto(&engine, &probe, &device, &preflight, dir.path());
    let resultado =
        ropero_runtime::launch::launch(&spec(), &ctx).expect("el arranque tiene que funcionar");

    assert_eq!(resultado.port, 8100);
    assert!(*probe.llamadas.lock().expect("candado") >= 4);
}

#[test]
fn el_sello_cambia_cuando_cambia_el_argv() {
    let dir = TempDir::nuevo();
    let engine = FakeEngine::nuevo();
    let probe = FakeProbe::siempre();

    let preflight = preflight_ok();
    let device = device_vacio();
    let primero = ropero_runtime::launch::launch(
        &spec(),
        &contexto(&engine, &probe, &device, &preflight, dir.path()),
    )
    .expect("el arranque tiene que funcionar");

    let mut otro = spec();
    otro.argv.push("--ctx-size".to_string());
    otro.argv.push("8192".to_string());
    let segundo = ropero_runtime::launch::launch(
        &otro,
        &contexto(&engine, &probe, &device, &preflight, dir.path()),
    )
    .expect("el arranque tiene que funcionar");

    assert_ne!(
        primero.state.stamp, segundo.state.stamp,
        "un argv distinto tiene que dar un sello distinto"
    );
    assert!(!primero.state.stamp.is_empty());
}
