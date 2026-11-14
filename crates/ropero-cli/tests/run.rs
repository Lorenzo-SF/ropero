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
    cmd.env("LOG_DIR", fixture_root().join("logs"));
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
}

impl Arranque {
    /// Lanza el modelo en segundo plano y espera al `ROPERO_PORT=`.
    fn nuevo(estado: &Path, path: &Path, gguf: &Path, args: &[&str]) -> (Self, u16) {
        let mut cmd = bin(estado, path, gguf);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut hijo = cmd.spawn().expect("arrancar");

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
        let puerto = puerto.expect("el motor no arrancó: no salió ROPERO_PORT=");
        let arranque = Self { hijo, lineas };
        (arranque, puerto)
    }

    /// Las líneas que ha escrito hasta ahora.
    fn lineas(&self) -> Vec<String> {
        self.lineas.lock().expect("candado").clone()
    }
}

impl Drop for Arranque {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
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
fn un_modelo_inexistente_sale_con_uno() {
    let motor = MotorFalso::nuevo();
    let modelo = prepara_modelo_descargado();
    let estado = TempDir::nuevo("inexistente");

    let (code, _stdout, stderr) = run(estado.path(), motor.path(), modelo.path(), &["no-existe"]);

    assert_eq!(code, 1, "stderr={stderr}");
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

/// Si un proceso sigue vivo.
fn proceso_vivo(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
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
