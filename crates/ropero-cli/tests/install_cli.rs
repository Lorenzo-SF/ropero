//! Tests de **caja negra** de `ropero install` (F5).
//!
//! A diferencia de `tests/install.rs` (que fija el comportamiento que ya
//! tenía), esta batería **recorre el contrato normativo** de la fase de
//! instalación contra el **binario**, no contra las librerías:
//!
//! - doc B §4.10.7 — el contrato de `--prio` (valor, ámbito, precedencia,
//!   cuándo se ignora y qué pasa sin terminal).
//! - doc CLI §9 — `ropero install [<nada>|engines|models]`, la lista
//!   desatendida (ids o alias, con espacios o comas) y el orden
//!   `engines` → `models`.
//! - doc A — R-CLI-16 (modo desatendido), R-INST-26 (`install engines` sin
//!   TTY), R-MOD-16 (la secuencia de la descarga), R-MOD-18 (verificación
//!   por contenido real) y R-MOD-19 (un exit 0 de `hf` no es un modelo
//!   instalado).
//!
//! ## La batería es hermética
//!
//! Cada test monta su propia máquina de pruebas en un temporal:
//!
//! - **catálogo**: `models/<motor>/*.toml` y `engines/<motor>.toml`;
//! - **`$BIN_DIR`**: donde se ven los enlaces de los motores;
//! - **`GGUF_DIR`** y **`SAFETENSORS_DIR`**: donde acaban los artefactos;
//! - **`PATH`**: un temporal con un binario falso (el motor) y un **`hf`
//!   falso** que **graba sus argumentos** y crea el fichero que se le pide.
//!
//! Nada toca la máquina de quien ejecuta los tests, y el `PATH` no lleva
//! los binarios del sistema: el `hf` falso se invoca con `sh` y sólo usa
//! *builtins*.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ─────────────────────────────────────────────────────────────────────────────
// El catálogo de pruebas
// ─────────────────────────────────────────────────────────────────────────────

/// El manifiesto del motor de pruebas.
///
/// `detect.binaries` es `llama-server`: el binario falso del `PATH`.
const MOTOR: &str = r#"
id = "turbo-llama"
label = "turbo-llama (fork)"
color = "amarillo"
description = "llama.cpp (fork privado del dueño, turboquant)"
maturity = "proven"
formats = ["gguf", "safetensors"]
port_min = 8200
port_max = 8299
deps = ["git", "cmake"]

[detect]
binaries = ["llama-server"]
paths = ["/usr/local/bin/llama-server"]
"#;

/// Un modelo GGUF con **las dos** vías (`[download]` y `[build]`).
///
/// Es el caso en el que `--prio` decide (doc B §4.10.7).
fn modelo_ambos_vias(nombre: &str) -> String {
    format!(
        r#"
[model]
name = "{nombre}"
engine = "turbo-llama"

[files]
gguf = "{nombre}.gguf"

[download]
repo = "ejemplo/{nombre}"
file = "{nombre}.gguf"
size = "~4 GB"

[build]
repo = "ejemplo/{nombre}-pesos"
"#
    )
}

/// Un modelo GGUF con **sólo** `[download]`.
fn modelo_solo_download(nombre: &str) -> String {
    format!(
        r#"
[model]
name = "{nombre}"
engine = "turbo-llama"

[files]
gguf = "{nombre}.gguf"

[download]
repo = "ejemplo/{nombre}"
file = "{nombre}.gguf"
size = "~4 GB"
"#
    )
}

/// Un modelo GGUF con **sólo** `[build]`.
fn modelo_solo_build(nombre: &str) -> String {
    format!(
        r#"
[model]
name = "{nombre}"
engine = "turbo-llama"

[files]
gguf = "{nombre}.gguf"

[build]
repo = "ejemplo/{nombre}-pesos"
"#
    )
}

/// Un modelo GGUF con `[download]` y un **alias**.
fn modelo_con_alias(nombre: &str, alias: &str) -> String {
    format!(
        r#"
[model]
name = "{nombre}"
engine = "turbo-llama"
alias = "{alias}"

[files]
gguf = "{nombre}.gguf"

[download]
repo = "ejemplo/{nombre}"
file = "{nombre}.gguf"
size = "~4 GB"
"#
    )
}

/// Un checkpoint **safetensors** (R-MOD-17: sin staging, reanudable).
fn modelo_checkpoint(nombre: &str) -> String {
    format!(
        r#"
[model]
name = "{nombre}"
engine = "turbo-llama"
format = "safetensors"
path = "ejemplo/{nombre}"

[files]

[download]
repo = "ejemplo/{nombre}"
size = "~18 GB"
excludes = ["original/*"]
"#
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// El `hf` falso
// ─────────────────────────────────────────────────────────────────────────────

/// El `hf` falso: graba sus argumentos y finge la descarga.
///
/// El **modo** se lee de un fichero al lado del script (`modo`), así cada
/// test cambia el comportamiento de `hf` sin tocar el entorno del proceso:
///
/// | Modo | Qué hace |
/// |---|---|
/// | `ok` (por defecto) | crea el fichero pedido (GGUF) o los pesos y el `config.json` (checkpoint) |
/// | `nada` | sale con 0 **sin dejar nada** (R-MOD-19) |
/// | `vacio` | deja el fichero **vacío** (R-MOD-18) |
///
/// Sólo *builtins*: el `PATH` de la batería es un temporal (sin `dirname`
/// ni `mkdir`); el directorio de destino lo crea `ropero`.
const HF_FALSO: &str = r#"#!/bin/sh
base="${0%/*}"
modo="ok"
if [ -f "$base/modo" ]; then
  modo=""
  while IFS= read -r linea || [ -n "$linea" ]; do modo="$linea"; done < "$base/modo"
fi

# Cada invocación se anota con un separador, para que el test pueda ver
# cuántas veces se llamó a `hf` y con qué argumentos.
{ echo "== invocacion =="; for a in "$@"; do echo "$a"; done; } >> "$base/args.log"

dir=""
prev=""
for a in "$@"; do
  if [ "$prev" = "--local-dir" ]; then dir="$a"; fi
  prev="$a"
done
[ -n "$dir" ] || exit 3

# Los posicionales: el primero es el repo y el segundo (si lo hay) el fichero.
n=0
file=""
skip=0
for a in "$@"; do
  if [ "$skip" = "1" ]; then skip=0; continue; fi
  case "$a" in
    download) continue ;;
    --local-dir|--exclude) skip=1; continue ;;
    -*) continue ;;
    *) n=$((n + 1)); if [ "$n" = "2" ]; then file="$a"; fi ;;
  esac
done

case "$modo" in
  nada) : ;;
  vacio)
    if [ -n "$file" ]; then : > "$dir/$file"; fi
    ;;
  *)
    if [ -n "$file" ]; then
      printf 'contenido falso\n' > "$dir/$file"
    else
      printf 'pesos\n' > "$dir/model.safetensors"
      printf '{}\n' > "$dir/config.json"
    fi
    ;;
esac
exit 0
"#;

// ─────────────────────────────────────────────────────────────────────────────
// La máquina de pruebas
// ─────────────────────────────────────────────────────────────────────────────

/// El separador que el `hf` falso pone delante de cada invocación.
const SEPARADOR: &str = "== invocacion ==";

/// El programa que da un **pty** para probar el menú.
///
/// Sin terminal no hay menú (R-INST-26), así que el flujo interactivo se
/// prueba con un pty: `script` lo da sin dependencias (util-linux en Linux,
/// BSD en macOS).
fn script() -> PathBuf {
    for dir in ["/usr/bin", "/bin", "/usr/local/bin"] {
        let ruta = Path::new(dir).join("script");
        if ruta.is_file() {
            return ruta;
        }
    }
    panic!("no se encuentra 'script', que es lo que da el pty del menú");
}

/// La máquina de pruebas: catálogo, rutas y `PATH` controlados.
struct Fake {
    /// La raíz del temporal.
    base: PathBuf,
    /// La raíz del catálogo (`ROPERO_MODELS_DIR`).
    root: PathBuf,
    /// El directorio de binarios (`ROPERO_BIN_DIR`).
    bin: PathBuf,
    /// El `PATH` controlado.
    path: PathBuf,
    /// El directorio de los GGUF (`GGUF_DIR`).
    gguf: PathBuf,
    /// El directorio de los checkpoints (`SAFETENSORS_DIR`).
    safetensors: PathBuf,
}

impl Fake {
    /// Monta la máquina de pruebas en un temporal.
    ///
    /// El catálogo trae el motor y los modelos del doc B §4.10.7:
    /// `nemotron` (una vía), `groot` y `qwen` (las dos) y `checkpoint`
    /// (safetensors).
    fn nuevo(nombre: &str) -> Self {
        let base = std::env::temp_dir().join(format!("ropero-install-cli-{nombre}"));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("catalogo");
        let bin = base.join("bin");
        let path = base.join("path");
        let gguf = base.join("gguf");
        let safetensors = base.join("safetensors");
        for dir in [
            root.join("models").join("turbo-llama"),
            root.join("engines"),
            bin.clone(),
            path.clone(),
            gguf.clone(),
            safetensors.clone(),
            base.join("logs"),
        ] {
            std::fs::create_dir_all(&dir).expect("crear el temporal");
        }
        std::fs::write(root.join("engines").join("turbo-llama.toml"), MOTOR).expect("motor");
        let fake = Fake {
            base,
            root,
            bin,
            path,
            gguf,
            safetensors,
        };
        fake.modelo("nemotron", &modelo_solo_download("nemotron"));
        fake.modelo("groot", &modelo_ambos_vias("groot"));
        fake.modelo("qwen", &modelo_ambos_vias("qwen"));
        fake.modelo("checkpoint", &modelo_checkpoint("checkpoint"));
        fake
    }

    /// Escribe (o sobrescribe) un modelo del catálogo.
    fn modelo(&self, nombre: &str, contenido: &str) {
        std::fs::write(
            self.root
                .join("models")
                .join("turbo-llama")
                .join(format!("{nombre}.toml")),
            contenido,
        )
        .expect("escribir el modelo");
    }

    /// Pone un binario falso en el `PATH` controlado (el motor).
    fn con_binario(self, nombre: &str) -> Self {
        let destino = self.path.join(nombre);
        std::fs::write(&destino, "#!/bin/sh\nexit 0\n").expect("binario falso");
        self.con_permisos(&destino);
        self
    }

    /// Pone el `hf` falso en el `PATH` controlado.
    fn con_hf(self) -> Self {
        let destino = self.path.join("hf");
        std::fs::write(&destino, HF_FALSO).expect("hf falso");
        self.con_permisos(&destino);
        self
    }

    /// Cambia el modo del `hf` falso (`nada`, `vacio`, `ok`).
    fn modo_hf(&self, modo: &str) {
        std::fs::write(self.path.join("modo"), modo).expect("modo del hf falso");
    }

    /// Da permisos de ejecución a un fichero.
    fn con_permisos(&self, ruta: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permisos = std::fs::metadata(ruta).expect("permisos").permissions();
            permisos.set_mode(0o755);
            std::fs::set_permissions(ruta, permisos).expect("chmod");
        }
    }

    /// Corre `ropero` contra la máquina de pruebas.
    ///
    /// Sin terminal: los tests no son interactivos, y un menú que se traga
    /// un EOF acabaría eligiendo por su cuenta (R-INST-26).
    fn corre(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
            .args(args)
            .env("ROPERO_MODELS_DIR", &self.root)
            .env("ROPERO_BIN_DIR", &self.bin)
            .env("GGUF_DIR", &self.gguf)
            .env("SAFETENSORS_DIR", &self.safetensors)
            .env("LOG_DIR", self.base.join("logs"))
            .env("HOME", &self.base)
            .env("PATH", &self.path)
            .output()
            .expect("correr ropero");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// Corre `ropero` **con terminal**, para poder probar el menú.
    ///
    /// Sin terminal no hay menú (R-INST-26), así que el flujo del menú se
    /// prueba con un **pty**: `script` lo da sin dependencias (util-linux en
    /// Linux, BSD en macOS). Devuelve todo lo que salió por el pty.
    fn corre_con_tty(&self, args: &[&str], entrada: &str) -> String {
        let exe = env!("CARGO_BIN_EXE_ropero");
        let mut cmd = Command::new(script());
        #[cfg(target_os = "macos")]
        {
            cmd.arg("-q").arg("/dev/null").arg(exe).args(args);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let linea = format!("{} {}", exe, args.join(" "));
            cmd.arg("-qec").arg(linea).arg("/dev/null");
        }
        let mut hijo = cmd
            .env("ROPERO_MODELS_DIR", &self.root)
            .env("ROPERO_BIN_DIR", &self.bin)
            .env("GGUF_DIR", &self.gguf)
            .env("SAFETENSORS_DIR", &self.safetensors)
            .env("LOG_DIR", self.base.join("logs"))
            .env("HOME", &self.base)
            .env("PATH", &self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("correr ropero con terminal");
        {
            let mut tubo = hijo.stdin.take().expect("stdin");
            tubo.write_all(entrada.as_bytes())
                .expect("escribir la respuesta");
        }
        let salida = hijo.wait_with_output().expect("esperar a ropero");
        format!(
            "{}{}",
            String::from_utf8_lossy(&salida.stdout),
            String::from_utf8_lossy(&salida.stderr)
        )
    }

    /// Las invocaciones del `hf` falso: una lista de argumentos por llamada.
    fn llamadas_hf(&self) -> Vec<Vec<String>> {
        let Ok(texto) = std::fs::read_to_string(self.path.join("args.log")) else {
            return Vec::new();
        };
        let mut llamadas: Vec<Vec<String>> = Vec::new();
        for linea in texto.lines() {
            if linea == SEPARADOR {
                llamadas.push(Vec::new());
                continue;
            }
            if let Some(ultima) = llamadas.last_mut() {
                ultima.push(linea.to_string());
            }
        }
        llamadas
    }

    /// La ruta del GGUF de un modelo (R-MOD-6: `$GGUF_DIR/<modelo>/<fichero>`).
    fn gguf_de(&self, modelo: &str) -> PathBuf {
        self.gguf.join(modelo).join(format!("{modelo}.gguf"))
    }

    /// La ruta del checkpoint de un modelo (`$SAFETENSORS_DIR/<[model].path>`).
    fn checkpoint_de(&self) -> PathBuf {
        self.safetensors.join("ejemplo").join("checkpoint")
    }

    /// Los nombres de las entradas de un directorio (ordenados).
    fn entradas(&self, dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(leidas) = std::fs::read_dir(dir) {
            for entrada in leidas.flatten() {
                out.push(entrada.file_name().to_string_lossy().into_owned());
            }
        }
        out.sort();
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El menú sin terminal es un error de uso (R-INST-26, doc CLI §9)
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install engines` sin lista y sin terminal sale con 2 y el
/// mensaje dice **qué hacer**: pasar la lista.
#[test]
fn install_engines_sin_lista_y_sin_tty_dice_que_pasar_la_lista() {
    let fake = Fake::nuevo("engines-sin-tty");
    let (code, stdout, stderr) = fake.corre(&["install", "engines"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("terminal"),
        "el mensaje no dice que hace falta una terminal: {stderr}"
    );
    assert!(
        stderr.contains("ropero install engines"),
        "el mensaje no dice qué hacer: {stderr}"
    );
    assert!(
        !stdout.contains("enlace creado"),
        "no debía instalar nada: stdout={stdout}"
    );
}

/// `ropero install models` sin lista y sin terminal sale con 2 y el
/// mensaje dice **qué hacer**: pasar la lista.
#[test]
fn install_models_sin_lista_y_sin_tty_dice_que_pasar_la_lista() {
    let fake = Fake::nuevo("models-sin-tty");
    let (code, stdout, stderr) = fake.corre(&["install", "models"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("terminal"),
        "el mensaje no dice que hace falta una terminal: {stderr}"
    );
    assert!(
        stderr.contains("ropero install models"),
        "el mensaje no dice qué hacer: {stderr}"
    );
    assert!(
        !stdout.contains("descargado"),
        "no debía descargar nada: stdout={stdout}"
    );
}

/// `ropero install` a secas hace **engines y luego models** (doc CLI §9):
/// sin terminal, el primer paso (los motores) es el que falla.
#[test]
fn install_a_secas_sin_tty_falla_en_los_engines() {
    let fake = Fake::nuevo("install-a-secas");
    let (code, _stdout, stderr) = fake.corre(&["install"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("install engines"),
        "el primer paso debe ser el de los motores: {stderr}"
    );
    assert!(
        !stderr.contains("install models"),
        "no debe llegar a los modelos si los motores ya han fallado: {stderr}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La descarga de verdad (R-MOD-16, R-MOD-18, R-MOD-19)
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install models <lista>` **descarga de verdad** con el `hf`
/// falso, y el GGUF acaba en disco con contenido.
#[test]
fn install_models_descarga_el_gguf_de_verdad() {
    let fake = Fake::nuevo("descarga-gguf").con_hf();
    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download · descargado"),
        "stdout={stdout}"
    );
    let destino = fake.gguf_de("nemotron");
    assert!(destino.is_file(), "el GGUF no existe: {destino:?}");
    let contenido = std::fs::metadata(&destino).expect("metadatos");
    assert!(contenido.len() > 0, "el GGUF está vacío: {destino:?}");
}

/// El GGUF se baja con `hf download <repo> <fichero> --local-dir <staging>`,
/// y el staging vive **dentro** del directorio de destino (R-MOD-15).
#[test]
fn install_models_llama_a_hf_con_los_argumentos_exactos_del_gguf() {
    let fake = Fake::nuevo("args-gguf").con_hf();
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let llamadas = fake.llamadas_hf();
    assert_eq!(llamadas.len(), 1, "hf debía llamarse una vez: {llamadas:?}");
    let args = &llamadas[0];
    assert_eq!(
        &args[..4],
        &[
            "download",
            "ejemplo/nemotron",
            "nemotron.gguf",
            "--local-dir"
        ],
        "hf no se ha llamado con los argumentos exactos: {args:?}"
    );
    // El staging es un directorio dentro del destino (R-MOD-15).
    let destino_dir = fake.gguf.join("nemotron");
    let staging = Path::new(&args[4]);
    assert!(
        staging.starts_with(&destino_dir),
        "el staging debe vivir dentro del destino: {staging:?} no está en {destino_dir:?}"
    );
}

/// El checkpoint se baja con `hf download <repo> --local-dir <destino>` y
/// **un `--exclude` por cada excluido**: los de basura siempre y los del
/// modelo.
#[test]
fn install_models_llama_a_hf_con_los_excludes_por_defecto_del_checkpoint() {
    let fake = Fake::nuevo("args-checkpoint").con_hf();
    let (code, stdout, stderr) = fake.corre(&["install", "models", "checkpoint"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("checkpoint: vía download · descargado"),
        "stdout={stdout}"
    );
    let llamadas = fake.llamadas_hf();
    assert_eq!(llamadas.len(), 1, "hf debía llamarse una vez: {llamadas:?}");
    let args = &llamadas[0];
    let esperado = [
        "download".to_string(),
        "ejemplo/checkpoint".to_string(),
        "--local-dir".to_string(),
        fake.checkpoint_de().to_string_lossy().to_string(),
    ];
    assert_eq!(
        &args[..4],
        &esperado[..],
        "hf no se ha llamado con los argumentos exactos: {args:?}"
    );
    // Los excluses por defecto (R-MOD-14) y el del modelo.
    let excluses: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| *i > 0 && args[i - 1] == "--exclude")
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        excluses,
        vec![
            ".gitattributes",
            "*.md",
            ".DS_Store",
            "*.png",
            "*.jpg",
            "*.gif",
            "original/*"
        ],
        "los --exclude no son los esperados: {args:?}"
    );
}

/// Un modelo **ya descargado** no se vuelve a bajar: no se llama a `hf` y
/// sale con 0 diciendo «ya descargado» (R-MOD-16.1).
#[test]
fn install_models_no_vuelve_a_bajar_un_modelo_ya_descargado() {
    let fake = Fake::nuevo("ya-descargado").con_hf();
    let destino = fake.gguf_de("nemotron");
    std::fs::create_dir_all(destino.parent().expect("dir")).expect("directorio");
    std::fs::write(&destino, "ya está\n").expect("gguf");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download · ya descargado"),
        "stdout={stdout}"
    );
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.llamadas_hf()
    );
}

/// Un fichero **vacío** en el destino **no** cuenta como descargado: se
/// vuelve a bajar (R-MOD-18: `[[ -f && -s ]]`).
#[test]
fn install_models_vuelve_a_bajar_un_fichero_vacio() {
    let fake = Fake::nuevo("fichero-vacio").con_hf();
    let destino = fake.gguf_de("nemotron");
    std::fs::create_dir_all(destino.parent().expect("dir")).expect("directorio");
    std::fs::write(&destino, "").expect("gguf vacío");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download · descargado"),
        "un fichero vacío no cuenta como descargado: stdout={stdout}"
    );
    assert_eq!(
        fake.llamadas_hf().len(),
        1,
        "hf debía llamarse: {:?}",
        fake.llamadas_hf()
    );
    let contenido = std::fs::metadata(&destino).expect("metadatos");
    assert!(contenido.len() > 0, "el GGUF sigue vacío: {destino:?}");
}

/// Sin `hf` en el `PATH` el error es un **1** y el mensaje nombra `hf`;
/// además, no se pregunta nada antes (R-MOD-16.3).
#[test]
fn install_models_sin_hf_dice_que_falta_hf_y_no_pregunta() {
    let fake = Fake::nuevo("sin-hf");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stderr.contains("falta 'hf'"),
        "el mensaje debe nombrar hf: stderr={stderr}"
    );
    assert!(
        !stdout.contains("falta 'hf'"),
        "el error no va a stdout (R-MOD-11): stdout={stdout}"
    );
    assert!(
        stderr.contains("huggingface_hub"),
        "el mensaje no dice cómo se arregla: stderr={stderr}"
    );
    let todo = format!("{stdout}{stderr}");
    assert!(
        !todo.contains("¿Descargar"),
        "no debía preguntar por la descarga: {todo}"
    );
    assert!(
        !todo.contains("¿Cuál?"),
        "no debía preguntar por la vía: {todo}"
    );
}

/// `hf` sale con 0 pero **no deja el checkpoint**: es un error (R-MOD-19) y
/// el mensaje dice que el destino está incompleto.
#[test]
fn install_models_con_hf_que_no_deja_el_checkpoint_dice_que_esta_incompleto() {
    let fake = Fake::nuevo("hf-incompleto").con_hf();
    fake.modo_hf("nada");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "checkpoint"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stderr.contains("hf terminó pero"),
        "el mensaje debe decir que hf terminó pero está incompleto: stderr={stderr}"
    );
    assert!(
        stderr.contains("está incompleto"),
        "el mensaje debe decir que está incompleto: stderr={stderr}"
    );
    assert!(
        !stdout.contains("descargado"),
        "no puede decir que está descargado: stdout={stdout}"
    );
}

/// `hf` sale con 0 pero deja el GGUF **vacío**: la re-verificación lo ve y
/// es un error (R-MOD-18 y R-MOD-19).
#[test]
fn install_models_con_hf_que_deja_el_gguf_vacio_dice_que_esta_incompleto() {
    let fake = Fake::nuevo("hf-vacio").con_hf();
    fake.modo_hf("vacio");
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stderr.contains("hf terminó pero"),
        "el mensaje debe decir que hf terminó pero está incompleto: stderr={stderr}"
    );
    assert!(
        stderr.contains("está incompleto"),
        "el mensaje debe decir que está incompleto: stderr={stderr}"
    );
}

/// `hf` sale con 0 pero el GGUF **no aparece**: es un error (R-MOD-16.6).
#[test]
fn install_models_con_hf_que_no_deja_el_gguf_falla_diciendo_que_no_aparece() {
    let fake = Fake::nuevo("hf-sin-fichero").con_hf();
    fake.modo_hf("nada");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 1, "stderr={stderr}");
    // El wording es el de R-MOD-19: «hf terminó pero <destino> está
    // incompleto».
    assert!(
        stderr.contains("hf terminó pero") && stderr.contains("está incompleto"),
        "stderr={stderr}"
    );
    assert!(
        !stdout.contains("descargado"),
        "no puede decir que está descargado: stdout={stdout}"
    );
}

/// El fichero que está **con el nombre del repo** (una descarga a medio
/// renombrar) se renombra y se da por bueno, sin llamar a `hf`
/// (R-MOD-16.2).
#[test]
fn install_models_renombra_el_fichero_con_el_nombre_del_repo_sin_llamar_a_hf() {
    let fake = Fake::nuevo("renombrar").con_hf();
    let dir = fake.gguf.join("nemotron");
    std::fs::create_dir_all(&dir).expect("directorio");
    // El fichero del repo tiene **otro** nombre que el final: es la
    // descarga a medio renombrar (R-MOD-16.2).
    fake.modelo(
        "nemotron",
        r#"
[model]
name = "nemotron"
engine = "turbo-llama"

[files]
gguf = "nemotron-final.gguf"

[download]
repo = "ejemplo/nemotron"
file = "nemotron-en-el-repo.gguf"
size = "~4 GB"
"#,
    );
    std::fs::write(dir.join("nemotron-en-el-repo.gguf"), "a medias\n").expect("fichero");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download · ya descargado"),
        "stdout={stdout}"
    );
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.llamadas_hf()
    );
    let final_g = fake.gguf.join("nemotron").join("nemotron-final.gguf");
    assert!(final_g.is_file(), "no se ha renombrado: {final_g:?}");
}

/// El staging se borra al terminar la descarga (R-MOD-16.8): en el
/// directorio del modelo sólo queda el fichero final.
#[test]
fn install_models_borra_el_staging_al_terminar_la_descarga() {
    let fake = Fake::nuevo("staging").con_hf();
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(
        fake.entradas(&fake.gguf.join("nemotron")),
        vec!["nemotron.gguf".to_string()],
        "ha quedado el staging por el camino"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El contrato de `--prio` (doc B §4.10.7)
// ─────────────────────────────────────────────────────────────────────────────

/// Un `--prio` **inválido** es un error de uso (exit 2) y el mensaje
/// incluye el **valor recibido**.
#[test]
fn install_models_con_prio_invalido_dice_el_valor_recibido() {
    let fake = Fake::nuevo("prio-invalido").con_hf();
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "nemotron", "--prio=fast"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("fast"),
        "el mensaje no dice el valor recibido: {stderr}"
    );
    assert!(
        stderr.contains("download") && stderr.contains("build"),
        "el mensaje no dice los valores válidos: {stderr}"
    );
}

/// Un `--prio` **inválido** en posición **global** es el mismo error de uso.
#[test]
fn install_models_con_prio_invalido_global_dice_el_valor_recibido() {
    let fake = Fake::nuevo("prio-invalido-global").con_hf();
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "--prio=fast", "nemotron"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("fast"),
        "el mensaje no dice el valor recibido: {stderr}"
    );
}

/// Un `--prio` **ignorado** (el modelo no tiene ese camino) **no** es un
/// error y **no** aborta: los demás modelos de la lista se instalan igual.
#[test]
fn un_prio_ignorado_no_aborta_los_demas_modelos() {
    let fake = Fake::nuevo("prio-ignorado").con_hf();
    fake.modelo("alfa", &modelo_solo_download("alfa"));
    // `alfa` sólo tiene `[download]`: `--prio=build` se ignora.
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "alfa",
        "--prio=build",
        "nemotron",
        "--prio=download",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("alfa: vía download · descargado"),
        "el modelo con el --prio ignorado debe seguir instalándose: stdout={stdout}"
    );
    assert!(
        stdout.contains("nemotron: vía download · descargado"),
        "los demás modelos se instalan igual: stdout={stdout}"
    );
    assert!(
        stderr.contains("se ignora"),
        "el --prio ignorado no se ha avisado: {stderr}"
    );
}

/// Un `--prio` que **ya es el defecto** del modelo (sólo tiene un camino y
/// es ese) también se ignora, y tampoco aborta.
#[test]
fn un_prio_que_ya_es_el_defecto_se_ignora_sin_abortar() {
    let fake = Fake::nuevo("prio-defecto").con_hf();
    fake.modelo("alfa", &modelo_solo_download("alfa"));
    let (code, stdout, stderr) = fake.corre(&["install", "models", "alfa", "--prio=download"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("alfa: vía download · descargado"),
        "stdout={stdout}"
    );
    assert!(
        stderr.contains("se ignora"),
        "el --prio ignorado no se ha avisado: {stderr}"
    );
}

/// Un `--prio` pedido para un camino que el modelo **no tiene** se ignora
/// y el modelo se instala por el camino que sí tiene (aquí, `build`).
///
/// La vía `build` todavía no se fabrica (es `ropero-forge`): sale 1.
#[test]
fn un_prio_ignorado_deja_la_via_que_el_modelo_si_tiene() {
    let fake = Fake::nuevo("prio-sin-camino").con_hf();
    fake.modelo("delta", &modelo_solo_build("delta"));
    let (code, stdout, stderr) = fake.corre(&["install", "models", "delta", "--prio=download"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("delta: vía build"),
        "el modelo debe ir por el camino que sí tiene: stdout={stdout}"
    );
    assert!(
        stderr.contains("se ignora"),
        "el --prio ignorado no se ha avisado: {stderr}"
    );
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse (no hay descarga): {:?}",
        fake.llamadas_hf()
    );
}

/// Los **ejemplos canónicos** del doc B §4.10.7.
///
/// `ropero install models nemotron groot --prio=build qwen --prio=download`
/// significa exactamente:
///
/// | Modelo | `--prio` | Consecuencia |
/// |---|---|---|
/// | `nemotron` | (ninguno) | se procesa **con lo que tenga** |
/// | `groot` | `build` | se usa su camino `build` |
/// | `qwen` | `download` | se usa su camino `download` |
#[test]
fn los_ejemplos_canonicos_del_prio_eligen_la_via_de_la_tabla() {
    let fake = Fake::nuevo("canonico").con_hf();
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "nemotron",
        "groot",
        "--prio=build",
        "qwen",
        "--prio=download",
    ]);
    // La vía `build` todavía no se fabrica (es `ropero-forge`): sale 1.
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download"),
        "nemotron va con lo que tenga: stdout={stdout}"
    );
    assert!(
        stdout.contains("groot: vía build"),
        "groot va por build: stdout={stdout}"
    );
    assert!(
        stdout.contains("qwen: vía download"),
        "qwen va por download: stdout={stdout}"
    );
}

/// El **precedencia** del doc B §4.10.7: con un `--prio` global y otro
/// por modelo, **manda el por modelo**.
///
/// `ropero install models --prio=build nemotron groot --prio=download`:
/// `nemotron` va por `build` (el global) y `groot` por `download` (el suyo).
#[test]
fn el_prio_por_modelo_manda_sobre_el_global() {
    let fake = Fake::nuevo("precedencia").con_hf();
    // `nemotron` con las dos vías: el global `build` le aplica de verdad.
    fake.modelo("nemotron", &modelo_ambos_vias("nemotron"));
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "--prio=build",
        "nemotron",
        "groot",
        "--prio=download",
    ]);
    // La vía `build` todavía no se fabrica: sale 1.
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía build"),
        "nemotron va por el global: stdout={stdout}"
    );
    assert!(
        stdout.contains("groot: vía download"),
        "groot va por el suyo, que manda: stdout={stdout}"
    );
}

/// El ámbito del `--prio` **por modelo** termina en el siguiente modelo.
///
/// `ropero install models --prio=download groot --prio=build qwen`: `groot`
/// va por su `--prio=build` y `qwen` por el **global** (`download`): el
/// `--prio` de `groot` no alcanza a `qwen`.
#[test]
fn el_prio_por_modelo_no_alcanza_al_siguiente_modelo() {
    let fake = Fake::nuevo("ambito").con_hf();
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "--prio=download",
        "groot",
        "--prio=build",
        "qwen",
    ]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("groot: vía build"),
        "groot va por su --prio: stdout={stdout}"
    );
    assert!(
        stdout.contains("qwen: vía download"),
        "el --prio de groot no alcanza a qwen: stdout={stdout}"
    );
}

/// Sin `--prio` y con **las dos vías** se pregunta; sin terminal es un
/// **error de uso** (exit 2) cuyo mensaje dice **qué hacer**.
#[test]
fn install_models_con_dos_vias_y_sin_prio_sin_tty_dice_que_pasar_el_prio() {
    let fake = Fake::nuevo("pregunta-via").con_hf();
    let (code, stdout, stderr) = fake.corre(&["install", "models", "groot"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("--prio=download") && stderr.contains("--prio=build"),
        "el mensaje no dice qué hacer: {stderr}"
    );
    assert!(
        !stdout.contains("descargado"),
        "no debía descargar nada: stdout={stdout}"
    );
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.llamadas_hf()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La lista: ids o alias, con espacios o comas (doc CLI §9)
// ─────────────────────────────────────────────────────────────────────────────

/// La lista acepta el **alias** de un modelo: se instala el modelo real
/// (sus artefactos van a su directorio, no al del alias).
#[test]
fn install_models_acepta_el_alias_de_un_modelo() {
    let fake = Fake::nuevo("alias-modelo").con_hf();
    fake.modelo("alfa", &modelo_con_alias("alfa", "a"));
    let (code, stdout, stderr) = fake.corre(&["install", "models", "a"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("vía download · descargado"),
        "el alias debe instalar el modelo: stdout={stdout}"
    );
    assert!(
        fake.gguf_de("alfa").is_file(),
        "el modelo real no se ha descargado: {:?}",
        fake.entradas(&fake.gguf)
    );
}

/// La lista acepta **alias separados por comas**.
#[test]
fn install_models_acepta_una_lista_de_alias_con_comas() {
    let fake = Fake::nuevo("alias-comas").con_hf();
    fake.modelo("alfa", &modelo_con_alias("alfa", "a"));
    let (code, stdout, stderr) = fake.corre(&["install", "models", "a,qwen", "--prio=download"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("vía download · descargado"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("qwen: vía download · descargado"),
        "el segundo nombre de la lista no se ha instalado: stdout={stdout}"
    );
    assert!(
        fake.gguf_de("alfa").is_file() && fake.gguf_de("qwen").is_file(),
        "no se han descargado los dos modelos: {:?}",
        fake.entradas(&fake.gguf)
    );
}

/// La lista de motores acepta el **alias legacy** de un motor (ADR-004).
#[test]
fn install_engines_acepta_el_alias_legacy_de_un_motor() {
    let fake = Fake::nuevo("alias-motor").con_binario("llama-server");
    let (code, stdout, stderr) = fake.corre(&["install", "engines", "turbo"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("turbo-llama"),
        "el alias debe instalar el motor canónico: stdout={stdout}"
    );
    assert_eq!(
        fake.entradas(&fake.bin),
        vec!["turbo-llama-server".to_string()],
        "no se ha creado el enlace"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los motores: sólo el enlace (R-INST-1, R-INST-25)
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install engines <id>` enlaza el binario bajo el nombre canónico
/// de Turbo-Llama, sin pisar el `llama-server` oficial.
#[test]
fn install_engines_crea_el_enlace_y_no_copia_ni_descarga() {
    let fake = Fake::nuevo("engines-enlace").con_binario("llama-server");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 0, "stderr={stderr}");

    // Turbo-Llama añade su prefijo al basename para coexistir con llama-server.
    let destino = fake.bin.join("turbo-llama-server");
    let meta = std::fs::symlink_metadata(&destino).expect("metadatos del enlace");
    assert!(
        meta.file_type().is_symlink(),
        "lo que hay en el BIN_DIR no es un enlace: {destino:?}"
    );
    assert_eq!(
        std::fs::read_link(&destino).expect("leer el enlace"),
        fake.path.join("llama-server"),
        "el enlace no apunta al binario resuelto"
    );
    // No se ha copiado nada: el binario sigue donde estaba y es un fichero.
    let origen = fake.path.join("llama-server");
    assert!(origen.is_file(), "el binario ha desaparecido: {origen:?}");
    assert_eq!(
        fake.entradas(&fake.bin),
        vec!["turbo-llama-server".to_string()],
        "no debe haber nada más en el BIN_DIR"
    );
    // Y no se ha descargado nada.
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.llamadas_hf()
    );
    assert!(
        fake.entradas(&fake.gguf).is_empty(),
        "no debía descargarse ningún modelo: {:?}",
        fake.entradas(&fake.gguf)
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Las rutas del catálogo, acotadas al directorio de modelos
// ─────────────────────────────────────────────────────────────────────────────

/// Un `[download].file` que **se sale** del directorio del modelo es un
/// error del catálogo (exit 1): el mensaje dice el campo y el valor, y el
/// fichero de fuera **no se toca**.
#[test]
fn install_models_con_un_file_que_se_sale_dice_el_campo_y_el_valor() {
    let fake = Fake::nuevo("file-fuera").con_hf();
    fake.modelo(
        "nemotron",
        r#"
[model]
name = "nemotron"
engine = "turbo-llama"

[files]
gguf = "nemotron.gguf"

[download]
repo = "ejemplo/nemotron"
file = "../../secreto.txt"
size = "~4 GB"
"#,
    );
    // La «víctima»: un fichero de fuera del directorio del modelo.
    let victima = fake.base.join("secreto.txt");
    std::fs::write(&victima, "VICTIMA ORIGINAL").expect("víctima");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    let todo = format!("{stdout}{stderr}");
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        todo.contains("[download].file"),
        "el mensaje no dice el campo: {todo}"
    );
    assert!(
        todo.contains("../../secreto.txt"),
        "el mensaje no dice el valor recibido: {todo}"
    );
    assert_eq!(
        std::fs::read_to_string(&victima).expect("leer la víctima"),
        "VICTIMA ORIGINAL",
        "el fichero de fuera se ha movido o sobrescrito"
    );
    assert!(
        fake.llamadas_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.llamadas_hf()
    );
    assert!(
        !stdout.contains("descargado"),
        "no podía descargarse nada: stdout={stdout}"
    );
}

/// Un `[download.aux.*].name` que se sale **sobrescribiría** el fichero de
/// fuera: es un error del catálogo y el fichero no se toca.
#[test]
fn install_models_con_un_aux_que_se_sale_no_sobrescribe_el_fichero_de_fuera() {
    let fake = Fake::nuevo("aux-fuera").con_hf();
    fake.modelo(
        "nemotron",
        r#"
[model]
name = "nemotron"
engine = "turbo-llama"

[files]
gguf = "nemotron.gguf"

[download]
repo = "ejemplo/nemotron"
file = "nemotron.gguf"
size = "~4 GB"

[download.aux.chat_template]
repo = "ejemplo/nemotron"
file = "chat_template.jinja"
name = "../../victima.txt"
size = "9 KB"
use = "run"
"#,
    );
    let victima = fake.base.join("victima.txt");
    std::fs::write(&victima, "VICTIMA ORIGINAL").expect("víctima");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    let todo = format!("{stdout}{stderr}");
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        todo.contains("[download.aux.chat_template].name"),
        "el mensaje no dice el campo: {todo}"
    );
    assert_eq!(
        std::fs::read_to_string(&victima).expect("leer la víctima"),
        "VICTIMA ORIGINAL",
        "el fichero de fuera se ha sobrescrito"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El GGUF va donde el runtime lo busca (R-MOD-6)
// ─────────────────────────────────────────────────────────────────────────────

/// Un `[files].gguf` **con ruta** se descarga en esa ruta: el runtime lo
/// busca en `$GGUF_DIR/<modelo>/<[files].gguf>` (R-MOD-6), no en su
/// `basename`.
#[test]
fn install_models_descarga_el_gguf_en_su_subdirectorio() {
    let fake = Fake::nuevo("gguf-subdir").con_hf();
    fake.modelo(
        "nemotron",
        r#"
[model]
name = "nemotron"
engine = "turbo-llama"

[files]
gguf = "carpeta/nemotron.gguf"

[download]
repo = "ejemplo/nemotron"
file = "nemotron.gguf"
size = "~4 GB"
"#,
    );

    let (code, stdout, stderr) = fake.corre(&["install", "models", "nemotron"]);

    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("nemotron: vía download · descargado"),
        "stdout={stdout}"
    );
    let destino = fake
        .gguf
        .join("nemotron")
        .join("carpeta")
        .join("nemotron.gguf");
    assert!(
        destino.is_file(),
        "el GGUF no está donde lo busca el runtime: {destino:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El menú: elegir un modelo ES consentir su descarga (R-MOD-27)
// ─────────────────────────────────────────────────────────────────────────────

/// En el **menú**, elegir un modelo **es** consentir su descarga: no se
/// pregunta el tamaño (con su default «no») y el modelo **se descarga**.
#[test]
fn en_el_menu_elegir_el_modelo_es_consentir_la_descarga() {
    let fake = Fake::nuevo("menu-consentimiento")
        .con_hf()
        .con_binario("llama-server");

    let salida = fake.corre_con_tty(&["install", "models"], "nemotron\n");

    assert!(
        !salida.contains("¿Descargar"),
        "no debía preguntar el tamaño: {salida}"
    );
    assert!(
        salida.contains("nemotron: vía download · descargado"),
        "elegir el modelo no ha sido consentir la descarga: {salida}"
    );
    assert!(
        fake.gguf_de("nemotron").is_file(),
        "el GGUF no está: {salida}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los menores: el `--prio` de los motores y el modelo sin vías
// ─────────────────────────────────────────────────────────────────────────────

/// `--prio` en `install engines` es un **error de uso** (exit 2): el flag
/// sólo vale para `ropero install models` (doc CLI §9.3).
#[test]
fn install_engines_con_prio_es_error_de_uso() {
    let fake = Fake::nuevo("prio-engines").con_binario("llama-server");
    let (code, stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama", "--prio=build"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("--prio") && stderr.contains("install models"),
        "el mensaje no dice qué hacer: stderr={stderr}"
    );
    assert!(
        !stdout.contains("enlace creado"),
        "no debía instalar nada: stdout={stdout}"
    );
}

/// Un modelo **sin vías** es un error de uso (exit 2, R-MOD-26): su
/// artefacto se consigue a mano.
#[test]
fn install_models_con_un_modelo_sin_vias_sale_con_2() {
    let fake = Fake::nuevo("sin-vias").con_hf();
    fake.modelo(
        "vacio",
        r#"
[model]
name = "vacio"
engine = "turbo-llama"

[files]
"#,
    );
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "vacio"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("a mano"),
        "el mensaje no dice cómo se consigue el artefacto: stderr={stderr}"
    );
}
