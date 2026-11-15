//! La **caja negra** de `ropero gguffer`: el comando, con dobles.
//!
//! Los dobles son los del doc (T-29/T-30): un **motor falso** (un
//! directorio con `convert_hf_to_gguf.py` y `build/bin/llama-quantize`
//! que graban sus argumentos) y un **`hf` falso** (que copia el fixture
//! del repo y graba **todas** sus llamadas, las que llegan y las que no).
//!
//! Lo que se afirma, desde fuera:
//!
//! - **sin motor no se baja nada** (R-GGUF-56): el error nombra el motor
//!   que falta y la variable que lo apunta, y el log del `hf` queda
//!   **vacío**;
//! - las **reglas duras** del doc CLI v2 §11.2 (fuente única, imatrix en
//!   su forma y en pareja, `--out-principal` con sufijo `.gguf`) son
//!   errores de uso (código 2);
//! - la **tirada completa** con los dobles: el GGUF sale, con su
//!   procedencia, y los argumentos grabados son los del doc;
//! - un **fallo de la cuantización** no borra nada (R-GGUF-16) y deja el
//!   checkpoint de la fase (T-30).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Los códigos de salida del doc CLI v2 §3.
const CODIGO_OK: i32 = 0;
const CODIGO_ERROR_EJECUCION: i32 = 1;
const CODIGO_ERROR_USO: i32 = 2;

/// El conversor falso: un python mínimo que escribe un GGUF de verdad
/// (la magic `GGUF` y 2 KiB) y graba sus argumentos en `conv.log`, un
/// nivel por encima de sí (está en la raíz del motor).
///
/// Menciona `no-mtp` a propósito: el conversor que no conoce el flag no
/// lo recibe (R-GGUF-36), y para verlo pasar hay que que el conversor
/// **sepa** de él.
const CONVERT_FALSO: &str = r#"#!/usr/bin/env python3
# El conversor falso: escribe la salida con la magic GGUF.
# Este conversor entiende --no-mtp (no-mtp aparece aqui).
import os, sys

args = sys.argv[1:]
out = args[args.index("--outfile") + 1]
with open(out, "wb") as f:
    f.write(b"GGUF")
    f.write(bytes(2048))
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "..", "conv.log"), "a") as f:
    f.write(" ".join(args) + "\n")
"#;

/// El cuantizador falso: `[--imatrix IMX] <in> <out> <QUANT>`, que copia
/// la entrada al final y graba sus argumentos en `quant.log` (tres niveles
/// por encima de sí: está en `motor/build/bin`).
///
/// Con `--help` enseña los quants que **publica** (como el motor de
/// verdad); con el fichero `quant-modo` al lado, falla (para el test del
/// fallo de la cuantización).
const QUANTIZE_FALSO: &str = r#"#!/bin/sh
# El cuantizador falso: [--imatrix IMX] <in> <out> <QUANT>
base="${0%/*}"
if [ "$1" = "--help" ]; then
  echo "  0  or F32"
  echo "  1  or F16"
  echo " 15  or Q4_K : alias for Q4_K_M"
  echo " 15  or Q4_K_M"
  echo " 39  or NVFP4"
  exit 0
fi
printf '%s\n' "$@" >> "$base/../../../quant.log"
in=""; out=""; quant=""
prev=""
for a in "$@"; do
  if [ "$prev" = "--imatrix" ]; then prev=""; continue; fi
  case "$a" in --imatrix) prev="$a"; continue ;; esac
  if [ -z "$in" ]; then in="$a"; elif [ -z "$out" ]; then out="$a"; else quant="$a"; fi
  prev=""
done
modo=""
if [ -f "$base/../../../quant-modo" ]; then
  modo=$(cat "$base/../../../quant-modo")
fi
case "$modo" in
  fallo) echo "llama-quantize: fallo de prueba" >&2; exit 1 ;;
esac
cp -f "$in" "$out" || exit 1
"#;

/// El `hf` falso: `download <repo> --local-dir <dir> [<fichero>]`, que
/// copia el fixture del repo y **graba todas las llamadas** (las que
/// llegan y las que no): lo que se afirma es que no se llamó ninguna.
const HF_FALSO: &str = r#"#!/bin/sh
# El `hf` falso: download <repo> --local-dir <dir> [<fichero>]
base="${0%/*}"
fixture="$base/../hf-repos"
log="$base/../hf.log"
repo=""; dir=""; fichero=""
prev=""
n=0
for a in "$@"; do
  if [ "$prev" = "--local-dir" ]; then dir="$a"; prev=""; continue; fi
  case "$a" in download) continue ;; --local-dir) prev="$a"; continue ;; -*) continue ;;
    *) n=$((n + 1)); if [ "$n" = "1" ]; then repo="$a"; else fichero="$a"; fi ;;
  esac
done
printf 'download %s %s -> %s\n' "$repo" "$fichero" "$dir" >> "$log"
[ -n "$repo" ] && [ -n "$dir" ] || exit 2
if [ -n "$fichero" ]; then
  mkdir -p "$(dirname "$dir/$fichero")"
  cp -f "$fixture/$repo/$fichero" "$dir/$fichero" 2>/dev/null || exit 1
else
  mkdir -p "$dir"
  cp -rf "$fixture/$repo/." "$dir/" 2>/dev/null || exit 1
fi
"#;

/// Los dobles y el entorno que los apunta.
struct Dobles {
    base: PathBuf,
}

impl Dobles {
    /// Un directorio nuevo para los dobles.
    fn nuevo() -> Dobles {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CONTADOR: AtomicU32 = AtomicU32::new(0);
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("ropero-gguffer-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("crear el base");
        let dobles = Dobles { base };
        dobles.hf_falso();
        dobles.pesos();
        dobles.imatrix();
        dobles
    }

    /// El motor falso: el conversor y el cuantizador.
    fn con_motor(&self) -> &Dobles {
        let motor = self.base.join("motor");
        fs::create_dir_all(motor.join("build/bin")).expect("crear build/bin");
        fs::write(motor.join("convert_hf_to_gguf.py"), CONVERT_FALSO).expect("el conversor");
        let quantize = motor.join("build/bin/llama-quantize");
        fs::write(&quantize, QUANTIZE_FALSO).expect("el cuantizador");
        fs::set_permissions(&quantize, permisos_ejecutable()).expect("permisos");
        self
    }

    /// El `hf` falso, en `<base>/bin` (que va en el `PATH`).
    fn hf_falso(&self) {
        let bin = self.base.join("bin");
        fs::create_dir_all(&bin).expect("crear bin");
        let hf = bin.join("hf");
        fs::write(&hf, HF_FALSO).expect("escribir el hf");
        fs::set_permissions(&hf, permisos_ejecutable()).expect("permisos");
    }

    /// El fixture del repo: la metadata y el peso.
    fn repo(&self, nombre: &str) {
        let dir = self.base.join("hf-repos").join(nombre);
        fs::create_dir_all(&dir).expect("crear el repo del fixture");
        fs::write(dir.join("config.json"), "{\"model_type\": \"qwen3\"}\n")
            .expect("escribir config.json");
        fs::write(dir.join("modelo.safetensors"), vec![0u8; 64]).expect("escribir el peso");
    }

    /// Un fichero suelto del fixture (el imatrix remoto).
    fn fichero(&self, repo: &str, nombre: &str) {
        let dir = self.base.join("hf-repos").join(repo);
        fs::create_dir_all(&dir).expect("crear el repo del fixture");
        fs::write(dir.join(nombre), b"imatrix de prueba").expect("escribir el fichero");
    }

    /// Un directorio de pesos local (la fuente `--path`).
    fn pesos(&self) {
        let dir = self.base.join("pesos");
        fs::create_dir_all(&dir).expect("crear los pesos");
        fs::write(dir.join("config.json"), "{\"model_type\": \"qwen3\"}\n")
            .expect("escribir config.json");
    }

    /// La matriz de importancia local.
    fn imatrix(&self) {
        fs::write(self.base.join("imatrix.dat"), b"imatrix de prueba").expect("el imatrix");
    }

    /// El motor falso se pone en modo «fallo» (el cuantizador falla).
    fn modo_fallo(&self) {
        fs::write(self.base.join("quant-modo"), "fallo").expect("el modo");
    }

    /// La ruta de los pesos locales.
    fn pesos_dir(&self) -> String {
        self.base.join("pesos").to_string_lossy().into_owned()
    }

    /// La ruta del imatrix local.
    fn imatrix_fichero(&self) -> String {
        self.base.join("imatrix.dat").to_string_lossy().into_owned()
    }

    /// El log de un doble (vacío si no llegó a escribir).
    fn log(&self, nombre: &str) -> String {
        fs::read_to_string(self.base.join(nombre)).unwrap_or_default()
    }

    /// El comando `ropero` con el entorno de los dobles.
    fn cmd(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ropero"));
        cmd.env(
            "PATH",
            format!("{}:/usr/bin:/bin", self.base.join("bin").to_string_lossy()),
        )
        .env("HOME", self.base.join("home"))
        .env("LLAMA_DIR", self.base.join("motor"))
        .env("LLAMA_OFFICIAL_DIR", self.base.join("no-oficial"))
        .env("GGUF_DIR", self.base.join("gguf"))
        .env("GGUF_WORK_DIR", self.base.join("work"))
        .env("ROPERO_MODELS_DIR", self.base.join("modelos"));
        cmd
    }

    /// Lanza `ropero` y devuelve (código, stdout, stderr).
    fn ejecuta(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.cmd().args(args).output().expect("lanzar ropero");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

impl Drop for Dobles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Los permisos de un ejecutable (`0755`).
fn permisos_ejecutable() -> fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    fs::Permissions::from_mode(0o755)
}

/// **R-GGUF-56**: sin motor, el error nombra el motor que falta y **la
/// variable que lo apunta**, y no se baja **ni un byte**.
#[test]
fn sin_motor_no_se_baja_nada_y_el_error_dice_que_falta() {
    let dobles = Dobles::nuevo();
    dobles.repo("acme/modelo");
    let (code, _stdout, stderr) = dobles.ejecuta(&["gguffer", "--repo", "acme/modelo"]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    // El mensaje dice **qué** falta y **qué hacer**.
    assert!(stderr.contains("llama.cpp"), "no nombra el motor: {stderr}");
    assert!(
        stderr.contains("LLAMA_DIR"),
        "no nombra la variable: {stderr}"
    );
    assert!(
        stderr.contains("convert_hf_to_gguf.py"),
        "no dice qué falta: {stderr}"
    );
    // Y el `hf` no se llamó ni una vez: cero bytes.
    assert!(
        dobles.log("hf.log").trim().is_empty(),
        "cero bytes descargados: hf.log = {}",
        dobles.log("hf.log")
    );
}

/// El diagnóstico **sin motor**: enseña qué falta y sale con 1.
#[test]
fn el_diagnostico_sin_motor_enseña_que_falta() {
    let dobles = Dobles::nuevo();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--diagnostico"]);
    assert_eq!(
        code, CODIGO_ERROR_EJECUCION,
        "stdout={stdout} stderr={stderr}"
    );
    // El informe enseña dónde se buscó…
    assert!(stdout.contains("LLAMA_DIR"), "stdout={stdout}");
    assert!(stdout.contains("LLAMA_OFFICIAL_DIR"), "stdout={stdout}");
    // …y el error dice el motor y la variable.
    assert!(stderr.contains("llama.cpp"), "stderr={stderr}");
    assert!(stderr.contains("LLAMA_DIR"), "stderr={stderr}");
}

/// El diagnóstico **con motor falso**: enseña qué motor se usaría y sale
/// con 0.
#[test]
fn el_diagnostico_con_motor_falso_enseña_las_herramientas() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--diagnostico"]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("convert_hf_to_gguf.py"), "stdout={stdout}");
    assert!(stdout.contains("llama-quantize"), "stdout={stdout}");
    assert!(stdout.contains("se usaría"), "stdout={stdout}");
}

/// `--quant` es **repetible** y admite **coma** (R-GGUF-7).
///
/// Se comprueba por el lado que se ve: un quant que no existe se nombra,
/// y el nombre es el que se partió de la lista.
#[test]
fn el_quant_es_repetible_y_admite_coma() {
    let dobles = Dobles::nuevo();
    // Con coma: el segundo es el que no existe.
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "q4_k_m,inventado",
    ]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    assert!(
        stderr.contains("INVENTADO"),
        "no se partió la coma: {stderr}"
    );
    // Repetible: el segundo `--quant` también se mira.
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "q4_k_m",
        "--quant",
        "inventado",
    ]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    assert!(stderr.contains("INVENTADO"), "no se repite: {stderr}");
}

/// El imatrix **sin `--quant`** no tiene sentido: error de uso (R-GGUF-6).
#[test]
fn el_imatrix_sin_quant_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("--quant"), "no dice qué hacer: {stderr}");
}

/// El imatrix local y el remoto son **excluyentes** (R-GGUF-6).
#[test]
fn el_imatrix_con_el_imatrix_repo_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "q4_k_m",
        "--imatrix",
        &dobles.imatrix_fichero(),
        "--imatrix-repo",
        "owner/name",
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("excluyente"), "no lo dice: {stderr}");
}

/// El par del imatrix remoto va **siempre junto** (R-GGUF-6).
#[test]
fn el_imatrix_repo_sin_el_imatrix_file_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "q4_k_m",
        "--imatrix-repo",
        "owner/name",
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("pareja"), "no lo dice: {stderr}");
}

/// La fuente es **una de tres** y son excluyentes (R-GGUF-3).
#[test]
fn la_fuente_doble_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "owner/name",
        "--path",
        &dobles.pesos_dir(),
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains("excluyentes"), "no lo dice: {stderr}");
}

/// `--out-principal` exige la ruta de un fichero `.gguf` (R-GGUF-4).
#[test]
fn el_out_principal_sin_sufijo_gguf_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--out-principal",
        "salida",
    ]);
    assert_eq!(code, CODIGO_ERROR_USO, "stderr={stderr}");
    assert!(stderr.contains(".gguf"), "no lo dice: {stderr}");
}

/// La tirada completa con los dobles: descarga, convierte, cuantiza,
/// verifica, limpia y escribe la procedencia.
#[test]
fn la_tirada_completa_fabrica_el_gguf() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
        "--no-mtp",
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");

    // El GGUF está, con la magic, y con su sidecar de procedencia
    // (R-GGUF-19: sin procedencia no se publica).
    let salida = dobles.base.join("gguf/modelo-Q4_K_M.gguf");
    let contenido = fs::read(&salida).expect("el GGUF está");
    assert_eq!(&contenido[..4], b"GGUF");
    let sidecar = fs::read_to_string(salida.with_extension("gguf.fuente.json"))
        .or_else(|_| fs::read_to_string(format!("{}.fuente.json", salida.to_string_lossy())))
        .expect("el sidecar está");
    assert!(sidecar.contains("\"quant\": \"Q4_K_M\""), "dice {sidecar}");
    assert!(
        sidecar.contains("\"fuente_repo\": \"acme/modelo\""),
        "dice {sidecar}"
    );

    // Los argumentos grabados son los del doc.
    let conv = dobles.log("conv.log");
    assert!(conv.contains("--outfile"), "dice {conv}");
    assert!(conv.contains("--outtype f16"), "dice {conv}");
    assert!(conv.contains("--no-mtp"), "dice {conv}");
    let quant = dobles.log("quant.log");
    assert!(quant.contains("--imatrix"), "dice {quant}");
    assert!(quant.contains("Q4_K_M"), "dice {quant}");
    assert!(quant.contains("modelo-F16.gguf"), "dice {quant}");

    // La limpieza: el clon y el intermedio se han ido (R-GGUF-17/18).
    assert!(
        !dobles.base.join("work/acme__modelo").exists(),
        "el clon se borró"
    );
    assert!(
        !dobles.base.join("gguf/modelo-F16.gguf").exists(),
        "el intermedio se borró"
    );
}

/// **R-GGUF-18**: el F16 pedido **como salida** (junto a un quant) no se
/// borra en la limpieza: es un GGUF recién producido, no un intermedio.
#[test]
fn el_f16_pedido_como_salida_sobrevive_a_la_limpieza() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--quant",
        "F16",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    // El F16 pedido sigue ahí…
    let f16 = dobles.base.join("gguf/modelo-F16.gguf");
    assert!(f16.is_file(), "el F16 pedido se borró: es una salida");
    // …y el quant de verdad también.
    assert!(dobles.base.join("gguf/modelo-Q4_K_M.gguf").is_file());
}

/// El imatrix **remoto** (`--imatrix-repo` + `--imatrix-file`) llega al
/// cuantizador: se baja a la caché y su ruta va en el argv.
#[test]
fn el_imatrix_remoto_llega_al_cuantizador() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    dobles.fichero("owner/imatrix", "imatrix.dat");
    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix-repo",
        "owner/imatrix",
        "--imatrix-file",
        "imatrix.dat",
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    let quant = dobles.log("quant.log");
    assert!(quant.contains("--imatrix"), "dice {quant}");
    let esperado = dobles
        .base
        .join("work/acme__modelo/imatrix/owner__imatrix/imatrix.dat");
    assert!(
        quant.contains(&esperado.to_string_lossy().to_string()),
        "el cuantizador no recibió la ruta del imatrix: {quant}"
    );
}

/// **R-GGUF-16**: al fallar la cuantización **no se borra nada**: el clon
/// y el intermedio se quedan, y el error dice cómo reintentar sólo la
/// cuantización. Y el checkpoint dice por dónde se sigue (T-30).
#[test]
fn al_fallar_la_cuantizacion_se_conservan_el_clon_y_el_intermedio() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    dobles.modo_fallo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    // El error dice cómo se reintenta sólo la cuantización.
    assert!(stderr.contains("llama-quantize"), "stderr={stderr}");
    // Nada se borró.
    assert!(
        dobles.base.join("work/acme__modelo").is_dir(),
        "el clon se queda"
    );
    assert!(
        dobles.base.join("gguf/modelo-F16.gguf").is_file(),
        "el intermedio se queda"
    );
    // Y el checkpoint dice por dónde se reanuda.
    let checkpoint = dobles
        .base
        .join("work/acme__modelo/.ropero-forge.json")
        .exists();
    assert!(checkpoint, "el checkpoint está");
    let checkpoint = fs::read_to_string(dobles.base.join("work/acme__modelo/.ropero-forge.json"))
        .expect("el checkpoint se lee");
    assert!(
        checkpoint.contains("conversion"),
        "el checkpoint dice por dónde se sigue: {checkpoint}"
    );
}

/// La procedencia es **obligatoria** (R-GGUF-19): si no se puede
/// escribir, la tirada **falla**. Un GGUF sin saber cómo se hizo no se
/// publica con vergüenza.
#[test]
fn sin_la_procedencia_la_tirada_falla() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    // El sitio del sidecar es un **directorio**: la escritura falla.
    let sidecar = dobles.base.join("gguf/modelo-Q4_K_M.gguf.fuente.json");
    fs::create_dir_all(&sidecar).expect("crear el sitio del sidecar");
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_ERROR_EJECUCION, "stderr={stderr}");
    assert!(stderr.contains("procedencia"), "stderr={stderr}");
}

/// `--list-quants` enseña los quants **del motor** (los que publica) y
/// los **ya publicados** de ese modelo.
#[test]
fn la_lista_de_quants_enseña_los_del_motor_y_los_publicados() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    // Un GGUF ya hecho en el directorio de salida: está publicado.
    let salida = dobles.base.join("gguf");
    fs::create_dir_all(&salida).expect("crear el directorio de salida");
    fs::write(salida.join("modelo-Q4_K_M.gguf"), b"GGUF").expect("el gguf publicado");
    let (code, stdout, stderr) =
        dobles.ejecuta(&["gguffer", "--list-quants", "--repo", "acme/modelo"]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    // Los del motor, con su número (el `file_type` del GGUF).
    assert!(stdout.contains("Q4_K_M"), "stdout={stdout}");
    assert!(stdout.contains(" 15"), "stdout={stdout}");
    // Y los ya publicados de ese modelo.
    assert!(stdout.contains("modelo-Q4_K_M.gguf"), "stdout={stdout}");
}

/// Sin motor, la lista dice **qué hacer** y sale con 1.
#[test]
fn la_lista_de_quants_sin_motor_dice_que_hacer() {
    let dobles = Dobles::nuevo();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--list-quants"]);
    assert_eq!(
        code, CODIGO_ERROR_EJECUCION,
        "stdout={stdout} stderr={stderr}"
    );
    assert!(stderr.contains("LLAMA_DIR"), "stderr={stderr}");
    // La tabla de reserva se enseña igual: son los quants conocidos.
    assert!(stdout.contains("Q4_K_M"), "stdout={stdout}");
}

/// El diagnóstico está **exento** de la regla del imatrix sin `--quant`
/// (R-GGUF-6): preguntar por el entorno no es cuantizar.
#[test]
fn el_diagnostico_esta_exento_de_la_regla_del_imatrix() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--diagnostico",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
}
