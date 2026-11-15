//! La **caja negra** de la forja (F6 · `ropero gguffer`), contra el
//! contrato del doc CLI v2 §11 y del doc A (R-GGUF-3…8, 15…20, 54…57).
//!
//! Todo se afirma **desde fuera del binario**: con un **motor falso**
//! (un `convert_hf_to_gguf.py` y un `build/bin/llama-quantize` que graban
//! sus argumentos) y un **`hf` falso** (que graba todas sus llamadas).
//!
//! Lo que se cubre:
//!
//! - la precondición del motor (T-29): sin motor, exit 1, el mensaje nombra
//!   el motor y la variable, y el `hf` **no se llama**;
//! - el diagnóstico (`--diagnostico`) y la lista (`--list-quants`);
//! - las reglas duras de uso (exit 2): fuente una y sólo una, imatrix en su
//!   forma y en pareja, `--out-principal` con `.gguf`, `--intermedio` válido;
//! - `--quant` repetible y con coma (R-GGUF-7);
//! - la tirada completa: el GGUF, su procedencia y los argumentos grabados;
//! - el imatrix remoto, el fallo de cuantización (R-GGUF-16), la procedencia
//!   obligatoria (R-GGUF-19), el F16 pedido como salida (R-GGUF-18) y la
//!   reanudación.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Los códigos de salida del doc CLI v2 §3.
const CODIGO_OK: i32 = 0;
const CODIGO_ERROR: i32 = 1;
const CODIGO_USO: i32 = 2;

/// El conversor falso: escribe un GGUF con la magic `GGUF` y graba sus
/// argumentos en `conv.log`, un nivel por encima de sí (está en la raíz del
/// motor). Menciona `no-mtp` a propósito, para que el flag se le pueda pasar.
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
/// la entrada al final y graba sus argumentos en `quant.log` (al nivel del
/// base: está en `motor/build/bin`). Con `--help` enseña los quants que
/// publica; con el fichero `quant-modo` al lado, falla.
const QUANTIZE_FALSO: &str = r#"#!/bin/sh
# El cuantizador falso: [--imatrix IMX] <in> <out> <QUANT>
base="${0%/*}"
if [ "$1" = "--help" ]; then
  echo "  0  or F32"
  echo "  1  or F16"
  echo " 15  or Q4_K_M"
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
/// copia el fixture del repo y graba **todas** sus llamadas: lo que se
/// afirma es que no se llamó ninguna.
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
        let base =
            std::env::temp_dir().join(format!("ropero-gguffer-qa-{}-{n}", std::process::id()));
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

    /// Una ruta dentro del base.
    fn ruta(&self, rel: &str) -> PathBuf {
        self.base.join(rel)
    }

    /// El texto de un fichero del base (vacío si no está).
    fn texto(&self, rel: &str) -> String {
        fs::read_to_string(self.base.join(rel)).unwrap_or_default()
    }

    /// Las llamadas que el `hf` falso registró.
    fn hf_llamadas(&self) -> usize {
        self.texto("hf.log")
            .lines()
            .filter(|l| !l.is_empty())
            .count()
    }

    /// Las **descargas completas** de un repo (sin fichero: el clon entero).
    fn bajadas_completas(&self, repo: &str) -> usize {
        let prefijo = format!("download {repo}  ->");
        self.texto("hf.log")
            .lines()
            .filter(|l| l.starts_with(&prefijo))
            .count()
    }

    /// Los GGUF que hay en el directorio de salida, ordenados por nombre.
    fn salidas(&self) -> Vec<String> {
        let Ok(entradas) = fs::read_dir(self.base.join("gguf")) else {
            return Vec::new();
        };
        let mut nombres: Vec<String> = entradas
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".gguf"))
            .collect();
        nombres.sort();
        nombres
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

// ── 1 · el diagnóstico ───────────────────────────────────────────────────────

/// **T-29 / R-GGUF-56**: el diagnóstico sin motor sale con 1 y nombra el
/// motor que falta y la variable que lo apunta.
#[test]
fn el_diagnostico_sin_motor_nombra_el_motor_y_la_variable() {
    let dobles = Dobles::nuevo();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--diagnostico"]);
    assert_eq!(code, CODIGO_ERROR, "stdout={stdout} stderr={stderr}");
    // El informe enseña dónde se buscó…
    assert!(stdout.contains("$LLAMA_DIR"), "stdout={stdout}");
    assert!(stdout.contains("$LLAMA_OFFICIAL_DIR"), "stdout={stdout}");
    // …y el error dice el motor y la variable.
    assert!(stderr.contains("llama.cpp"), "stderr={stderr}");
    assert!(stderr.contains("$LLAMA_DIR"), "stderr={stderr}");
    assert!(
        stderr.contains("convert_hf_to_gguf.py"),
        "no dice qué falta: {stderr}"
    );
}

/// El diagnóstico **con motor falso**: sale 0 y dice dónde está y qué
/// herramientas ve.
#[test]
fn el_diagnostico_con_motor_falso_dice_donde_y_que_herramientas() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--diagnostico"]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    let motor = dobles.ruta("motor").to_string_lossy().into_owned();
    assert!(stdout.contains(&motor), "no dice dónde: {stdout}");
    assert!(
        stdout.contains("convert_hf_to_gguf.py"),
        "no ve el conversor: {stdout}"
    );
    assert!(
        stdout.contains("llama-quantize"),
        "no ve el cuantizador: {stdout}"
    );
    assert!(
        stdout.contains("se usaría"),
        "no dice cuál usaría: {stdout}"
    );
}

/// **T-29**: `--list-quants` sin motor sale con 1 y dice **qué hacer**.
#[test]
fn la_lista_de_quants_sin_motor_sale_1_y_dice_que_hacer() {
    let dobles = Dobles::nuevo();
    let (code, stdout, stderr) = dobles.ejecuta(&["gguffer", "--list-quants"]);
    assert_eq!(code, CODIGO_ERROR, "stdout={stdout} stderr={stderr}");
    // La tabla de reserva se enseña igual: son los quants conocidos.
    assert!(stdout.contains("Q4_K_M"), "stdout={stdout}");
    assert!(stderr.contains("llama.cpp"), "stderr={stderr}");
    assert!(stderr.contains("instala"), "no dice qué hacer: {stderr}");
    assert!(
        stderr.contains("LLAMA_DIR"),
        "no nombra la variable: {stderr}"
    );
}

// ── 2 · las reglas duras de uso (exit 2) ─────────────────────────────────────

/// **R-GGUF-7**: `--quant` es repetible y admite coma; las dos formas dan el
/// mismo resultado.
#[test]
fn el_quant_repetible_y_con_coma_dan_el_mismo_resultado() {
    let coma = Dobles::nuevo();
    coma.con_motor();
    coma.repo("acme/modelo");
    let (code, stdout, stderr) = coma.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M,Q4_0",
        "--imatrix",
        &coma.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");

    let repetido = Dobles::nuevo();
    repetido.con_motor();
    repetido.repo("acme/modelo");
    let (code, stdout, stderr) = repetido.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--quant",
        "Q4_0",
        "--imatrix",
        &repetido.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");

    assert_eq!(coma.salidas(), repetido.salidas());
    assert_eq!(
        coma.salidas(),
        vec![
            "modelo-Q4_0.gguf".to_string(),
            "modelo-Q4_K_M.gguf".to_string()
        ]
    );
}

/// **R-GGUF-6**: el imatrix sin `--quant` es error de uso.
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
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("--quant"), "no dice qué hacer: {stderr}");
}

/// **R-GGUF-6**: el imatrix local y el par remoto son excluyentes.
#[test]
fn el_imatrix_local_y_el_par_remoto_son_excluyentes() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
        "--imatrix-repo",
        "owner/name",
    ]);
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("excluyente"), "no lo dice: {stderr}");
}

/// **R-GGUF-6**: el par del imatrix remoto va siempre junto.
#[test]
fn el_imatrix_repo_sin_file_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "Q4_K_M",
        "--imatrix-repo",
        "owner/name",
    ]);
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("pareja"), "no lo dice: {stderr}");
}

/// **R-GGUF-3**: la fuente es una de tres, y son excluyentes.
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
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("excluyentes"), "no lo dice: {stderr}");
}

/// **R-GGUF-3**: sin ninguna fuente también es error de uso.
#[test]
fn la_fuente_vacia_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&["gguffer", "--quant", "Q4_K_M"]);
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("fuente"), "no lo dice: {stderr}");
}

/// **R-GGUF-4**: `--out-principal` exige la ruta de un fichero `.gguf`.
#[test]
fn el_out_principal_sin_sufijo_gguf_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "Q4_K_M",
        "--out-principal",
        "salida",
    ]);
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains(".gguf"), "no lo dice: {stderr}");
}

/// El intermedio sólo admite `f16` o `bf16`: un valor inválido es uso.
#[test]
fn el_intermedio_invalido_es_error_de_uso() {
    let dobles = Dobles::nuevo();
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--path",
        &dobles.pesos_dir(),
        "--quant",
        "Q4_K_M",
        "--intermedio",
        "q4",
    ]);
    assert_eq!(code, CODIGO_USO, "stderr={stderr}");
    assert!(stderr.contains("intermedio"), "no lo dice: {stderr}");
    assert!(
        stderr.contains("f16") && stderr.contains("bf16"),
        "no dice qué admite: {stderr}"
    );
}

// ── 3 · la precondición, antes de bajar nada (T-29) ─────────────────────────

/// **T-29 / R-GGUF-56**: sin motor, una tirada con `--repo` sale con 1 y el
/// `hf` **no se llama**: cero bytes descargados.
#[test]
fn sin_motor_no_se_llama_al_hf() {
    let dobles = Dobles::nuevo();
    dobles.repo("acme/modelo");
    let (code, _stdout, stderr) = dobles.ejecuta(&["gguffer", "--repo", "acme/modelo"]);
    assert_eq!(code, CODIGO_ERROR, "stderr={stderr}");
    assert!(stderr.contains("llama.cpp"), "stderr={stderr}");
    assert!(stderr.contains("LLAMA_DIR"), "stderr={stderr}");
    assert_eq!(
        dobles.hf_llamadas(),
        0,
        "el hf no se tiene que llamar: hf.log={}",
        dobles.texto("hf.log")
    );
}

// ── 4 · la tirada completa ──────────────────────────────────────────────────

/// La tirada completa con el motor falso: el GGUF sale, con su procedencia
/// (el quant **por nombre**) y con los argumentos grabados esperados.
#[test]
fn la_tirada_completa_deja_el_gguf_su_procedencia_y_sus_argumentos() {
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
    // El informe dice qué se hizo (R-GGUF-20).
    assert!(
        stdout.contains("cuantizado a Q4_K_M"),
        "el informe no dice qué se cuantizó: {stdout}"
    );

    // El GGUF está, con la magic…
    let contenido = fs::read(dobles.ruta("gguf/modelo-Q4_K_M.gguf")).expect("el GGUF está");
    assert_eq!(&contenido[..4], b"GGUF");
    // …y con su sidecar de procedencia (R-GGUF-19).
    let sidecar = dobles.texto("gguf/modelo-Q4_K_M.gguf.fuente.json");
    assert!(
        sidecar.contains("\"quant\": \"Q4_K_M\""),
        "el quant no va por nombre: {sidecar}"
    );
    assert!(
        sidecar.contains("\"fuente_repo\": \"acme/modelo\""),
        "no dice de dónde salió: {sidecar}"
    );

    // Los argumentos grabados son los del doc.
    let conv = dobles.texto("conv.log");
    assert!(conv.contains("--outfile"), "conv.log={conv}");
    assert!(conv.contains("--outtype f16"), "conv.log={conv}");
    assert!(conv.contains("--no-mtp"), "conv.log={conv}");
    let quant = dobles.texto("quant.log");
    assert!(quant.contains("--imatrix"), "quant.log={quant}");
    assert!(quant.contains("Q4_K_M"), "quant.log={quant}");
    assert!(quant.contains("modelo-F16.gguf"), "quant.log={quant}");
}

/// El imatrix **remoto** (`--imatrix-repo` + `--imatrix-file`) llega al
/// cuantizador: su ruta de caché va en el argv.
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
    let quant = dobles.texto("quant.log");
    assert!(quant.contains("--imatrix"), "quant.log={quant}");
    let cache = dobles
        .ruta("work/acme__modelo/imatrix/owner__imatrix/imatrix.dat")
        .to_string_lossy()
        .into_owned();
    assert!(
        quant.contains(&cache),
        "el cuantizador no recibió la ruta del imatrix: {quant}"
    );
    assert!(
        dobles.texto("hf.log").contains("owner/imatrix"),
        "el hf no bajó el imatrix: hf.log={}",
        dobles.texto("hf.log")
    );
}

/// **R-GGUF-16**: al fallar la cuantización **no se borra nada** (el clon y
/// el intermedio se quedan), el error dice cómo reintentar sólo la
/// cuantización y el checkpoint marca por dónde seguir (T-30).
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
    assert_eq!(code, CODIGO_ERROR, "stderr={stderr}");
    assert!(
        stderr.contains("reintentar sólo la cuantización"),
        "no dice cómo reintentar: {stderr}"
    );
    assert!(stderr.contains("llama-quantize"), "stderr={stderr}");
    assert!(
        dobles.ruta("work/acme__modelo").is_dir(),
        "el clon se queda"
    );
    assert!(
        dobles.ruta("gguf/modelo-F16.gguf").is_file(),
        "el intermedio se queda"
    );
    let checkpoint = dobles.texto("work/acme__modelo/.ropero-forge.json");
    assert!(
        checkpoint.contains("conversion"),
        "el checkpoint dice por dónde se sigue: {checkpoint}"
    );
}

/// **R-GGUF-19 / T-34**: la procedencia es obligatoria: si el sidecar no se
/// puede escribir, la tirada **falla** (no sale 0 sin sidecar).
#[test]
fn sin_la_procedencia_la_tirada_falla() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    // El sitio del sidecar es un **directorio**: la escritura falla.
    fs::create_dir_all(dobles.ruta("gguf/modelo-Q4_K_M.gguf.fuente.json"))
        .expect("crear el sitio del sidecar");
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_ERROR, "stderr={stderr}");
    assert!(stderr.contains("procedencia"), "stderr={stderr}");
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
    assert!(
        dobles.ruta("gguf/modelo-F16.gguf").is_file(),
        "el F16 pedido es una salida: no se borra"
    );
    assert!(dobles.ruta("gguf/modelo-Q4_K_M.gguf").is_file());
}

/// **R-GGUF-20**: el informe no miente en una tirada reanudada. Relanzar la
/// misma tirada **no repite la descarga completa** (el trabajo caro) y dice
/// «ya estaba», no «no se cuantizó».
#[test]
fn la_reanudacion_no_repite_la_descarga_completa_y_no_miente() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    dobles.repo("acme/modelo");
    let (code, _stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stderr={stderr}");
    let bajadas = dobles.bajadas_completas("acme/modelo");
    assert_eq!(bajadas, 1, "hf.log={}", dobles.texto("hf.log"));

    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--repo",
        "acme/modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    assert_eq!(
        dobles.bajadas_completas("acme/modelo"),
        bajadas,
        "la descarga completa no se repite: hf.log={}",
        dobles.texto("hf.log")
    );
    assert!(stdout.contains("ya estaba"), "stdout={stdout}");
    assert!(
        !stdout.contains("no se cuantizó"),
        "el informe miente: {stdout}"
    );
}

// ── 5 · la cuantización no es obligatoria (doc CLI v2 §11.1) ────────────────

/// **Doc CLI v2 §11.1**: no todos los GGUFs requieren cuantizar. Un F16
/// pedido sobre pesos en claro es conversión de dtype: no pide imatrix y el
/// informe no afirma una cuantización que no hubo.
#[test]
fn el_f16_no_es_cuantizar_y_no_pide_imatrix() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    let (code, stdout, stderr) =
        dobles.ejecuta(&["gguffer", "--path", &dobles.pesos_dir(), "--quant", "F16"]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    assert!(
        dobles.ruta("gguf/pesos-F16.gguf").is_file(),
        "el F16 pedido está"
    );
    assert!(
        !stdout.contains("cuantizado a"),
        "el F16 no es cuantización: {stdout}"
    );
}

/// **R-GGUF-3**: la tercera fuente, `--gguf`, se cuantiza directamente (sin
/// conversor): el GGUF de entrada no se toca.
#[test]
fn la_fuente_gguf_se_cuantiza_directamente() {
    let dobles = Dobles::nuevo();
    dobles.con_motor();
    let entrada = dobles.ruta("entrada/modelo-F16.gguf");
    fs::create_dir_all(entrada.parent().expect("el directorio")).expect("crear la entrada");
    let mut datos = b"GGUF".to_vec();
    datos.extend(std::iter::repeat_n(0u8, 2048));
    fs::write(&entrada, datos).expect("escribir el GGUF de entrada");
    let entrada = entrada.to_string_lossy().into_owned();
    let (code, stdout, stderr) = dobles.ejecuta(&[
        "gguffer",
        "--gguf",
        &entrada,
        "--out",
        "modelo",
        "--quant",
        "Q4_K_M",
        "--imatrix",
        &dobles.imatrix_fichero(),
    ]);
    assert_eq!(code, CODIGO_OK, "stdout={stdout} stderr={stderr}");
    assert!(dobles.ruta("gguf/modelo.gguf").is_file(), "el quant está");
    let sidecar = dobles.texto("gguf/modelo.gguf.fuente.json");
    assert!(
        sidecar.contains("\"quant\": \"Q4_K_M\""),
        "el quant no va por nombre: {sidecar}"
    );
    // El conversor no intervino: la fuente ya era un GGUF.
    assert!(
        !dobles.ruta("conv.log").exists(),
        "el conversor no tiene que intervenir"
    );
}
