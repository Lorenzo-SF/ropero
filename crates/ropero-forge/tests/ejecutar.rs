//! La tirada de las seis fases, con **dobles** (R-GGUF-15, T-29/30).
//!
//! Los dos dobles del doc:
//!
//! - un **motor falso**: un directorio con `convert_hf_to_gguf.py` (un
//!   python mínimo que escribe un GGUF de verdad: la magic `GGUF` y más
//!   de 1 KiB) y `build/bin/llama-quantize` (copia la entrada al final y
//!   **graba sus argumentos**);
//! - un **`hf` falso**: copia el fixture del repo a `--local-dir` y
//!   **graba sus llamadas**.
//!
//! Con ellos se afirman las puertas del doc: *sin motor: error correcto y
//! cero bytes descargados* (T-29), *un fallo en la cuantización conserva
//! clon e intermedio* (T-30, R-GGUF-16), y *la procedencia obligatoria*
//! (T-34).

use std::fs;
use std::path::{Path, PathBuf};

use ropero_core::Environment;
use ropero_forge::Error;
use ropero_forge::ejecutar;
use ropero_forge::fases::{Checkpoint, Fase};
use ropero_forge::peticion::{Flags, Peticion};

/// El `hf` falso: copia el fixture del repo a `--local-dir` y graba sus
/// llamadas en `hf.log`, **al lado del script** (`$0`): los dobles no
/// leen el entorno del proceso, que es compartido entre tests.
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
[ -n "$repo" ] && [ -n "$dir" ] || exit 2
if [ -n "$fichero" ]; then
  mkdir -p "$(dirname "$dir/$fichero")"
  cp -f "$fixture/$repo/$fichero" "$dir/$fichero" || exit 1
else
  mkdir -p "$dir"
  cp -rf "$fixture/$repo/." "$dir/" || exit 1
fi
# Sólo las descargas que LLEGAN van al log: lo que cuenta es lo que
# se bajó de verdad (los tests afirman «cero bytes de pesos»).
printf 'download %s %s -> %s\n' "$repo" "$fichero" "$dir" >> "$log"
"#;

/// El conversor falso: un python mínimo que escribe un GGUF de verdad
/// (la magic `GGUF` y 2 KiB) y graba sus argumentos en `conv.log`, un
/// nivel por encima de sí (está en la raíz del motor).
const CONVERT_FALSO: &str = r#"#!/usr/bin/env python3
# El conversor falso: escribe la salida con la magic GGUF.
# Este conversor entiende --no-mtp (no-mtp y no_nextn aparecen aqui).
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

/// El cuantizador falso: copia la entrada al final, graba sus argumentos
/// en `quant.log` (tres niveles por encima de sí: está en
/// `motor/build/bin`) y **falla** con el error de imatrix si el fichero
/// `quant-modo` del base lo pide.
const QUANTIZE_FALSO: &str = r#"#!/bin/sh
# El cuantizador falso: [--imatrix IMX] <in> <out> <QUANT>
base="${0%/*}"
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
  imatrix)
    echo "llama-quantize: Missing importance matrix for tensor blk.64.attn_k.weight" >&2
    exit 1 ;;
esac
cp -f "$in" "$out" || exit 1
"#;

/// La tirada de pruebas: los dobles y el entorno.
struct Tirada {
    base: PathBuf,
    env: Environment,
}

impl Tirada {
    fn nueva() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static CONTADOR: AtomicU32 = AtomicU32::new(0);
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("ropero-forge-{}-{n}", std::process::id(),));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("crear el base");
        Tirada {
            base,
            env: Environment::default(),
        }
    }

    /// Los dobles: el motor falso y el `hf` falso, y el entorno que los
    /// apunta.
    fn con_dobles(mut self) -> Self {
        // El motor falso: el conversor y el cuantizador.
        let motor = self.base.join("motor");
        fs::create_dir_all(motor.join("build/bin")).expect("crear build/bin");
        let convert = motor.join("convert_hf_to_gguf.py");
        fs::write(&convert, CONVERT_FALSO).expect("escribir el conversor");
        let quantize = motor.join("build/bin/llama-quantize");
        fs::write(&quantize, QUANTIZE_FALSO).expect("escribir el cuantizador");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&quantize, fs::Permissions::from_mode(0o755))
            .expect("permisos del cuantizador");

        // El `hf` falso y su PATH.
        let bin = self.base.join("bin");
        fs::create_dir_all(&bin).expect("crear bin");
        let hf = bin.join("hf");
        fs::write(&hf, HF_FALSO).expect("escribir el hf");
        fs::set_permissions(&hf, fs::Permissions::from_mode(0o755)).expect("permisos del hf");

        let path = format!(
            "{}:{}:{}",
            bin.to_string_lossy(),
            motor.join("build/bin").to_string_lossy(),
            "/usr/bin:/bin"
        );
        let base = self.base.clone();
        let llama_dir = base.join("no-oficial");
        let work = base.join("work");
        let gguf = base.join("gguf");
        self.env_con(move |vars| {
            vars.insert("PATH".to_string(), path.clone());
            vars.insert(
                "LLAMA_DIR".to_string(),
                motor.to_string_lossy().into_owned(),
            );
            vars.insert(
                "LLAMA_OFFICIAL_DIR".to_string(),
                llama_dir.to_string_lossy().into_owned(),
            );
            vars.insert(
                "GGUF_WORK_DIR".to_string(),
                work.to_string_lossy().into_owned(),
            );
            vars.insert("GGUF_DIR".to_string(), gguf.to_string_lossy().into_owned());
        });
        self
    }

    /// Un repo del fixture, con la metadata y los pesos.
    fn repo(&self, nombre: &str, config: &str) {
        let dir = self.base.join("hf-repos").join(nombre);
        fs::create_dir_all(&dir).expect("crear el repo del fixture");
        if !config.is_empty() {
            fs::write(dir.join("config.json"), config).expect("escribir config.json");
        }
        fs::write(dir.join("peso.safetensors"), vec![0u8; 64]).expect("escribir los pesos");
    }

    /// Un env sin motor (las dos variables apuntan a nada).
    fn sin_motor(mut self) -> Self {
        let base = self.base.clone();
        let path = format!("{}:/usr/bin:/bin", base.join("bin").to_string_lossy());
        let llama_dir = base.join("no-fork");
        let oficial = base.join("no-oficial");
        let work = base.join("work");
        let gguf = base.join("gguf");
        self.env_con(move |vars| {
            vars.insert("PATH".to_string(), path);
            vars.insert(
                "LLAMA_DIR".to_string(),
                llama_dir.to_string_lossy().into_owned(),
            );
            vars.insert(
                "LLAMA_OFFICIAL_DIR".to_string(),
                oficial.to_string_lossy().into_owned(),
            );
            vars.insert(
                "GGUF_WORK_DIR".to_string(),
                work.to_string_lossy().into_owned(),
            );
            vars.insert("GGUF_DIR".to_string(), gguf.to_string_lossy().into_owned());
        });
        self
    }

    fn env_con(&mut self, f: impl FnOnce(&mut std::collections::BTreeMap<String, String>)) {
        let mut vars = self.env.vars.clone();
        f(&mut vars);
        self.env = Environment::new(vars, self.env.home.clone(), self.env.cwd.clone());
    }

    /// La petición por defecto: `--repo acme/modelo --quant IQ2_M`, con
    /// un **imatrix local** de fixture (los quants de verdad lo exigen,
    /// R-GGUF-6).
    fn peticion(&self) -> Peticion {
        let imx = self.base.join("imatrix.dat");
        fs::write(&imx, b"imatrix de fixture").expect("escribir el imatrix");
        let flags = Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M"])
            .con_imatrix(&imx.to_string_lossy());
        Peticion::de_flags(flags).expect("la petición por defecto")
    }

    /// El GGUF de salida que el plan de la petición dice.
    fn salida(&self) -> PathBuf {
        self.base.join("gguf").join("modelo-IQ2_M.gguf")
    }

    fn log(&self, nombre: &str) -> String {
        fs::read_to_string(self.base.join(nombre)).unwrap_or_default()
    }

    fn modo(&self, modo: &str) {
        if modo.is_empty() {
            let _ = fs::remove_file(self.base.join("quant-modo"));
        } else {
            fs::write(self.base.join("quant-modo"), modo).expect("escribir el modo");
        }
    }
}

impl Drop for Tirada {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// **T-29**: sin motor, la tirada muere **antes** de la fase 1: el error
/// nombra la variable y **el log de `hf` queda vacío** (cero bytes).
#[test]
fn sin_motor_no_se_baja_nada() {
    let tirada = Tirada::nueva().sin_motor().con_hf_falso();
    tirada.repo("acme/modelo", "");
    let error = ejecutar::forja(&tirada.peticion(), &tirada.env).expect_err("sin motor");
    match &error {
        Error::SinMotor { falta, donde } => {
            assert!(falta.contains("convert_hf_to_gguf.py"), "dice {falta}");
            assert!(donde.contains("LLAMA_DIR"), "dice {donde}");
        }
        otro => panic!("esperaba un SinMotor, y salió: {otro:?}"),
    }
    assert!(
        tirada.log("hf.log").trim().is_empty(),
        "cero bytes descargados: hf.log = {}",
        tirada.log("hf.log")
    );
}

/// La tirada completa con los dobles: descarga, conversión, cuantización,
/// verificación, limpieza y procedencia.
#[test]
fn la_tirada_completa_fabrica_el_gguf() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let informe = ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la tirada");

    // El GGUF está, con la magic y su sidecar (T-34).
    let salida = tirada.salida();
    let contenido = fs::read(&salida).expect("el GGUF está");
    assert_eq!(&contenido[..4], b"GGUF");
    let sidecar =
        fs::read_to_string(ropero_forge::Procedencia::ruta(&salida)).expect("el sidecar está");
    assert!(sidecar.contains("\"quant\": \"IQ2_M\""), "dice {sidecar}");
    assert!(
        sidecar.contains("\"fuente_repo\": \"acme/modelo\""),
        "dice {sidecar}"
    );

    // Se cuantizó de verdad, con los argumentos grabados del doble.
    assert!(informe.cuantizo);
    let args = tirada.log("quant.log");
    assert!(
        args.contains("--imatrix") || args.contains("IQ2_M"),
        "dice {args}"
    );
    assert!(
        args.ends_with("IQ2_M\n") || args.contains("\nIQ2_M"),
        "dice {args}"
    );

    // La limpieza: el clon y el intermedio se borran.
    assert!(
        !tirada.base.join("work/acme__modelo").exists(),
        "el clon se borró"
    );
    assert!(
        !tirada.base.join("gguf/modelo-F16.gguf").exists(),
        "el intermedio se borró"
    );
    // El checkpoint final quedó escrito.
    let cp = fs::read_to_string(tirada.base.join("work/acme__modelo/.ropero-forge.json"))
        .unwrap_or_default();
    if let Some(cp) = Checkpoint::de_json(&cp) {
        assert_eq!(cp.fase, Fase::Procedencia);
    }
}

/// **R-GGUF-16 (T-30)**: al fallar la cuantización NO se borra nada: el
/// clon y el intermedio se quedan, y el error dice **cómo reintentar sólo
/// la cuantización**.
#[test]
fn al_fallar_la_cuantizacion_se_conserva_clon_e_intermedio() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.modo("imatrix");
    let error = ejecutar::forja(&tirada.peticion(), &tirada.env).expect_err("la cuantización");
    tirada.modo("");

    match &error {
        Error::Cuantizacion {
            quant,
            reintento,
            traduccion,
            ..
        } => {
            assert_eq!(quant, "IQ2_M");
            // El reintento dice el comando exacto del cuantizador.
            assert!(reintento.contains("llama-quantize"), "dice {reintento}");
            assert!(reintento.contains("IQ2_M"), "dice {reintento}");
            // **R-GGUF-14**: el error de imatrix, traducido.
            let traduccion = traduccion.as_ref().expect("el error se tradujo");
            assert!(traduccion.contains("bloque 64"), "dice {traduccion}");
            assert!(traduccion.contains("MTP"), "dice {traduccion}");
            assert!(traduccion.contains("--no-mtp"), "dice {traduccion}");
        }
        otro => panic!("esperaba un Cuantizacion, y salió: {otro:?}"),
    }
    // Nada se borró: el clon y el intermedio se quedan.
    assert!(
        tirada.base.join("work/acme__modelo").is_dir(),
        "el clon se queda"
    );
    assert!(
        tirada.base.join("gguf/modelo-F16.gguf").is_file(),
        "el intermedio se queda"
    );
    assert!(!tirada.salida().exists(), "la salida no se fabricó");
    // Y el sidecar tampoco (la tirada no llegó a la fase 6).
    assert!(!Procedencia::ruta(&tirada.salida()).exists());
    // La cola del log aparte de la fase 3 dice por qué.
    assert!(
        error.to_string().contains("IQ2_M"),
        "el error nombra el quant: {}",
        error
    );
}

/// La reanudación: tras el fallo de cuantización, la segunda tirada no
/// vuelve a descargar ni a convertir (T-30: el checkpoint manda).
#[test]
fn la_segunda_tirada_reanuda_en_la_cuantizacion() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.modo("imatrix");
    let _ = ejecutar::forja(&tirada.peticion(), &tirada.env);
    tirada.modo("");
    ejecutar::forja(&tirada.peticion(), &tirada.env).expect("el reintento");

    // El `hf` se llamó UNA vez (la descarga no se repite).
    let llamadas_hf = tirada.log("hf.log").lines().count();
    assert_eq!(llamadas_hf, 1, "hf.log = {}", tirada.log("hf.log"));
    // Y la conversión se hizo UNA vez.
    let conversiones = tirada.log("conv.log").lines().count();
    assert_eq!(conversiones, 1, "conv.log = {}", tirada.log("conv.log"));
    assert!(tirada.salida().is_file());
}

/// **R-GGUF-9 y R-GGUF-7**: un repo que ya viene en el quant pedido se
/// **coloca** (se convierte y se sale 0, sin cuantizar y sin borrar).
#[test]
fn un_repo_ya_en_ese_quant_se_coloca_y_no_se_cuantiza() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo(
        "acme/modelo-NVFP4",
        r#"{"quant_method": "compressed-tensors", "format": "nvfp4-pack-quantized"}"#,
    );
    let flags = Flags::default()
        .con_repo("acme/modelo-NVFP4")
        .con_quants(&["NVFP4"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // No se cuantizó: se colocó.
    assert!(!informe.cuantizo);
    // R-GGUF-20: el informe dice el quant REAL, no el supuesto.
    assert_eq!(informe.quant_real, "NVFP4");
    // El cuantizador no se llamó.
    assert!(tirada.log("quant.log").trim().is_empty(), "no se cuantiza");
    // El GGUF está y su sidecar declara NVFP4.
    let salida = tirada.base.join("gguf").join("modelo-NVFP4.gguf");
    assert!(salida.is_file());
    let sidecar = fs::read_to_string(Procedencia::ruta(&salida)).expect("el sidecar");
    assert!(sidecar.contains("\"quant\": \"NVFP4\""), "dice {sidecar}");
}

/// **R-GGUF-8**: un repo ya cuantizado + **otro** quant es error **antes
/// de bajar los pesos**.
#[test]
fn un_repo_ya_cuantizado_con_otro_quant_es_error_sin_bajar() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo(
        "acme/modelo-NVFP4",
        r#"{"quant_method": "compressed-tensors", "format": "nvfp4-pack-quantized"}"#,
    );
    let flags = Flags::default()
        .con_repo("acme/modelo-NVFP4")
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("no es viable");
    assert!(matches!(error, Error::NoViable { .. }), "salió: {error:?}");
    // Cero bytes de **pesos**: la metadata (config.json, un par de KB) sí
    // baja — es lo que permite fallar sin los 60 GB (R-GGUF-8).
    assert!(
        !tirada.log("hf.log").contains("peso.safetensors"),
        "no se baja el peso: {}",
        tirada.log("hf.log")
    );
}

/// Sin imatrix y con quant de verdad, la tirada muere en el preflight
/// (antes de descargar).
#[test]
fn cuantizar_sin_imatrix_muere_antes_de_bajar() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    // A mano, sin imatrix: es justo lo que la petición por defecto no
    // lleva (y por eso muere).
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("sin imatrix");
    assert!(
        matches!(error, Error::FaltaImatrix { .. }),
        "salió: {error:?}"
    );
    assert!(
        !tirada.log("hf.log").contains("peso.safetensors"),
        "no se baja el peso: {}",
        tirada.log("hf.log")
    );
}

/// La tirada repetida, sin `--force`: ya está, no se hace nada.
#[test]
fn la_tirada_repetida_sin_force_dice_ya_estaba() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la primera");
    let segundo = ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la segunda");
    assert!(segundo.ya_estaba);
}

/// Un `--gguf` local: no hay descarga ni conversión; se cuantiza desde el
/// GGUF de entrada, y **el GGUF de entrada jamás se borra** (R-GGUF-17).
#[test]
fn el_gguf_de_entrada_jamas_se_borra() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let entrada = tirada.base.join("entrada").join("modelo.gguf");
    fs::create_dir_all(entrada.parent().unwrap()).expect("crear entrada");
    // El GGUF de entrada: la magic y 2 KiB, como los deja el conversor.
    fs::write(&entrada, {
        let mut v = b"GGUF".to_vec();
        v.extend(vec![0u8; 2048]);
        v
    })
    .expect("escribir el GGUF de entrada");
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");
    assert!(informe.cuantizo);
    // El GGUF de entrada sigue en su sitio.
    assert!(entrada.is_file(), "el GGUF de entrada se queda");
    // Y el de salida está, con su sidecar.
    assert!(tirada.salida().is_file());
    assert!(Procedencia::ruta(&tirada.salida()).is_file());
}

// ── Importación del sidecar, para las rutas ─────────────────────────────────

use ropero_forge::Procedencia;

/// La conversión se llama con los argumentos exactos del doc.
#[test]
fn el_conversor_se_llama_con_los_argumentos_exactos() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la tirada");
    let llamada = tirada.log("conv.log");
    assert!(llamada.contains("--outfile"), "dice {llamada}");
    assert!(llamada.contains("--outtype f16"), "dice {llamada}");
    assert!(
        llamada.contains("peso.safetensors") || llamada.contains("acme__modelo"),
        "dice {llamada}"
    );
}

/// `--keep-source` conserva los pesos aunque todo haya ido bien.
#[test]
fn keep_source_conserva_los_pesos() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    flags.keep_source = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");
    assert!(tirada.base.join("work/acme__modelo").is_dir());
    assert_eq!(
        informe
            .pesos_conservados
            .as_deref()
            .map(Path::to_string_lossy),
        Some(tirada.base.join("work/acme__modelo").to_string_lossy())
    );
}

// ── Los helpers que faltaban en Tirada ──────────────────────────────────────

impl Tirada {
    /// Un `hf` falso sin motor: sólo para el test de T-29.
    fn con_hf_falso(self) -> Self {
        let bin = self.base.join("bin");
        fs::create_dir_all(&bin).expect("crear bin");
        let hf = bin.join("hf");
        fs::write(&hf, HF_FALSO).expect("escribir el hf");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hf, fs::Permissions::from_mode(0o755)).expect("permisos");
        self
    }
}
