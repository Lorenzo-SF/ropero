//! Los tests de `ropero install`: el menú, el modo desatendido, el
//! `--prio` y la instalación real de motores.
//!
//! La batería es **hermética**: el catálogo, el `$BIN_DIR` y el `PATH` son
//! temporales y controlados, así que ningún test toca la máquina de quien
//! lo ejecuta.
//!
//! ## Los fakes
//!
//! - **catálogo**: un temporal con `models/<motor>/*.toml` y
//!   `engines/<motor>.toml`.
//! - **`$BIN_DIR`**: un temporal donde se ven los enlaces.
//! - **`PATH`**: un temporal con un binario falso, para que el motor
//!   "esté instalado" sin que lo esté.
//! - **`hf`**: un script falso que **graba sus argumentos** y crea el
//!   fichero que se le pide: es lo que permite comprobar que la descarga
//!   es **de verdad** (y con qué argumentos se llama a `hf`).

use std::path::PathBuf;
use std::process::Command;

/// El manifiesto del motor de pruebas.
const MOTOR: &str = r#"
id = "turbo-llama"
label = "turbo-llama (fork)"
color = "amarillo"
description = "llama.cpp (fork privado del dueño, turboquant)"
maturity = "proven"
formats = ["gguf"]
port_min = 8200
port_max = 8299

[detect]
binaries = ["llama-server"]
paths = ["/usr/local/bin/llama-server"]
"#;

/// Un modelo con **sólo** `[download]`.
const SOLO_DOWNLOAD: &str = r#"
[model]
name = "modelo-alpha"
engine = "turbo-llama"

[files]
gguf = "modelo-alpha.gguf"

[download]
repo = "ejemplo/modelo-alpha"
file = "modelo-alpha.gguf"
size = "~4 GB"
"#;

/// Un modelo con **las dos** vías.
const DOS_VIAS: &str = r#"
[model]
name = "modelo-beta"
engine = "turbo-llama"

[files]
gguf = "modelo-beta.gguf"

[download]
repo = "ejemplo/modelo-beta"
file = "modelo-beta.gguf"

[build]
repo = "ejemplo/modelo-beta-pesos"
"#;

/// Un modelo con **sólo** `[build]`.
const SOLO_BUILD: &str = r#"
[model]
name = "modelo-gamma"
engine = "turbo-llama"

[files]
gguf = "modelo-gamma.gguf"

[build]
repo = "ejemplo/modelo-gamma-pesos"
"#;

/// El `hf` falso: graba sus argumentos y crea el fichero que se le pide.
///
/// Es el downloader de la batería: sin él no se puede comprobar que
/// `ropero install models` **descarga de verdad** (T-26).
const HF_FALSO: &str = r#"#!/bin/sh
# Sólo builtins: el PATH de la batería es un temporal y no tiene dirname
# ni mkdir (el directorio de destino lo crea ropero).
base="${0%/*}"
printf '%s\n' "$@" > "$base/args.log"

dir=""
prev=""
for a in "$@"; do
  if [ "$prev" = "--local-dir" ]; then dir="$a"; fi
  prev="$a"
done
[ -n "$dir" ] || exit 3

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

if [ -n "$file" ]; then
  printf 'contenido falso\n' > "$dir/$file"
else
  printf 'pesos\n' > "$dir/model.safetensors"
  printf '{}\n' > "$dir/config.json"
fi
exit 0
"#;

/// La máquina de pruebas: catálogo, `$BIN_DIR` y `PATH` temporales.
struct Fake {
    /// La raíz del catálogo.
    root: PathBuf,
    /// El `$BIN_DIR`.
    bin: PathBuf,
    /// El `PATH` controlado.
    path: PathBuf,
}

impl Fake {
    /// Monta la máquina de pruebas en un temporal.
    fn nuevo(nombre: &str) -> Self {
        let base = std::env::temp_dir().join(format!("ropero-install-{nombre}"));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("catalogo");
        let bin = base.join("bin");
        let path = base.join("path");
        for dir in [
            root.join("models").join("turbo-llama"),
            root.join("engines"),
            bin.clone(),
            path.clone(),
        ] {
            std::fs::create_dir_all(&dir).expect("crear el temporal");
        }
        std::fs::write(root.join("engines").join("turbo-llama.toml"), MOTOR).expect("motor");
        for (nombre, texto) in [
            ("modelo-alpha", SOLO_DOWNLOAD),
            ("modelo-beta", DOS_VIAS),
            ("modelo-gamma", SOLO_BUILD),
        ] {
            std::fs::write(
                root.join("models")
                    .join("turbo-llama")
                    .join(format!("{nombre}.toml")),
                texto,
            )
            .expect("modelo");
        }
        Fake { root, bin, path }
    }

    /// Pone un binario falso en el `PATH` controlado.
    fn con_binario(self, nombre: &str) -> Self {
        let destino = self.path.join(nombre);
        std::fs::write(&destino, "#!/bin/sh\nexit 0\n").expect("binario falso");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permisos = std::fs::metadata(&destino).expect("permisos").permissions();
            permisos.set_mode(0o755);
            std::fs::set_permissions(&destino, permisos).expect("chmod");
        }
        self
    }

    /// Pone un `hf` falso en el `PATH` controlado.
    fn con_hf(self) -> Self {
        let destino = self.path.join("hf");
        std::fs::write(&destino, HF_FALSO).expect("hf falso");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permisos = std::fs::metadata(&destino).expect("permisos").permissions();
            permisos.set_mode(0o755);
            std::fs::set_permissions(&destino, permisos).expect("chmod");
        }
        self
    }

    /// El `$HOME` de la máquina de pruebas (de ahí cuelga `$GGUF_DIR`).
    fn home(&self) -> PathBuf {
        self.root.parent().expect("HOME").to_path_buf()
    }

    /// Los argumentos con los que se llamó a `hf` (vacío si no se llamó).
    fn argumentos_hf(&self) -> Vec<String> {
        match std::fs::read_to_string(self.path.join("args.log")) {
            Ok(texto) => texto.lines().map(|l| l.to_string()).collect(),
            Err(_) => Vec::new(),
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
            .env("HOME", self.root.parent().expect("HOME"))
            .env("PATH", &self.path)
            .output()
            .expect("correr ropero");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// El contenido del `$BIN_DIR`.
    fn enlaces(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entradas) = std::fs::read_dir(&self.bin) {
            for entrada in entradas.flatten() {
                out.push(entrada.file_name().to_string_lossy().into_owned());
            }
        }
        out.sort();
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El menú sin terminal es un error de uso (R-INST-26)
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install engines` **sin lista** y **sin terminal** sale con 2 y
/// dice **qué hacer**.
#[test]
fn install_engines_sin_terminal_es_error_de_uso() {
    let fake = Fake::nuevo("engines-sin-tty");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("terminal"),
        "el mensaje no dice que hace falta una terminal: {stderr}"
    );
    assert!(
        stderr.contains("ropero install engines"),
        "el mensaje no dice qué hacer: {stderr}"
    );
}

/// `ropero install models` **sin lista** y **sin terminal** sale con 2.
#[test]
fn install_models_sin_terminal_es_error_de_uso() {
    let fake = Fake::nuevo("models-sin-tty");
    let (code, _stdout, stderr) = fake.corre(&["install", "models"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("terminal"),
        "el mensaje no dice que hace falta una terminal: {stderr}"
    );
    assert!(
        stderr.contains("ropero install models"),
        "el mensaje no dice qué hacer: {stderr}"
    );
}

/// `ropero install` a secas **sin terminal** sale con 2 (R-CLI-15).
#[test]
fn install_a_secas_sin_terminal_es_error_de_uso() {
    let fake = Fake::nuevo("install-sin-tty");
    let (code, _stdout, stderr) = fake.corre(&["install"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("terminal"), "stderr={stderr}");
}

// ─────────────────────────────────────────────────────────────────────────────
// La instalación real de motores
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install engines <id>` crea el **enlace** y sale con 0.
///
/// El enlace se llama como el `basename` del binario resuelto (R-INST-25)
/// y vive en el `$BIN_DIR` (R-INST-24).
#[test]
fn install_engines_crea_el_enlace_en_el_bin_dir() {
    let fake = Fake::nuevo("engines-enlace").con_binario("llama-server");
    let (code, stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(fake.enlaces(), vec!["llama-server".to_string()]);
    // El mensaje dice que se ha enlazado, y dice **dónde**.
    assert!(stdout.contains("enlace creado"), "stdout={stdout}");
    assert!(stdout.contains("llama-server"), "stdout={stdout}");
}

/// El motor que **ya está enlazado** se dice y **no se toca**.
#[test]
fn install_engines_no_toca_un_motor_ya_enlazado() {
    let fake = Fake::nuevo("engines-ya-enlazado").con_binario("llama-server");
    let (code, _stdout, _stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 0);
    let destino = fake.bin.join("llama-server");
    let antes = std::fs::read_link(&destino).expect("enlace");
    let (code, stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let despues = std::fs::read_link(&destino).expect("enlace");
    assert_eq!(antes, despues, "el enlace no se ha tocado");
    assert!(stdout.contains("ya está enlazado"), "stdout={stdout}");
}

/// Un motor **sin binario** es un error que dice **qué hacer**.
#[test]
fn install_engines_sin_binario_dice_que_hacer() {
    let fake = Fake::nuevo("engines-sin-binario");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stderr.contains("no encuentro el binario"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("instálalo"),
        "el mensaje no dice qué hacer: {stderr}"
    );
    assert!(fake.enlaces().is_empty(), "no se ha creado ningún enlace");
}

/// Un **fichero real** en el destino no se pisa: se avisa y se sale con 1.
#[test]
fn install_engines_no_pisa_un_fichero_real() {
    let fake = Fake::nuevo("engines-ocupado").con_binario("llama-server");
    std::fs::write(fake.bin.join("llama-server"), "soy un fichero\n").expect("fichero real");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stderr.contains("no lo toco"),
        "el mensaje debe decir que no se toca: {stderr}"
    );
    // Y el fichero sigue siendo un fichero, no un enlace.
    let contenido = std::fs::read_to_string(fake.bin.join("llama-server")).expect("fichero");
    assert_eq!(contenido, "soy un fichero\n");
}

/// La lista acepta **alias legacy** de motor (ADR-004).
#[test]
fn install_engines_acepta_alias_legacy() {
    let fake = Fake::nuevo("engines-alias").con_binario("llama-server");
    let (code, stdout, stderr) = fake.corre(&["install", "engines", "turbo"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(fake.enlaces(), vec!["llama-server".to_string()]);
    assert!(stdout.contains("turbo-llama"), "stdout={stdout}");
}

/// Un motor **desconocido** es un error de uso (código 2).
#[test]
fn install_engines_con_motor_desconocido_es_error_de_uso() {
    let fake = Fake::nuevo("engines-desconocido");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines", "inventado"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("motor desconocido"), "stderr={stderr}");
}

/// La lista acepta **espacios y comas**.
#[test]
fn install_engines_acepta_lista_con_comas() {
    let fake = Fake::nuevo("engines-comas").con_binario("llama-server");
    let (code, _stdout, stderr) = fake.corre(&["install", "engines", "turbo-llama,turbo"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(fake.enlaces(), vec!["llama-server".to_string()]);
}

/// `ropero install` sin lista no es un comando desconocido: es un error de
/// uso con lo que hace falta (una terminal).
#[test]
fn install_ya_no_esta_pendiente() {
    let fake = Fake::nuevo("install-no-pendiente");
    let (code, _stdout, stderr) = fake.corre(&["install"]);
    assert_ne!(code, 1, "install ya no está pendiente: {stderr}");
    assert!(
        !stderr.contains("todavía no está implementado"),
        "install no debe decir que está pendiente: {stderr}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El contrato de `--prio`
// ─────────────────────────────────────────────────────────────────────────────

/// Un `--prio` **inválido** es un error de uso con el valor recibido.
#[test]
fn install_models_con_prio_invalido_es_error_de_uso() {
    let fake = Fake::nuevo("models-prio-invalido");
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha", "--prio=fast"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("fast"),
        "el mensaje no dice el valor: {stderr}"
    );
}

/// Con las **dos vías** y `--prio=build`, se usa `build` (no se pregunta).
#[test]
fn install_models_con_dos_vias_y_prio_no_pregunta() {
    let fake = Fake::nuevo("models-dos-vias");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-beta", "--prio=build"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(stdout.contains("modelo-beta: vía build"), "stdout={stdout}");
    assert!(
        !stderr.contains("necesita una terminal"),
        "no debía preguntar: {stderr}"
    );
}

/// Con las **dos vías** y **sin** `--prio`, sin terminal es un error de
/// uso que dice **qué hacer**.
#[test]
fn install_models_con_dos_vias_y_sin_prio_sin_terminal_es_error_de_uso() {
    let fake = Fake::nuevo("models-pregunta");
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "modelo-beta"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(
        stderr.contains("--prio=download") && stderr.contains("--prio=build"),
        "el mensaje no dice qué hacer: {stderr}"
    );
}

/// El `--prio` **global** (antes de la lista) vale para todos los modelos.
#[test]
fn install_models_con_prio_global_lo_usa_todos() {
    let fake = Fake::nuevo("models-prio-global");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "--prio=build", "modelo-beta"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(stdout.contains("modelo-beta: vía build"), "stdout={stdout}");
}

/// El `--prio` **por modelo** manda sobre el global, y su ámbito **termina
/// en el siguiente modelo**.
#[test]
fn install_models_con_prio_por_modelo_manda_sobre_el_global() {
    let fake = Fake::nuevo("models-prio-por-modelo");
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "--prio=download",
        "modelo-alpha",
        "modelo-beta",
        "--prio=build",
        "modelo-gamma",
        "--prio=download",
    ]);
    assert_eq!(code, 1, "stderr={stderr}");
    // modelo-alpha: el global (download).
    assert!(
        stdout.contains("modelo-alpha: vía download"),
        "stdout={stdout}"
    );
    // modelo-beta: el suyo (build), por encima del global.
    assert!(stdout.contains("modelo-beta: vía build"), "stdout={stdout}");
    // modelo-gamma: el suyo (download), pero sólo tiene [build]: se ignora.
    assert!(
        stdout.contains("modelo-gamma: vía build"),
        "stdout={stdout}"
    );
    assert!(
        stderr.contains("se ignora"),
        "el --prio ignorado no se ha avisado: {stderr}"
    );
}

/// Un `--prio` ignorado **no** aborta: los demás modelos se instalan igual.
#[test]
fn un_prio_ignorado_no_aborta_los_demas() {
    let fake = Fake::nuevo("models-prio-ignorado");
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "modelo-gamma",
        "--prio=download",
        "modelo-beta",
        "--prio=download",
    ]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("modelo-gamma: vía build"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("modelo-beta: vía download"),
        "stdout={stdout}"
    );
    assert!(stderr.contains("se ignora"), "stderr={stderr}");
}

/// Un modelo **desconocido** es un error de uso.
#[test]
fn install_models_con_modelo_desconocido_es_error_de_uso() {
    let fake = Fake::nuevo("models-desconocido");
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "inventado"]);
    assert_eq!(code, 2, "stderr={stderr}");
    assert!(stderr.contains("modelo desconocido"), "stderr={stderr}");
}

/// `install models` **no dice "instalado"** cuando la descarga falla: dice
/// lo que pasó y **qué hacer**.
#[test]
fn install_models_no_dice_que_ha_instalado() {
    let fake = Fake::nuevo("models-honesto");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        !stdout.contains("instalado"),
        "no puede decir que está instalado: {stdout}"
    );
    assert!(stdout.contains("vía download"), "stdout={stdout}");
    assert!(stdout.contains("error"), "stdout={stdout}");
    assert!(
        stderr.contains("huggingface_hub"),
        "el mensaje debe decir qué falta: {stderr}"
    );
}

/// La lista de modelos acepta **comas**.
#[test]
fn install_models_acepta_lista_con_comas() {
    let fake = Fake::nuevo("models-comas");
    let (code, stdout, stderr) = fake.corre(&[
        "install",
        "models",
        "modelo-alpha,modelo-beta",
        "--prio=download",
    ]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(stdout.contains("modelo-alpha"), "stdout={stdout}");
    assert!(stdout.contains("modelo-beta"), "stdout={stdout}");
}

// ─────────────────────────────────────────────────────────────────────────────
// La descarga de verdad
// ─────────────────────────────────────────────────────────────────────────────

/// `ropero install models <modelo>` **descarga de verdad**: el GGUF acaba
/// en disco y la línea dice `descargado`.
#[test]
fn install_models_descarga_el_gguf_con_hf() {
    let fake = Fake::nuevo("models-descarga").con_hf();
    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("modelo-alpha: vía download · descargado"),
        "stdout={stdout}"
    );
    let destino = fake
        .home()
        .join("models/gguf/modelo-alpha/modelo-alpha.gguf");
    assert!(
        destino.is_file(),
        "el GGUF no se ha descargado: {destino:?}"
    );
    assert!(
        !destino.as_os_str().is_empty(),
        "el GGUF está vacío: {destino:?}"
    );
}

/// `hf` se llama con los **argumentos exactos**: `download <repo>
/// <fichero> --local-dir <staging>`.
#[test]
fn install_models_llama_a_hf_con_los_argumentos_exactos() {
    let fake = Fake::nuevo("models-args").con_hf();
    let (code, _stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let args = fake.argumentos_hf();
    assert_eq!(
        &args[..4],
        &[
            "download",
            "ejemplo/modelo-alpha",
            "modelo-alpha.gguf",
            "--local-dir"
        ],
        "hf no se ha llamado con los argumentos exactos: {args:?}"
    );
}

/// El GGUF que **ya está** no se vuelve a bajar: se dice y no se llama a
/// `hf`.
#[test]
fn install_models_no_vuelve_a_bajar_un_modelo_ya_descargado() {
    let fake = Fake::nuevo("models-ya-descargado").con_hf();
    let destino = fake
        .home()
        .join("models/gguf/modelo-alpha/modelo-alpha.gguf");
    std::fs::create_dir_all(destino.parent().expect("dir")).expect("directorio");
    std::fs::write(&destino, "ya está\n").expect("gguf");

    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha"]);

    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("modelo-alpha: vía download · ya descargado"),
        "stdout={stdout}"
    );
    assert!(
        fake.argumentos_hf().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.argumentos_hf()
    );
}

/// La vía `build` dice la verdad: todavía **no** se fabrica, así que no se
/// dice que se ha instalado nada.
#[test]
fn install_models_con_la_via_build_dice_que_no_esta_implementada() {
    let fake = Fake::nuevo("models-build").con_hf();
    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-gamma"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(
        stdout.contains("modelo-gamma: vía build"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("todavía no está implementado"),
        "el mensaje no dice que no está implementado: {stdout}"
    );
    assert!(
        !stdout.contains("instalado"),
        "no puede decir que está instalado: {stdout}"
    );
    assert!(
        stderr.contains("ropero-forge"),
        "el mensaje no dice qué hacer: {stderr}"
    );
}

/// Sin `hf` en el `PATH` el error es claro y **no se pregunta nada**.
#[test]
fn install_models_sin_hf_dice_que_hace_falta_hf() {
    let fake = Fake::nuevo("models-sin-hf");
    let (code, stdout, stderr) = fake.corre(&["install", "models", "modelo-alpha"]);
    assert_eq!(code, 1, "stderr={stderr}");
    assert!(stdout.contains("falta 'hf'"), "stdout={stdout}");
    assert!(
        stderr.contains("huggingface_hub"),
        "el mensaje no dice cómo se arregla: {stderr}"
    );
}
