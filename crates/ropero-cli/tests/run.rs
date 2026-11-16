//! Tests de integración del arranque y la parada en la CLI.
//!
//! ## El motor falso
//!
//! Para probar el arranque de verdad hace falta un motor: algo que se
//! lance, escuche en un puerto y responda. El fixture de estos tests
//! monta uno: un script que hace las tres cosas.
//!
//! ## El entorno
//!
//! Los tests son **herméticos**: apuntan el catálogo al fixture, la
//! variable de estado a un directorio temporal, y el `PATH` a un
//! directorio donde sólo está el motor falso. Así el motor falso es el
//! único que ropero puede encontrar, y el test no depende de lo que haya
//! instalado en la máquina.
//!
//! ## El arranque es en primer plano
//!
//! `ropero <modelo>` **espera** al motor: es lo que dice el doc CLI v2
//! §8 (el Ctrl-C lo detiene). Por eso los tests que arrancan lo lanzan
//! en segundo plano y leen su salida mientras corre.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// El directorio del fixture.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Un directorio temporal que se borra al soltarlo.
struct TempDir(PathBuf);

impl TempDir {
    fn nuevo(nombre: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "ropero-cli-{nombre}-{}-{}",
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

/// El motor falso: se lanza, escucha y responde.
///
/// Va en un directorio **propio** de cada test: si compartieran el
/// directorio, un test que deja el motor puesto haría que otro test
/// creído que no está instalado lo encontrara.
struct MotorFalso {
    dir: TempDir,
}

impl MotorFalso {
    /// Prepara el motor falso.
    fn nuevo() -> Self {
        let dir = TempDir::nuevo("motor");
        let ruta = dir.path().join("llama-server");
        std::fs::write(&ruta, MOTOR_FALSO).expect("escribir el motor falso");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ruta).expect("metadatos").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ruta, perms).expect("permisos");
        }
        Self { dir }
    }

    /// El `PATH` donde está el motor falso.
    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// El motor falso: se lanza, escucha y responde.
const MOTOR_FALSO: &str = r#"#!/bin/sh
# Motor falso del fixture: escucha en el puerto que le pasan y responde.
puerto=""
previo=""
for arg in "$@"; do
    if [ "$previo" = "--port" ]; then
        puerto="$arg"
    fi
    previo="$arg"
done

if [ -z "$puerto" ]; then
    echo "no me han pasado --port" >&2
    exit 1
fi

if [ -x /usr/bin/python3 ]; then
    exec /usr/bin/python3 -c '
import http.server, socketserver, sys

puerto = int(sys.argv[1])

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        cuerpo = b"{\"data\":[{\"id\":\"modelo-alpha\"}]}"
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)
    def log_message(self, *a):
        pass

socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", puerto), H) as s:
    s.serve_forever()
' "$puerto"
fi

# Sin python3: se queda vivo y ya está.
while true; do
    sleep 1
done
"#;

/// El comando `ropero` con el entorno del fixture.
///
/// El `PATH` es el del motor falso que se le pase: así cada test ve
/// **su** motor, y uno sin motor ve un `PATH` vacío (motor no
/// instalado).
fn bin(estado: &Path, path: &Path, gguf: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ropero"));
    cmd.env("ROPERO_MODELS_DIR", fixture_root());
    cmd.env("GGUF_DIR", gguf);
    cmd.env("SAFETENSORS_DIR", fixture_root().join("safetensors"));
    cmd.env("LOG_DIR", estado.join("logs"));
    // El estado en disco va a un temporal: los tests no tocan el estado
    // real de la máquina.
    cmd.env("ROPERO_STATE_DIR", estado);
    cmd.env("PATH", path);
    cmd
}

/// Ejecuta el binario y devuelve (exit code, stdout, stderr).
fn run(estado: &Path, path: &Path, gguf: &Path, args: &[&str]) -> (i32, String, String) {
    let out = bin(estado, path, gguf).args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Un modelo descargado: el fixture lo declara, pero el fichero no
/// existe. Se crea para que el preflight pase.
///
/// El GGUF va a un directorio **propio** de cada test: si compartieran
/// el directorio, un test que lo crea haría que otro test-creído que el
/// modelo no está descargado lo encontrara.
fn prepara_modelo_descargado() -> TempDir {
    let dir = TempDir::nuevo("modelo");
    let ruta = dir.path().join("modelo-alpha");
    std::fs::create_dir_all(&ruta).expect("crear el directorio del modelo");
    std::fs::write(ruta.join("modelo-alpha-q4_k_m.gguf"), b"falso").expect("escribir el gguf");
    dir
}

/// Un arranque en segundo plano: se lee su stdout línea a línea.
///
/// El arranque en primer plano **espera** al motor (doc CLI v2 §8), así
/// que el test lo lanza en segundo plano y lee el `ROPERO_PORT=` cuando
/// sale.
struct Arranque {
    hijo: Child,
    lineas: std::sync::Arc<Mutex<Vec<String>>>,
    stderr: std::sync::Arc<Mutex<Vec<String>>>,
    /// El directorio de estado: de aquí sale el PID del motor.
    estado: PathBuf,
    /// El puerto en el que sirve el motor.
    puerto: u16,
}

impl Arranque {
    /// Lanza el modelo en segundo plano y espera al `ROPERO_PORT=`.
    fn nuevo(estado: &Path, path: &Path, gguf: &Path, args: &[&str]) -> (Self, u16) {
        let mut cmd = bin(estado, path, gguf);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut hijo = cmd.spawn().expect("arrancar");

        let stderr = hijo.stderr.take().expect("stderr");
        let errores = std::sync::Arc::new(Mutex::new(Vec::new()));
        let del_lector_err = std::sync::Arc::clone(&errores);
        std::thread::spawn(move || {
            for linea in BufReader::new(stderr).lines().map_while(Result::ok) {
                del_lector_err.lock().expect("candado").push(linea);
            }
        });
        let stdout = hijo.stdout.take().expect("stdout");
        let lineas = std::sync::Arc::new(Mutex::new(Vec::new()));
        let del_lector = std::sync::Arc::clone(&lineas);
        std::thread::spawn(move || {
            for linea in BufReader::new(stdout).lines().map_while(Result::ok) {
                del_lector.lock().expect("candado").push(linea);
            }
        });

        // Se espera al `ROPERO_PORT=`: es la señal de que arrancó.
        let mut puerto = None;
        for _ in 0..200 {
            let lineas = lineas.lock().expect("candado");
            if let Some(linea) = lineas
                .iter()
                .find(|l| l.starts_with("ROPERO_PORT="))
                .cloned()
            {
                puerto = linea.trim_start_matches("ROPERO_PORT=").parse::<u16>().ok();
                break;
            }
            drop(lineas);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let logs: Vec<String> = std::fs::read_dir(estado.join("logs"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| {
                        format!(
                            "{}: {}",
                            e.file_name().to_string_lossy(),
                            std::fs::read_to_string(e.path()).unwrap_or_default()
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let puerto = puerto.unwrap_or_else(|| {
            panic!(
                "el motor no arrancó: no salió ROPERO_PORT=; stderr={:?}; logs={logs:?}",
                errores.lock().expect("candado")
            )
        });
        let arranque = Self {
            hijo,
            lineas,
            stderr: errores,
            estado: estado.to_path_buf(),
            puerto,
        };
        (arranque, puerto)
    }

    /// Las líneas que ha escrito hasta ahora.
    fn lineas(&self) -> Vec<String> {
        self.lineas.lock().expect("candado").clone()
    }

    /// Las líneas de stderr que ha escrito hasta ahora.
    fn stderr_lineas(&self) -> Vec<String> {
        self.stderr.lock().expect("candado").clone()
    }

    /// Interrumpe ropero para que haga limpieza (uso, estado, motor).
    fn interrumpir(&mut self) {
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-INT", &self.hijo.id().to_string()])
                .status();
        }
        for _ in 0..40 {
            if let Ok(Some(_)) = self.hijo.try_wait() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for Arranque {
    fn drop(&mut self) {
        // Intenta parada limpia primero (permite registrar uso/limpieza).
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-INT", &self.hijo.id().to_string()])
                .status();
        }
        for _ in 0..20 {
            if let Ok(Some(_)) = self.hijo.try_wait() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
        // El motor es un **nieto** del test: su padre es el `ropero`, y
        // aquí sólo se mata al `ropero`. Sin parar el motor también, se
        // queda escuchando en el puerto del rango y el siguiente test —o
        // la siguiente vuelta de la suite— se lo encuentra ocupado.
        para_el_motor(&self.estado, self.puerto);
    }
}

/// Para el motor que sirve en el puerto, si sigue vivo.
///
/// Se para con la **misma** parada que usa `ropero stop`: el PID del
/// estado se verifica con su `start_time` antes de mandar ninguna señal
/// al grupo, para que un PID reciclado no reciba un SIGKILL que no es
/// nuestro.
///
/// El PID sale del estado en disco: si el estado ya no está, es que un
/// `stop` ya lo paró.
fn para_el_motor(estado: &Path, puerto: u16) {
    let stopper = ropero_runtime::OsStopper;
    let _ = ropero_runtime::SysStopper::parar_puerto(
        estado,
        puerto,
        &stopper,
        std::time::Duration::from_secs(2),
    );
}

/// Si alguien está escuchando en el puerto.
fn puerto_escuchando(puerto: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", puerto)).is_ok()
}

/// El arranque de un modelo sale con 0 y dice el puerto.
///
/// El contrato R-OC: `ROPERO_PORT=<n>` es la **última** línea de stdout.
#[test]
fn el_arranque_dice_el_puerto() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("arranque");

    let (arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );

    // El puerto sale del rango del motor del fixture (turbo-llama:
    // 8200-8299).
    assert!(
        (8200..=8299).contains(&puerto),
        "el puerto {puerto} tiene que estar en el rango del motor"
    );
    // Y la línea `ROPERO_PORT=` es la última de stdout.
    let lineas = arranque.lineas();
    let ultima = lineas.last().expect("stdout no vacío");
    assert!(
        ultima.starts_with("ROPERO_PORT="),
        "la última línea tiene que ser ROPERO_PORT=: {lineas:?}"
    );
}

#[test]
fn el_arranque_escribe_el_estado_en_disco() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("estado");

    let (_arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );

    let fichero = estado.path().join("ports").join(puerto.to_string());
    let texto = std::fs::read_to_string(&fichero).expect("el fichero de estado");
    assert!(texto.contains("MODEL=modelo-alpha"), "estado={texto}");
    assert!(texto.contains("ENGINE=turbo-llama"), "estado={texto}");
    assert!(texto.contains(&format!("PORT={puerto}")), "estado={texto}");
    assert!(texto.contains("PID="), "estado={texto}");
    assert!(texto.contains("STAMP="), "estado={texto}");
    assert!(texto.contains("TS="), "estado={texto}");
    assert!(texto.contains("LOG="), "estado={texto}");
    assert!(texto.contains("LEVEL="), "estado={texto}");
    assert!(texto.contains("STARTTIME="), "estado={texto}");
    assert!(texto.contains("VERSION=2"), "estado={texto}");
}

#[test]
fn el_arranque_con_cpu_tambien_arranca() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("cpu");

    let (_arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha", "--cpu"],
    );

    assert!((8200..=8299).contains(&puerto), "puerto={puerto}");
}

#[test]
fn el_arranque_con_un_puerto_pedido_usa_ese_puerto() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("puerto");

    let (_arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha", "--port", "8299"],
    );

    assert_eq!(puerto, 8299);
    assert!(
        estado.path().join("ports").join("8299").exists(),
        "el estado del puerto pedido tiene que estar"
    );
}

#[test]
fn un_modelo_inexistente_sale_con_dos() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("inexistente");

    let (code, _stdout, stderr) = run(estado.path(), motor.path(), modelo.path(), &["no-existe"]);

    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("no-existe"), "stderr={stderr}");
}

#[test]
fn un_motor_que_no_esta_instalado_sale_con_uno() {
    // Un `PATH` vacío: el motor no está instalado.
    let vacio = TempDir::nuevo("sin-path");
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("sin-motor");

    let (code, _stdout, stderr) = run(
        estado.path(),
        vacio.path(),
        modelo.path(),
        &["modelo-alpha"],
    );

    assert_eq!(code, 1, "stderr={stderr}");
    // El mensaje dice **qué hacer**.
    assert!(stderr.contains("ropero install engines"), "stderr={stderr}");
}

#[test]
fn un_modelo_que_no_esta_descargado_sale_con_uno() {
    let motor = MotorFalso::nuevo();
    // **Sin** preparar el modelo: el GGUF no existe.
    let vacio = TempDir::nuevo("sin-gguf");
    let estado = TempDir::nuevo("sin-modelo");

    let (code, _stdout, stderr) = run(estado.path(), motor.path(), vacio.path(), &["modelo-alpha"]);

    assert_eq!(code, 1, "stderr={stderr}");
    assert!(stderr.contains("ropero install models"), "stderr={stderr}");
}

#[test]
fn stop_sin_nada_que_parar_no_es_un_error() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("stop-vacio");

    let (code, stdout, stderr) = run(estado.path(), motor.path(), modelo.path(), &["stop"]);

    assert_eq!(code, 0, "stdout={stdout}\nstderr={stderr}");
}

#[test]
fn stop_de_un_modelo_que_no_corre_no_es_un_error() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("stop-modelo");

    let (code, stdout, stderr) = run(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["stop", "modelo-alpha"],
    );

    assert_eq!(code, 0, "stdout={stdout}\nstderr={stderr}");
}

#[test]
fn stop_para_el_motor_que_arranco() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("stop-real");

    // 1. Se arranca el modelo.
    let (arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );

    // 2. Se lee el PID del estado.
    let ruta = estado.path().join("ports").join(puerto.to_string());
    let texto = espera_estado(&ruta);
    let pid: u32 = texto
        .lines()
        .find_map(|l| l.strip_prefix("PID="))
        .and_then(|v| v.parse().ok())
        .expect("el PID del estado");

    // 3. Se para.
    let (code, stdout, stderr) = run(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["stop", "modelo-alpha"],
    );
    assert_eq!(code, 0, "stdout={stdout}\nstderr={stderr}");

    // 4. El proceso está muerto y el estado, borrado.
    drop(arranque);
    for _ in 0..50 {
        if !proceso_vivo(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !proceso_vivo(pid),
        "el proceso {pid} tiene que estar muerto tras el stop"
    );
    assert!(
        !ruta.exists(),
        "el estado tiene que desaparecer tras el stop"
    );
}

#[test]
fn stop_sin_argumento_para_todos() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("stop-todos");

    let (arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );
    let ruta = estado.path().join("ports").join(puerto.to_string());
    let texto = espera_estado(&ruta);
    let pid: u32 = texto
        .lines()
        .find_map(|l| l.strip_prefix("PID="))
        .and_then(|v| v.parse().ok())
        .expect("el PID del estado");

    let (code, stdout, stderr) = run(estado.path(), motor.path(), modelo.path(), &["stop"]);
    assert_eq!(code, 0, "stdout={stdout}\nstderr={stderr}");

    drop(arranque);
    for _ in 0..50 {
        if !proceso_vivo(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !proceso_vivo(pid),
        "el proceso {pid} tiene que estar muerto tras el stop"
    );
    assert!(!ruta.exists(), "el estado tiene que desaparecer");
}

/// Si el motor sigue vivo.
fn proceso_vivo(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// El motor del test no se queda escuchando.
///
/// El motor falso es un nieto del test (su padre es el `ropero`): si no
/// se para al acabar el test, se queda sirviendo en el puerto del rango
/// y el siguiente test —o la siguiente vuelta de la suite— se lo
/// encuentra ocupado.
#[test]
fn el_motor_del_test_no_se_queda_escuchando() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("limpieza");

    let (arranque, puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );
    assert!(
        puerto_escuchando(puerto),
        "el motor tiene que estar escuchando mientras el test dura"
    );

    // Al soltar el arranque, el motor tiene que estar parado.
    drop(arranque);
    for _ in 0..50 {
        if !puerto_escuchando(puerto) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("el motor {puerto} tiene que estar parado tras el test");
}

/// Espera a que el fichero de estado exista y devuelve su contenido.
fn espera_estado(ruta: &Path) -> String {
    for _ in 0..100 {
        if let Ok(texto) = std::fs::read_to_string(ruta) {
            return texto;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("el estado no apareció: {}", ruta.display());
}

/// Motor falso que emite una línea identificable en su salida.
const MOTOR_FALSO_CON_SALIDA: &str = r#"#!/bin/sh
# Motor falso que escribe salida visible antes de servir.
echo "MOTOR_SALIDA_VISIBLE" >&2
puerto=""
previo=""
for arg in "$@"; do
    if [ "$previo" = "--port" ]; then
        puerto="$arg"
    fi
    previo="$arg"
done

if [ -z "$puerto" ]; then
    echo "no me han pasado --port" >&2
    exit 1
fi

if [ -x /usr/bin/python3 ]; then
    exec /usr/bin/python3 -c '
import http.server, socketserver, sys

puerto = int(sys.argv[1])

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        cuerpo = b"{\"data\":[{\"id\":\"modelo-alpha\"}]}"
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)
    def log_message(self, *a):
        pass

socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("127.0.0.1", puerto), H) as s:
    s.serve_forever()
' "$puerto"
fi

while true; do
    sleep 1
done
"#;

struct MotorFalsoConSalida {
    dir: TempDir,
}

impl MotorFalsoConSalida {
    fn nuevo() -> Self {
        let dir = TempDir::nuevo("motor-salida");
        let ruta = dir.path().join("llama-server");
        std::fs::write(&ruta, MOTOR_FALSO_CON_SALIDA).expect("escribir el motor falso");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ruta).expect("metadatos").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ruta, perms).expect("permisos");
        }
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// Lee el fichero de log del modelo, si existe.
fn lee_log_modelo(estado: &Path, modelo: &str) -> String {
    let ruta = estado.join("logs").join(format!("{modelo}.log"));
    std::fs::read_to_string(ruta).unwrap_or_default()
}

/// Lee el log de uso.
fn lee_usage(estado: &Path) -> String {
    std::fs::read_to_string(estado.join("logs").join("usage.log")).unwrap_or_default()
}

/// Puerto del estado del primer modelo servido (polling).
fn puerto_en_estado(estado: &Path) -> Option<u16> {
    let dir = estado.join("ports");
    for _ in 0..200 {
        if let Ok(entradas) = std::fs::read_dir(&dir) {
            for e in entradas.flatten() {
                if let Ok(p) = e.file_name().to_string_lossy().parse::<u16>() {
                    return Some(p);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

#[test]
fn foreground_muestra_salida_del_motor_y_genera_log_y_uso() {
    let motor = MotorFalsoConSalida::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("fg-stream");

    let (arranque, _puerto) = Arranque::nuevo(
        estado.path(),
        motor.path(),
        modelo.path(),
        &["modelo-alpha"],
    );

    // La salida en vivo del motor se muestra en la terminal (stderr).
    for _ in 0..40 {
        let visto = arranque
            .stderr_lineas()
            .iter()
            .any(|l| l.contains("MOTOR_SALIDA_VISIBLE"));
        let log = lee_log_modelo(estado.path(), "modelo-alpha");
        if visto && log.contains("MOTOR_SALIDA_VISIBLE") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let log = lee_log_modelo(estado.path(), "modelo-alpha");
    assert!(log.contains("MOTOR_SALIDA_VISIBLE"), "log={log}");
    assert!(
        arranque
            .stderr_lineas()
            .iter()
            .any(|l| l.contains("MOTOR_SALIDA_VISIBLE")),
        "stderr={:?}",
        arranque.stderr_lineas()
    );

    // Detenemos con interrupción limpia y comprobamos el registro de uso.
    let mut arranque = arranque;
    arranque.interrumpir();
    drop(arranque);
    for _ in 0..40 {
        let uso = lee_usage(estado.path());
        if uso.contains("model=modelo-alpha") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let uso = lee_usage(estado.path());
    assert!(uso.contains("model=modelo-alpha"), "usage={uso}");
    assert!(uso.contains("engine=turbo-llama"), "usage={uso}");
    assert!(uso.contains("mode=foreground"), "usage={uso}");
    assert!(
        uso.contains("result=ok") || uso.contains("result=interrupted"),
        "usage={uso}"
    );
    assert!(
        !uso.contains("SECRET"),
        "usage no debe contener secretos: {uso}"
    );
}

#[test]
fn background_es_silencioso_guarda_logs_y_uso_redactado() {
    let motor = MotorFalsoConSalida::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("bg");

    let out = bin(estado.path(), motor.path(), modelo.path())
        .args(["modelo-alpha", "--background", "--api-key", "SECRET123"])
        .output()
        .expect("ejecutar");

    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(code, 0, "stdout={stdout}\nstderr={stderr}");
    assert!(
        !stdout.contains("ROPERO_PORT"),
        "background no debe imprimir puerto: stdout={stdout:?}"
    );
    assert!(stdout.trim().is_empty(), "stdout={stdout:?}");
    assert!(
        !stderr.contains("MOTOR_SALIDA_VISIBLE"),
        "background no debe mostrar salida del motor: stderr={stderr:?}"
    );

    let puerto = puerto_en_estado(estado.path()).expect("debe haber estado de puerto");
    assert!(puerto_escuchando(puerto), "el motor debe seguir escuchando");

    // Log del motor bajo logs_dir.
    for _ in 0..40 {
        let log = lee_log_modelo(estado.path(), "modelo-alpha");
        if !log.is_empty() || estado.path().join("logs").join("modelo-alpha.log").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        estado.path().join("logs").join("modelo-alpha.log").exists(),
        "debe existir el log del modelo en logs_dir"
    );

    let uso = lee_usage(estado.path());
    assert!(uso.contains("model=modelo-alpha"), "usage={uso}");
    assert!(uso.contains("mode=background"), "usage={uso}");
    assert!(uso.contains("result=ok"), "usage={uso}");
    assert!(
        !uso.contains("SECRET123"),
        "api-key no debe quedar en usage: {uso}"
    );
    assert!(
        !uso.contains("--api-key"),
        "flags sensibles no deben quedar: {uso}"
    );

    para_el_motor(estado.path(), puerto);
}
