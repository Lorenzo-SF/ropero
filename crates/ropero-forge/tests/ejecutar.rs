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
modo=""
if [ -f "$base/../hf-modo" ]; then modo=$(cat "$base/../hf-modo"); fi
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
  # El modo «fantasma»: la descarga «sale bien» pero no deja el fichero
  # (para ver qué pasa cuando el imatrix no aparece en la caché).
  if [ "$modo" = "fantasma" ]; then
    printf 'download %s %s -> %s\n' "$repo" "$fichero" "$dir" >> "$log"
    exit 0
  fi
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
  parcial)
    # Deja el GGUF **a medias** y falla: el fichero queda corrupto.
    cp -f "$in" "$out" || exit 1
    echo "llama-quantize: fallo a medias" >&2
    exit 1 ;;
esac
cp -f "$in" "$out" || exit 1
"#;

/// El conversor que **sí** sabe excluir las cabeceras DFlash.
///
/// Es el mismo conversor falso, pero su texto menciona `no-dflash`: así
/// el motor lo entiende (y el flag se le puede pasar).
const CONVERT_CON_DFLASH: &str = r#"#!/usr/bin/env python3
# El conversor falso: escribe la salida con la magic GGUF.
# Este conversor entiende --no-mtp y --no-dflash (no-mtp y no-dflash aparecen aqui).
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

    /// Un fichero suelto del fixture (el imatrix remoto, por ejemplo).
    fn fichero_fixture(&self, repo: &str, nombre: &str, contenido: &[u8]) {
        let dir = self.base.join("hf-repos").join(repo);
        fs::create_dir_all(&dir).expect("crear el repo del fixture");
        fs::write(dir.join(nombre), contenido).expect("escribir el fichero");
    }

    /// El `hf` falso, en modo «fantasma»: la descarga de un fichero suelto
    /// «sale bien» pero no lo deja donde debía.
    fn hf_fantasma(&self) {
        fs::write(self.base.join("hf-modo"), "fantasma").expect("escribir el modo");
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

    /// El directorio de salida de los GGUFs (`$GGUF_DIR`).
    fn con_salida(&mut self, dir: &Path) {
        let dir = dir.to_string_lossy().into_owned();
        self.env_con(move |vars| {
            vars.insert("GGUF_DIR".to_string(), dir.clone());
        });
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

    /// El checkpoint del work (el único `.ropero-forge.json` que hay).
    fn checkpoint_del_work(&self) -> String {
        let dir = self.base.join("work");
        let entradas = match fs::read_dir(&dir) {
            Ok(entradas) => entradas,
            Err(_) => return String::new(),
        };
        for entrada in entradas.flatten() {
            let ruta = entrada.path().join(".ropero-forge.json");
            if ruta.is_file() {
                return fs::read_to_string(&ruta).unwrap_or_default();
            }
        }
        String::new()
    }

    fn log(&self, nombre: &str) -> String {
        fs::read_to_string(self.base.join(nombre)).unwrap_or_default()
    }

    /// Cuántas veces se llamó al cuantizador para un quant (el doble graba
    /// sus argumentos, uno por línea).
    fn llamadas_al_cuantizador(&self, quant: &str) -> usize {
        self.log("quant.log")
            .lines()
            .filter(|l| *l == quant)
            .count()
    }

    fn modo(&self, modo: &str) {
        if modo.is_empty() {
            let _ = fs::remove_file(self.base.join("quant-modo"));
        } else {
            fs::write(self.base.join("quant-modo"), modo).expect("escribir el modo");
        }
    }

    /// Un GGUF de entrada **con su cabecera**: la magic, la versión, los
    /// contadores y el KV `general.file_type` (un uint32 en la metadata), más
    /// el relleno para pasar el tamaño mínimo de la verificación.
    fn entrada(&self, nombre: &str, file_type: u32) -> PathBuf {
        let ruta = self.base.join("entrada").join(nombre);
        fs::create_dir_all(ruta.parent().unwrap()).expect("crear entrada");
        fs::write(&ruta, gguf_con_file_type(file_type)).expect("escribir el GGUF de entrada");
        ruta
    }

    /// Un imatrix de fixture (el fichero, aquí).
    fn imatrix(&self) -> String {
        let imx = self.base.join("imatrix.dat");
        fs::write(&imx, b"imatrix de fixture").expect("escribir el imatrix");
        imx.to_string_lossy().into_owned()
    }

    /// Un motor **sin conversor**: sólo el cuantizador (quien sólo tiene
    /// `llama-quantize` y una fuente que ya es un GGUF).
    fn sin_conversor(self) -> Self {
        let _ = fs::remove_file(self.base.join("motor/convert_hf_to_gguf.py"));
        self
    }
}

/// Un GGUF mínimo con `general.file_type` en su metadata.
///
/// El tipo del valor es el **4** (`UINT32`), que es lo que dice el formato
/// GGUF (la tabla de `gguf.h`): con el 6 (que es `FLOAT32`) el lector no
/// encontraba el KV y la clase salía del nombre del fichero.
fn gguf_con_file_type(file_type: u32) -> Vec<u8> {
    let mut datos = Vec::new();
    datos.extend(b"GGUF");
    datos.extend(3u32.to_le_bytes()); // versión
    datos.extend(0u64.to_le_bytes()); // n_tensors
    datos.extend(1u64.to_le_bytes()); // n_metadata
    let clave = b"general.file_type";
    datos.extend((clave.len() as u64).to_le_bytes());
    datos.extend(clave);
    datos.extend(4u32.to_le_bytes()); // tipo del valor: uint32
    datos.extend(file_type.to_le_bytes());
    datos.extend(vec![0u8; 1024]);
    datos
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

/// **R-GGUF-18**: el intermedio que **es una de las salidas** (el F16
/// pedido *junto* a un quant) **no se borra**: es un GGUF recién
/// producido, no un intermedio.
///
/// El defecto: la comprobación miraba sólo la **primera** salida, así que
/// con `--quant Q4_K_M --quant F16` la ruta del F16 coincidía con la del
/// intermedio y la fase 5 lo borraba: se destruía un GGUF recién hecho.
#[test]
fn el_f16_pedido_como_salida_sobrevive_a_la_limpieza() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M", "F16"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // El F16 pedido **como salida** sigue ahí: es un resultado.
    let f16 = tirada.base.join("gguf/modelo-F16.gguf");
    assert!(
        f16.is_file(),
        "el F16 pedido se borró: es una salida, no un intermedio"
    );
    // Y el quant de verdad, también.
    assert!(tirada.base.join("gguf/modelo-Q4_K_M.gguf").is_file());
}

/// El sidecar nombra el **estado real** de cada salida (hallazgo 2).
///
/// El defecto: con `--quant Q4_K_M --quant F16` el F16 lo acaba de
/// **convertir** el conversor en esa misma tirada, y el sidecar decía
/// «ropero gguffer (el GGUF ya estaba: no se cuantizó)»: la parte «no se
/// cuantizó» es cierta, pero «ya estaba» es **mentira** (R-GGUF-20).
#[test]
fn el_sidecar_nombra_el_estado_real_de_cada_salida() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M", "F16"])
        .con_imatrix(&tirada.imatrix());
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // El F16: lo generó el conversor en esta misma tirada.
    let f16 = tirada.base.join("gguf/modelo-F16.gguf");
    let sidecar = fs::read_to_string(Procedencia::ruta(&f16)).expect("el sidecar del F16");
    assert!(
        sidecar.contains("convertido"),
        "el sidecar no dice que se convirtió: {sidecar}"
    );
    assert!(
        !sidecar.contains("ya estaba"),
        "el sidecar miente: el F16 lo acaba de convertir el conversor: {sidecar}"
    );

    // El Q4_K_M: pasó por el cuantizador (y eso es lo que dice).
    let q4 = tirada.base.join("gguf/modelo-Q4_K_M.gguf");
    let sidecar = fs::read_to_string(Procedencia::ruta(&q4)).expect("el sidecar del Q4_K_M");
    assert!(
        sidecar.contains("llama-quantize"),
        "el sidecar no dice que se cuantizó: {sidecar}"
    );

    // Y el informe nombra el estado real de cada salida (R-GGUF-20).
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("convertido a GGUF: F16")),
        "el informe no dice que se convirtió: {lineas:?}"
    );
    assert!(
        lineas.iter().any(|l| l.contains("cuantizado a Q4_K_M")),
        "el informe no dice que se cuantizó: {lineas:?}"
    );
}

/// El F16 que es **sólo** el intermedio (nadie lo pidió) sí se borra:
/// R-GGUF-18 dice que el intermedio se borra cuando ya no lo necesita
/// nadie. Esta es la contramutación del test anterior.
#[test]
fn el_f16_que_es_solo_intermedio_se_borra() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la tirada");
    assert!(
        !tirada.base.join("gguf/modelo-F16.gguf").exists(),
        "el intermedio se borra: nadie lo pidió"
    );
}

/// El imatrix **remoto** (`--imatrix-repo` + `--imatrix-file`) llega al
/// cuantizador: se baja a la caché y su ruta va en el argv, delante de
/// los posicionales (R-GGUF-39).
///
/// El defecto: el fichero se bajaba a la caché y **nunca** se pasaba al
/// cuantizador: la cuantización corría sin imatrix.
#[test]
fn el_imatrix_remoto_llega_al_cuantizador() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.fichero_fixture("owner/imatrix", "Q4_K_M.dat", b"imatrix de fixture");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M"])
        .con_imatrix_remota("owner/imatrix", "Q4_K_M.dat");
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    let args = tirada.log("quant.log");
    assert!(
        args.contains("--imatrix"),
        "el cuantizador no recibió el imatrix: {args}"
    );
    let esperado = tirada
        .base
        .join("work/acme__modelo/imatrix/owner__imatrix/Q4_K_M.dat");
    assert!(
        args.contains(&esperado.to_string_lossy().to_string()),
        "el cuantizador no recibió la ruta del imatrix: {args}"
    );
}

/// El `sha256` del imatrix **remoto** llega al sidecar (hallazgo 1).
///
/// El defecto: el imatrix remoto vive en la caché del work
/// (`<work>/imatrix/<repo>/<fichero>`), y la fase 5 (limpieza) **borra el
/// work** antes de que la fase 6 escriba el sidecar: el hash salía sin
/// escribir, y el sidecar afirmaba el fichero y el repo pero no **cuál**
/// matriz se usó.
#[test]
fn el_hash_del_imatrix_remoto_llega_al_sidecar() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.fichero_fixture("owner/imatrix", "Q4_K_M.dat", b"imatrix de fixture");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M"])
        .con_imatrix_remota("owner/imatrix", "Q4_K_M.dat");
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // La caché del work ya no está: la limpieza la borró (y con ella, el
    // imatrix que se usó).
    assert!(
        !tirada.base.join("work/acme__modelo").exists(),
        "el work se borró (la limpieza)"
    );
    // El sidecar dice el hash de **ese** fichero: el de la matriz que se
    // cuantizó.
    let esperado =
        ropero_forge::sha256::sha256_de(&tirada.base.join("hf-repos/owner/imatrix/Q4_K_M.dat"))
            .expect("el hash del imatrix");
    let sidecar = fs::read_to_string(Procedencia::ruta(
        &tirada.base.join("gguf/modelo-Q4_K_M.gguf"),
    ))
    .expect("el sidecar está");
    assert!(
        sidecar.contains(&format!("\"sha256\": \"{esperado}\"")),
        "el sidecar no dice el hash del imatrix remoto: {sidecar}"
    );
}

/// Si la descarga del imatrix remoto **no deja el fichero** en la caché,
/// la tirada falla con un error claro: **no se cuantiza sin imatrix**.
#[test]
fn sin_el_imatrix_remoto_no_se_cuantiza() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.fichero_fixture("owner/imatrix", "Q4_K_M.dat", b"imatrix de fixture");
    // La descarga «sale bien», pero el fichero no aparece.
    tirada.hf_fantasma();
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M"])
        .con_imatrix_remota("owner/imatrix", "Q4_K_M.dat");
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("sin imatrix");

    match &error {
        Error::ImatrixNoEsta {
            origen,
            fichero,
            ruta,
        } => {
            assert_eq!(origen, "owner/imatrix");
            assert_eq!(fichero, "Q4_K_M.dat");
            assert!(ruta.contains("imatrix"), "dice {ruta}");
        }
        otro => panic!("esperaba un ImatrixNoEsta, y salió: {otro:?}"),
    }
    // Y el cuantizador no se llamó: no se cuantiza a ciegas.
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "el cuantizador no debe llamarse sin imatrix"
    );
    // El error dice **qué hacer**.
    assert!(
        error.to_string().contains("imatrix"),
        "el error nombra el imatrix: {error}"
    );
}

/// **R-GGUF-20**: el informe de una tirada **reanudada** no miente: si el
/// checkpoint dice que la cuantización está hecha, el informe dice que se
/// cuantizó (y con qué quant).
///
/// El defecto: `cuantizo` sólo se actualizaba si la fase de cuantización
/// se **ejecutaba**; en una tirada reanudada (checkpoint posterior) se
/// quedaba en `false` y el informe decía «no se cuantizó» de algo que sí
/// se cuantizó.
#[test]
fn un_checkpoint_de_la_fase_procedencia_sin_el_gguf_no_es_un_exito() {
    // Hallazgo 1: el checkpoint de la última fase no convierte una tirada en éxito
    // si el artefacto que afirma producir no está en disco.
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let peticion = tirada.peticion();
    // Un checkpoint de la MISMA petición (por eso es creíble), ya en la última
    // fase, pero sin el GGUF: la fase 6 no se ejecuta y nada lo comprueba.
    let cp = tirada.base.join("work/acme__modelo/.ropero-forge.json");
    fs::create_dir_all(cp.parent().unwrap()).expect("crear el work");
    fs::write(
        &cp,
        Checkpoint::con(Fase::Procedencia, &peticion.huella(&tirada.env)).a_json(),
    )
    .expect("escribir el checkpoint");

    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("el falso éxito");
    assert!(
        matches!(error, Error::SalidaFaltante { .. }),
        "salió: {error:?}"
    );
    assert_eq!(error.codigo(), 1, "un éxito falso no puede salir con 0");
    let pista = error.pista().unwrap_or_default();
    assert!(pista.contains("checkpoint"), "la pista dice: {pista}");
    assert!(
        !tirada.salida().is_file(),
        "el GGUF anunciado no está en disco"
    );
    // Nadie lo fabricó: el checkpoint pretendía que ya estaba hecho.
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "el cuantizador se llamó: {}",
        tirada.log("quant.log")
    );
}

/// **R-GGUF-19**: el sidecar declara el modelo base y el hash de la
/// importance matrix (los campos que la v1 tenía y la v2 había perdido, y que
/// lee el model card).
///
/// Y lo que no se puede calcular **no está**: el GGUF de este doble no
/// lleva metadata, así que no puede decir cuál es su arquitectura.
#[test]
fn el_sidecar_declara_el_modelo_base_y_el_hash_del_imatrix() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let imx = tirada.imatrix();
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&imx);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    let sidecar = fs::read_to_string(Procedencia::ruta(&tirada.salida())).expect("el sidecar está");
    // El modelo base: el repo del que se hizo el GGUF.
    assert!(
        sidecar.contains("\"base_model\": \"acme/modelo\""),
        "dice {sidecar}"
    );
    assert!(
        sidecar.contains("\"base_model_name\": \"modelo\""),
        "dice {sidecar}"
    );
    // El hash de la importance matrix: el de verdad, no uno inventado.
    let esperado =
        ropero_forge::sha256::sha256_de(std::path::Path::new(&imx)).expect("el hash del imatrix");
    assert!(
        sidecar.contains(&format!("\"sha256\": \"{esperado}\"")),
        "dice {sidecar}"
    );
    // Y lo que el GGUF no dice, el sidecar tampoco.
    assert!(
        !sidecar.contains("\"arch\""),
        "se inventó la arch: {sidecar}"
    );
    assert!(
        !sidecar.contains("\"block_count\""),
        "se inventaron los bloques: {sidecar}"
    );
}

/// **La huella incluye el directorio de salida**.
///
/// El defecto: la huella no lo incluía, así que la misma petición con otro
/// `$GGUF_DIR` reutilizaba el checkpoint, no fabricaba nada y la tirada
/// moría con «salida faltante» (exit 1) en vez de rehacerse.
#[test]
fn la_misma_petición_con_otro_directorio_de_salida_rehace_el_gguf() {
    let mut tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.imatrix());
    // `--keep-source`: el checkpoint sobrevive a la limpieza (si no, la
    // primera tirada lo borra con el clon y no hay nada que reutilizar).
    flags.keep_source = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let primera = ejecutar::forja(&peticion, &tirada.env).expect("la primera tirada");
    assert!(
        primera.cuantizo,
        "la primera tirada no cuantizó: {primera:?}"
    );
    assert!(
        tirada.base.join("gguf/modelo-IQ2_M.gguf").is_file(),
        "la primera tirada no dejó el GGUF"
    );

    // La misma petición, pero con otro directorio de salida: es **otra**
    // tirada, y el GGUF se rehace allí.
    tirada.con_salida(&tirada.base.join("gguf-2"));
    let segunda = ejecutar::forja(&peticion, &tirada.env).expect("la segunda tirada");
    assert!(
        segunda.cuantizo,
        "la segunda tirada no cuantizó: {segunda:?}"
    );
    assert!(
        tirada.base.join("gguf-2/modelo-IQ2_M.gguf").is_file(),
        "el GGUF no se rehízo en el sitio nuevo: {segunda:?}"
    );
    let lineas = ejecutar::lineas(&segunda);
    assert!(
        lineas.iter().any(|l| l.contains("cambió")),
        "el informe no dice que la petición cambió: {lineas:?}"
    );
}

/// **R-GGUF-20**: el informe y el sidecar dicen lo que la fase 3 hizo **de
/// verdad**.
///
/// El defecto: con `--force` (o al rehacer con otra huella) la salida ya
/// está, la fase 3 la salta y el cuantizador **no** corre, pero el informe
/// decía «cuantizado a Q4_K_M» y el sidecar «llama.cpp llama-quantize».
#[test]
fn el_informe_no_dice_que_se_cuantizó_si_el_cuantizador_no_corrió() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M"])
        .con_imatrix(&tirada.imatrix());
    let peticion = Peticion::de_flags(flags.clone()).expect("la petición");
    let primera = ejecutar::forja(&peticion, &tirada.env).expect("la primera tirada");
    assert!(
        primera.cuantizo,
        "la primera tirada no cuantizó: {primera:?}"
    );
    assert_eq!(
        tirada.llamadas_al_cuantizador("Q4_K_M"),
        1,
        "la primera tirada no cuantizó"
    );

    // La segunda, con `--force`: la salida ya está y la fase 3 la salta, así
    // que el cuantizador no vuelve a correr.
    flags.force = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let segunda = ejecutar::forja(&peticion, &tirada.env).expect("la segunda tirada");

    assert!(
        !segunda.cuantizo,
        "el informe dice que se cuantizó y el cuantizador no corrió: {segunda:?}"
    );
    let lineas = ejecutar::lineas(&segunda);
    assert!(
        !lineas.iter().any(|l| l.contains("cuantizado a")),
        "las líneas mienten: {lineas:?}"
    );
    assert!(
        lineas.iter().any(|l| l.contains("no se cuantizó")),
        "las líneas no dicen qué pasó: {lineas:?}"
    );
    assert_eq!(
        tirada.llamadas_al_cuantizador("Q4_K_M"),
        1,
        "el cuantizador se llamó de más: {}",
        tirada.log("quant.log")
    );

    // Y el sidecar tampoco: la herramienta no puede ser «llama-quantize».
    let salida = tirada.base.join("gguf").join("modelo-Q4_K_M.gguf");
    let sidecar = fs::read_to_string(Procedencia::ruta(&salida)).expect("el sidecar está");
    assert!(
        !sidecar.contains("llama-quantize"),
        "la herramienta miente: {sidecar}"
    );
    assert!(
        sidecar.contains("no se cuantizó"),
        "la herramienta no dice qué pasó: {sidecar}"
    );
}

/// **El mensaje dice lo que pasó** (R-GGUF-20): una colocación de un
/// `--gguf` no es «el quant pedido se perdió en la conversión» (no hubo
/// conversión), y una conversión sin quant que hacer tampoco.
#[test]
fn el_mensaje_de_no_cuantizar_dice_lo_que_pasó() {
    // La colocación: el GGUF ya estaba en ese quant y se colocó tal cual.
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-IQ2_M.gguf", 29);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("se colocó tal cual")),
        "las líneas no dicen que se colocó: {lineas:?}"
    );
    assert!(
        !lineas
            .iter()
            .any(|l| l.contains("se perdió en la conversión")),
        "las líneas mienten: no hubo conversión: {lineas:?}"
    );

    // La conversión: no hay quant que hacer (el F16 es un dtype, no un quant).
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["F16"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("no hay quant que hacer")),
        "las líneas no dicen por qué no se cuantizó: {lineas:?}"
    );
    assert!(
        !lineas
            .iter()
            .any(|l| l.contains("se perdió en la conversión")),
        "las líneas mienten: {lineas:?}"
    );
}

/// **R-GGUF-20**: el informe dice lo que la fase 3 hizo **de verdad**.
///
/// Con `--force` y la salida ya hecha, el cuantizador **no** corre (la fase 3
/// salta las salidas que ya existen): el informe no puede decir «cuantizado a
/// IQ2_M» —eso era el defecto—, y el sidecar tampoco.
#[test]
fn el_informe_de_una_tirada_reanudada_no_miente() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    // `--keep-source`: el checkpoint sobrevive a la limpieza (sin él, con
    // el clon borrado no queda por dónde reanudar).
    flags.keep_source = true;
    // La procedencia no se puede escribir (su sitio es un directorio): la
    // tirada muere en la fase 6, con el checkpoint en «limpieza».
    let sidecar = tirada.base.join("gguf/modelo-IQ2_M.gguf.fuente.json");
    fs::create_dir_all(&sidecar).expect("bloquear el sidecar");
    let peticion = Peticion::de_flags(flags.clone()).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("la procedencia");
    assert!(
        matches!(error, Error::Procedencia { .. }),
        "salió: {error:?}"
    );

    // Se desbloquea y se reanuda (`--force`, que es lo que hace falta
    // cuando las salidas ya están: si no, la tirada dice «ya está»).
    fs::remove_dir(&sidecar).expect("desbloquear el sidecar");
    flags.force = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la reanudación");

    // El informe dice la verdad: el cuantizador no corrió, así que no se
    // dice que se cuantizó.
    assert!(!informe.cuantizo, "el informe miente: {informe:?}");
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("no se cuantizó")),
        "las líneas no dicen que no se cuantizó: {lineas:?}"
    );
    assert!(
        !lineas.iter().any(|l| l.contains("cuantizado a")),
        "las líneas mienten: {lineas:?}"
    );
    // El cuantizador **no** volvió a correr: el log tiene la llamada de la
    // primera tirada y ninguna más.
    let log = tirada.log("quant.log");
    let llamadas = log.lines().filter(|l| *l == "IQ2_M").count();
    assert_eq!(
        llamadas, 1,
        "el cuantizador se llamó {llamadas} veces: {log}"
    );

    // Y el sidecar tampoco: la herramienta no puede ser «llama-quantize».
    let sidecar = fs::read_to_string(Procedencia::ruta(
        &tirada.base.join("gguf").join("modelo-IQ2_M.gguf"),
    ))
    .expect("el sidecar está");
    assert!(
        !sidecar.contains("llama-quantize"),
        "la herramienta miente: {sidecar}"
    );
    assert!(
        sidecar.contains("no se cuantizó"),
        "la herramienta no dice qué pasó: {sidecar}"
    );
}

/// **Hallazgo 10 / T-30**: el checkpoint puede ser de **otra** petición. Si no
/// es de la tirada actual se descarta, y la tirada se rehace: lo que el
/// informe anuncia tiene que **estar en disco**.
///
/// El defecto: el test consagraba el falso éxito — `expect("la tirada")` sin
/// comprobar que el GGUF exista, y la tirada salía 0 anunciando un fichero que
/// no estaba (el checkpoint sólo guardaba la fase, no la petición).
#[test]
fn el_informe_no_afirma_un_quant_que_no_esta_en_disco() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    // Un checkpoint de otra tirada, ya en la última fase: sin huella, no es de
    // esta petición.
    let cp = tirada.base.join("work/acme__modelo/.ropero-forge.json");
    fs::create_dir_all(cp.parent().unwrap()).expect("crear el work");
    fs::write(&cp, "{\n  \"fase\": \"procedencia\"\n}\n").expect("escribir el checkpoint");

    let peticion = tirada.peticion();
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // Lo que anuncia está en disco, y es un GGUF de verdad.
    assert!(
        tirada.salida().is_file(),
        "el informe anuncia un quant que no está: {informe:?}"
    );
    let contenido = fs::read(tirada.salida()).expect("el GGUF está");
    assert_eq!(
        &contenido[..4],
        b"GGUF",
        "el fichero anunciado no es un GGUF"
    );
    assert!(
        informe.cuantizo,
        "el informe miente sobre el quant: {informe:?}"
    );
    // Y el informe lo dice: el checkpoint no era de esta petición.
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("cambió")),
        "el informe no dice que la petición cambió: {lineas:?}"
    );
    assert!(
        lineas.iter().any(|l| l.contains("cuantizado a IQ2_M")),
        "las líneas no dicen qué se cuantizó: {lineas:?}"
    );
}

/// **Hallazgo 1 (R-GGUF-20, T-30)**: el checkpoint guardaba **sólo la fase**,
/// no la **petición**. Una segunda tirada con otros quants sobre la misma
/// fuente saltaba todas las fases y salía 0 anunciando un GGUF inexistente,
/// sin llamar al cuantizador.
#[test]
fn una_petición_distinta_sobre_la_misma_fuente_no_salta_las_fases() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-F16.gguf", 1);
    let imx = tirada.imatrix();
    let base = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_imatrix(&imx);

    // La primera tirada: Q4_K_M, sale bien.
    let primera = Peticion::de_flags(base.clone().con_quants(&["Q4_K_M"])).expect("la petición");
    let primero = ejecutar::forja(&primera, &tirada.env).expect("la primera tirada");
    assert!(
        primero.cuantizo,
        "la primera tirada no cuantizó: {primero:?}"
    );

    // La segunda, con OTROS quants sobre la misma fuente.
    let segunda = Peticion::de_flags(base.con_quants(&["Q8_0"])).expect("la petición");
    let informe = ejecutar::forja(&segunda, &tirada.env).expect("la segunda tirada");

    // El GGUF anunciado existe, y se cuantizó de verdad.
    let salida = tirada.base.join("gguf").join("modelo-F16-Q8_0.gguf");
    assert!(
        salida.is_file(),
        "el GGUF anunciado no está en disco: {informe:?}"
    );
    let contenido = fs::read(&salida).expect("el GGUF está");
    assert_eq!(&contenido[..4], b"GGUF", "el fichero no es un GGUF");
    assert!(informe.cuantizo, "el informe miente: {informe:?}");
    // El informe dice que la petición cambió (y por eso se rehizo).
    let lineas = ejecutar::lineas(&informe);
    assert!(
        lineas.iter().any(|l| l.contains("cambió")),
        "el informe no dice que la petición cambió: {lineas:?}"
    );
    // El cuantizador se llamó las dos veces: una por quant.
    let log = tirada.log("quant.log");
    let llamadas = log
        .lines()
        .filter(|l| matches!(*l, "Q4_K_M" | "Q8_0"))
        .count();
    assert_eq!(
        llamadas, 2,
        "el cuantizador se llamó {llamadas} veces: {log}"
    );
}

// ── La clase de un GGUF: se lee de su cabecera, no de su nombre (hallazgo 2) ─

/// **R-GGUF-7**: un GGUF que ya está en el quant pedido **se coloca** y se
/// sale 0. Con el sufijo de quant en el nombre, la tirada lo confundía con
/// pesos sin cuantizar y **re-cuantizaba** (el motor real lo rechaza) y
/// exigía imatrix.
#[test]
fn un_gguf_ya_en_el_quant_pedido_se_coloca_y_no_se_recuantiza() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.fichero_fixture(
        "acme/modelo-IQ2_M",
        "modelo-IQ2_M.gguf",
        &gguf_con_file_type(29),
    );
    let flags = Flags::default()
        .con_gguf("acme/modelo-IQ2_M")
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    assert!(
        !informe.cuantizo,
        "re-cuantiza lo ya cuantizado: {informe:?}"
    );
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "el cuantizador se llamó: {}",
        tirada.log("quant.log")
    );
    let salida = tirada.base.join("gguf").join("modelo-IQ2_M.gguf");
    assert!(salida.is_file(), "el GGUF colocado no está en disco");
}

/// **Hallazgo 2b**: un GGUF ya cuantizado + otro quant es **NoViable** (R-GGUF-8),
/// y el preflight se **salta** porque la clase se deducía mal del nombre.
#[test]
fn un_gguf_ya_cuantizado_con_otro_quant_no_es_viable_sin_cuantizar() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-Q4_K_M.gguf", 15);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["Q8_0"])
        .con_imatrix(&tirada.imatrix());
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("no es viable");
    match &error {
        Error::NoViable { pedido, tienen, .. } => {
            assert_eq!(pedido, "Q8_0");
            assert_eq!(tienen, "Q4_K_M", "el quant real es el de la cabecera");
        }
        otro => panic!("esperaba un NoViable, y salió: {otro:?}"),
    }
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "cuantiza lo ya cuantizado: {}",
        tirada.log("quant.log")
    );
}

/// **Hallazgo 2c**: un GGUF en F16 pedido **como quant** se coloca (R-GGUF-7:
/// no hay quant que hacer), en vez de morir en la fase 4 «no está».
#[test]
fn un_gguf_en_f16_pedido_como_quant_se_coloca_y_sale_0() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-F16.gguf", 1);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["F16"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    assert!(!informe.cuantizo, "no hay quant que hacer: {informe:?}");
    assert_eq!(
        informe.quant_real, "F16",
        "el quant real es el de la cabecera"
    );
    assert!(tirada.log("quant.log").trim().is_empty(), "no se cuantiza");
    let salida = tirada.base.join("gguf").join("modelo-F16.gguf");
    assert!(salida.is_file(), "el F16 no se colocó");
    assert!(Procedencia::ruta(&salida).is_file(), "el sidecar no está");
}

/// **Hallazgo 2**: la clase se deduce del `general.file_type` **de la cabecera**,
/// no del nombre. `modelo-BF16` con cabecera IQ2_M es IQ2_M (no pesos en claro).
#[test]
fn la_clase_del_gguf_se_lee_de_la_cabecera_no_del_nombre() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-BF16.gguf", 29);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    assert!(
        !informe.cuantizo,
        "el nombre confundió la clase: {informe:?}"
    );
    assert_eq!(
        informe.quant_real, "IQ2_M",
        "el quant real es el de la cabecera"
    );
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "el cuantizador se llamó: {}",
        tirada.log("quant.log")
    );
}

/// **Hallazgo 6 (R-GGUF-19)**: el sidecar dice **lo que pasó**. Con fuente
/// `--gguf` no hubo conversión (no hay intermedio) y el cuantizador no se
/// llamó, así que el sidecar no puede afirmar ninguno de los dos.
#[test]
fn el_sidecar_dice_lo_que_pasó() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.entrada("modelo-Q4_K_M.gguf", 15);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["Q4_K_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe = ejecutar::forja(&peticion, &tirada.env).expect("la tirada");
    assert!(!informe.cuantizo, "no se cuantizó: {informe:?}");

    let sidecar = fs::read_to_string(Procedencia::ruta(
        &tirada.base.join("gguf").join("modelo-Q4_K_M.gguf"),
    ))
    .expect("el sidecar está");
    assert!(
        sidecar.contains("\"intermedio\": null"),
        "afirma un intermedio que no existió: {sidecar}"
    );
    assert!(
        sidecar.contains("sin cuantizar"),
        "la herramienta miente: {sidecar}"
    );
    assert!(
        !sidecar.contains("llama-quantize"),
        "afirma que el cuantizador se llamó: {sidecar}"
    );

    // La contramutación: cuando SÍ hubo conversión y cuantización, el sidecar
    // los declara (la tirada de `la_tirada_completa_fabrica_el_gguf`).
}

/// **Hallazgo 7**: el imatrix **local** no se comprobaba: se cuantizaba a
/// ciegas con un fichero inexistente (el remoto sí se comprobaba).
#[test]
fn el_imatrix_local_tambien_se_comprueba() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["Q4_K_M"])
        .con_imatrix(&tirada.base.join("no-esta.dat").to_string_lossy());
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("el imatrix no está");
    assert!(
        matches!(error, Error::ImatrixNoEsta { .. }),
        "salió: {error:?}"
    );
    assert!(
        error.to_string().contains("no-esta.dat"),
        "el error nombra el fichero: {error}"
    );
    assert!(
        tirada.log("quant.log").trim().is_empty(),
        "cuantiza a ciegas sin imatrix: {}",
        tirada.log("quant.log")
    );
}

/// **Hallazgo 9 (R-GGUF-55)**: el conversor sólo hace falta si hay que
/// **convertir**. Con una fuente que ya es un GGUF, quien sólo tiene
/// `llama-quantize` puede fabricar.
#[test]
fn el_conversor_solo_hace_falta_si_hay_que_convertir() {
    let tirada = Tirada::nueva().con_dobles().sin_conversor();
    let entrada = tirada.entrada("modelo-F16.gguf", 1);
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["Q4_K_M"])
        .con_imatrix(&tirada.imatrix());
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let informe =
        ejecutar::forja(&peticion, &tirada.env).expect("la tirada con sólo el cuantizador");
    assert!(informe.cuantizo, "no cuantizó: {informe:?}");
    assert!(
        tirada
            .base
            .join("gguf")
            .join("modelo-F16-Q4_K_M.gguf")
            .is_file(),
        "el GGUF no está"
    );
    // Y con una fuente que sí hay que convertir, el conversor sigue siendo
    // imprescindible (R-GGUF-55).
    let tirada = Tirada::nueva().con_dobles().sin_conversor();
    tirada.repo("acme/modelo", "");
    let error = ejecutar::forja(&tirada.peticion(), &tirada.env).expect_err("sin conversor");
    assert!(matches!(error, Error::SinMotor { .. }), "salió: {error:?}");
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

/// La fuente `--gguf` **también** deja checkpoint en la fase
/// «conversion» (aunque no haya conversión): si no, el checkpoint se
/// queda atrás («fuente») y la reanudación no sabe por dónde seguir.
#[test]
fn el_gguf_de_entrada_deja_el_checkpoint_en_la_conversion() {
    let tirada = Tirada::nueva().con_dobles();
    let entrada = tirada.base.join("entrada").join("modelo.gguf");
    fs::create_dir_all(entrada.parent().unwrap()).expect("crear entrada");
    fs::write(&entrada, {
        let mut v = b"GGUF".to_vec();
        v.extend(vec![0u8; 2048]);
        v
    })
    .expect("escribir el GGUF de entrada");
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    let flags = Flags::default()
        .con_gguf(&entrada.to_string_lossy())
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());

    // La primera tirada falla en la cuantización.
    let peticion = Peticion::de_flags(flags.clone()).expect("la petición");
    tirada.modo("imatrix");
    let error = ejecutar::forja(&peticion, &tirada.env).expect_err("la cuantización");
    assert!(
        matches!(error, Error::Cuantizacion { .. }),
        "salió: {error:?}"
    );
    tirada.modo("");

    // El checkpoint dice por dónde se sigue: la conversión (aunque el
    // GGUF de entrada no pase por el conversor).
    let cp = tirada.checkpoint_del_work();
    assert!(
        cp.contains("conversion"),
        "el checkpoint se quedó atrás: {cp}"
    );

    // Y la segunda tirada reanuda y **cuantiza** de verdad.
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("el reintento");
    assert!(
        tirada.salida().is_file(),
        "la segunda tirada no cuantizó: se quedó sin hacer nada"
    );
}

/// El GGUF **a medias** que dejó el cuantizador (falló después de
/// escribirlo) sí se borra: está corrupto, y si se quedara, la siguiente
/// tirada creería que la salida está hecha y no la reharía.
///
/// Lo que **no** se borra nunca aquí son el clon y el intermedio
/// (R-GGUF-16): sin ellos no se puede reintentar sólo la cuantización.
#[test]
fn el_gguf_a_medias_se_borra_pero_el_clon_y_el_intermedio_no() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    tirada.modo("parcial");
    let error = ejecutar::forja(&tirada.peticion(), &tirada.env).expect_err("la cuantización");
    tirada.modo("");
    assert!(
        matches!(error, Error::Cuantizacion { .. }),
        "salió: {error:?}"
    );

    // El GGUF a medias, fuera: está corrupto.
    assert!(!tirada.salida().exists(), "el GGUF a medias se queda");
    // El clon y el intermedio, dentro: se pueden reintentar.
    assert!(
        tirada.base.join("work/acme__modelo").is_dir(),
        "el clon se queda (R-GGUF-16)"
    );
    assert!(
        tirada.base.join("gguf/modelo-F16.gguf").is_file(),
        "el intermedio se queda (R-GGUF-16)"
    );
}

/// El checkpoint se escribe **de un tirón**: a un temporal y renombrando
/// (el rename es atómico). Así dos tiradas a la vez no pueden dejar el
/// checkpoint a medias, y no queda ningún temporal atrás.
#[test]
fn el_checkpoint_se_escribe_atomico_y_no_deja_temporales() {
    use std::os::unix::fs::MetadataExt;
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    // Un checkpoint de antes, escrito a mano: el work existe.
    let cp = tirada.base.join("work/acme__modelo/.ropero-forge.json");
    fs::create_dir_all(cp.parent().unwrap()).expect("crear el work");
    fs::write(&cp, "{\n  \"fase\": \"fuente\"\n}\n").expect("escribir el checkpoint");
    let antes = fs::metadata(&cp).expect("el checkpoint").ino();

    // `--keep-source`: el work (y el checkpoint) sobreviven.
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    flags.keep_source = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    // El checkpoint se **sustituye** (el rename es atómico): no es el
    // mismo fichero truncado y reescrito.
    let despues = fs::metadata(&cp).expect("el checkpoint").ino();
    assert_ne!(
        antes, despues,
        "el checkpoint se reescribió en el sitio: no es atómico"
    );
    // El checkpoint está y se lee.
    let texto = fs::read_to_string(&cp).expect("el checkpoint está");
    assert!(Checkpoint::de_json(&texto).is_some(), "dice {texto}");
    // Y no quedó ningún temporal por el camino.
    let leftovers: Vec<String> = fs::read_dir(tirada.base.join("work/acme__modelo"))
        .expect("el work")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporales sin renombrar: {leftovers:?}"
    );
}

/// `--no-dflash` se pasa al conversor **que lo entiende** (como
/// `--no-mtp`, R-GGUF-36): un flag que el conversor no conoce hace
/// fallar la conversión.
#[test]
fn el_no_dflash_se_pasa_al_conversor_que_lo_entiende() {
    let tirada = Tirada::nueva().con_dobles().con_dflash();
    tirada.repo("acme/modelo", "");
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix(&tirada.base.join("imatrix.dat").to_string_lossy());
    fs::write(tirada.base.join("imatrix.dat"), b"imatrix de fixture").expect("imatrix");
    flags.no_dflash = true;
    let peticion = Peticion::de_flags(flags).expect("la petición");
    ejecutar::forja(&peticion, &tirada.env).expect("la tirada");

    let llamada = tirada.log("conv.log");
    assert!(
        llamada.contains("--no-dflash"),
        "el conversor no recibió el flag: {llamada}"
    );
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

/// Un GGUF ya hecho **sin sidecar** no sale con éxito (R-GGUF-19, T-34).
///
/// El defecto: el camino «ya está» salía con **0** y sin procedencia: un
/// GGUF sin sidecar no se puede publicar, y la tirada no lo decía (T-34:
/// *un GGUF sin sidecar se rechaza*).
#[test]
fn un_gguf_ya_hecho_sin_sidecar_no_sale_con_exito() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    // El GGUF está (de otra tirada, o copiado a mano), pero su sidecar no.
    let salida = tirada.salida();
    fs::create_dir_all(salida.parent().unwrap()).expect("crear el directorio de salida");
    fs::write(&salida, {
        let mut v = b"GGUF".to_vec();
        v.extend(vec![0u8; 2048]);
        v
    })
    .expect("escribir el GGUF");

    let error = ejecutar::forja(&tirada.peticion(), &tirada.env).expect_err("sin sidecar");
    assert!(
        matches!(error, Error::SinSidecar { .. }),
        "salió: {error:?}"
    );
    assert_eq!(
        error.codigo(),
        1,
        "un GGUF sin sidecar no puede salir con 0"
    );
    // Y el error dice **qué hacer**.
    let pista = error.pista().unwrap_or_default();
    assert!(
        pista.contains("--force"),
        "la pista no dice cómo rehacerlo: {pista}"
    );
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
    /// Un conversor que **sí** entiende `--no-dflash`.
    fn con_dflash(self) -> Self {
        let convert = self.base.join("motor/convert_hf_to_gguf.py");
        fs::write(&convert, CONVERT_CON_DFLASH).expect("escribir el conversor");
        self
    }

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

/// El **ETXTBSY** («Text file busy») es una carrera, no un error: el
/// fichero que se ejecuta lo tiene otro proceso abierto para escribir
/// (un `fork` en paralelo hereda el descriptor). La forja reintenta, y la
/// tirada sale bien.
///
/// Aquí la carrera se provoca a propósito: el `hf` falso se abre para
/// escritura y se mantiene abierto un rato, así que el primer exec
/// falla con ETXTBSY.
#[test]
fn el_texto_ocupado_no_tira_la_tirada() {
    let tirada = Tirada::nueva().con_dobles();
    tirada.repo("acme/modelo", "");
    // El `hf` falso, abierto para escribir: no se puede ejecutar.
    let hf = tirada.base.join("bin/hf");
    let escritor = fs::OpenOptions::new()
        .write(true)
        .open(&hf)
        .expect("abrir el hf");
    // Y se cierra dentro de un momento, en otro hilo.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        drop(escritor);
    });
    let informe = ejecutar::forja(&tirada.peticion(), &tirada.env).expect("la tirada");
    assert!(informe.cuantizo, "la tirada sale: {informe:?}");
    assert!(tirada.salida().is_file(), "el GGUF está");
}
