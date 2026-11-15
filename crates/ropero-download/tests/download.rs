//! La descarga real con `hf` (R-MOD-15…21, T-26).
//!
//! La puerta de T-26 es exacta: *«con `hf` falso: se llama con los
//! argumentos exactos»*. Por eso la batería usa un **`hf` falso**: un
//! script que **graba sus argumentos** y crea el fichero esperado. Con él
//! se comprueba la secuencia de R-MOD-16 paso a paso, el staging de
//! R-MOD-15, la reanudación de R-MOD-17 y la verificación por contenido de
//! R-MOD-18…21.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ropero_core::{Environment, ModelSpec, parse_model};
use ropero_download::Error;
use ropero_download::download::{Modo, Resultado, descarga, descarga_con, pregunta_de_descarga};
use ropero_download::plan::{STAGING, plan_de_descarga};

/// El `hf` falso: graba sus argumentos y finge la descarga.
///
/// El modo se lee de un fichero al lado del script (para que cada test
/// pueda cambiarlo sin tocar el entorno del proceso, que es compartido).
const HF_FALSO: &str = r#"#!/bin/sh
# El `hf` falso de la batería: graba los argumentos y crea el fichero.
# Sólo builtins: el PATH de la batería es un temporal (sin dirname ni mkdir).
base="${0%/*}"
modo="ok"
if [ -f "$base/modo" ]; then
  modo=""
  while IFS= read -r linea || [ -n "$linea" ]; do modo="$linea"; done < "$base/modo"
fi
printf '%s\n' "$@" > "$base/args.log"

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
  checkpoint)
    printf 'pesos\n' > "$dir/model.safetensors"
    printf '{}\n' > "$dir/model.safetensors.index.json"
    printf '{}\n' > "$dir/config.json"
    ;;
  *)
    if [ -n "$file" ]; then
      printf 'contenido falso\n' > "$dir/$file"
    else
      printf 'pesos\n' > "$dir/model.safetensors"
      printf '{}\n' > "$dir/model.safetensors.index.json"
      printf '{}\n' > "$dir/config.json"
    fi
    ;;
esac
exit 0
"#;

/// Un modelo GGUF cuyo fichero final **no** se llama como en el repo.
const GGUF: &str = r#"
[model]
name = "modelo"
engine = "turbo-llama"

[files]
gguf = "final.gguf"

[download]
repo = "ejemplo/modelo"
file = "en-el-repo.gguf"
size = "~4 GB"
"#;

/// Un modelo GGUF con un auxiliar de runtime.
const GGUF_CON_AUX: &str = r#"
[model]
name = "modelo"
engine = "turbo-llama"

[files]
gguf = "final.gguf"

[download]
repo = "ejemplo/modelo"
file = "en-el-repo.gguf"

[download.aux.chat_template]
repo = "otro/repo"
file = "chat_template.jinja"
name = "chat_template.jinja"
size = "9 KB"
use = "run"
"#;

/// Un GGUF **sin tamaño** declarado: el plan no sabe cuánto pesa.
const GGUF_SIN_TAMANO: &str = r#"
[model]
name = "modelo"
engine = "turbo-llama"

[files]
gguf = "final.gguf"

[download]
repo = "ejemplo/modelo"
file = "en-el-repo.gguf"
"#;

/// Un checkpoint safetensors.
const SAFETENSORS: &str = r#"
[model]
name = "checkpoint"
engine = "mlx-serve"
format = "safetensors"
path = "ejemplo/checkpoint"

[files]

[download]
repo = "ejemplo/checkpoint"
size = "~18 GB"
excludes = ["original/*"]
"#;

/// Un checkpoint safetensors con un auxiliar de runtime.
const SAFETENSORS_CON_AUX: &str = r#"
[model]
name = "checkpoint"
engine = "mlx-serve"
format = "safetensors"
path = "ejemplo/checkpoint"

[files]

[download]
repo = "ejemplo/checkpoint"

[download.aux.chat_template]
file = "chat_template.jinja"
name = "chat_template.jinja"
size = "9 KB"
use = "run"
"#;

/// La máquina de pruebas: el temporal y el `hf` falso.
struct Fake {
    base: PathBuf,
    path: PathBuf,
}

impl Fake {
    /// Monta la máquina de pruebas en un temporal.
    fn nuevo(nombre: &str) -> Self {
        let base = std::env::temp_dir().join(format!("ropero-download-{nombre}"));
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("bin");
        std::fs::create_dir_all(&path).expect("crear el temporal");
        let hf = path.join("hf");
        std::fs::write(&hf, HF_FALSO).expect("hf falso");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permisos = std::fs::metadata(&hf).expect("permisos").permissions();
            permisos.set_mode(0o755);
            std::fs::set_permissions(&hf, permisos).expect("chmod");
        }
        Fake { base, path }
    }

    /// Cambia el modo del `hf` falso.
    fn modo(&self, modo: &str) {
        std::fs::write(self.path.join("modo"), modo).expect("modo");
    }

    /// Los argumentos con los que se llamó a `hf` (vacío si no se llamó).
    fn argumentos(&self) -> Vec<String> {
        match std::fs::read_to_string(self.path.join("args.log")) {
            Ok(texto) => texto.lines().map(|l| l.to_string()).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// El entorno con el `PATH` del `hf` falso.
    fn entorno(&self) -> Environment {
        self.entorno_con_path(&self.path)
    }

    /// El entorno **sin** `hf` en el PATH.
    fn entorno_sin_hf(&self) -> Environment {
        self.entorno_con_path(&self.base.join("vacio"))
    }

    /// El entorno con un `PATH` dado.
    fn entorno_con_path(&self, path: &Path) -> Environment {
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), path.to_string_lossy().to_string());
        vars.insert(
            "GGUF_DIR".to_string(),
            self.base.join("gguf").to_string_lossy().to_string(),
        );
        vars.insert(
            "SAFETENSORS_DIR".to_string(),
            self.base.join("safetensors").to_string_lossy().to_string(),
        );
        Environment::new(vars, &self.base, &self.base)
    }

    /// Los ficheros que quedan en un directorio.
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

/// El modelo del TOML.
fn spec(toml: &str) -> ModelSpec {
    parse_model(toml, Path::new("modelo.toml")).expect("parse model")
}

// ─────────────────────────────────────────────────────────────────────────────
// Los argumentos exactos de `hf` (la puerta de T-26)
// ─────────────────────────────────────────────────────────────────────────────

/// En un GGUF, `hf` se llama con `download <repo> <fichero> --local-dir
/// <staging>`: el staging va en el plan como marcador, porque **no se puede
/// saber al calcularlo** (es un directorio único que se crea al descargar).
#[test]
fn los_argumentos_de_hf_en_gguf_son_los_exactos() {
    let fake = Fake::nuevo("gguf-args");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");
    assert_eq!(
        plan.hf_args,
        vec![
            "download",
            "ejemplo/modelo",
            "en-el-repo.gguf",
            "--local-dir",
            STAGING
        ]
    );

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");
    assert_eq!(resultado, Resultado::Descargado);

    let args = fake.argumentos();
    assert_eq!(
        &args[..4],
        &[
            "download",
            "ejemplo/modelo",
            "en-el-repo.gguf",
            "--local-dir"
        ],
        "hf no se ha llamado con los argumentos exactos: {args:?}"
    );
}

/// En un checkpoint, `hf` se llama con `download <repo> --local-dir
/// <destino>` y **un `--exclude` por cada excluido**: los de basura
/// siempre y los del modelo.
#[test]
fn los_argumentos_de_hf_en_safetensors_son_los_exactos() {
    let fake = Fake::nuevo("safetensors-args");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS), &env).expect("plan");
    let destino = plan.dest.to_string_lossy().to_string();
    assert_eq!(
        plan.hf_args,
        vec![
            "download",
            "ejemplo/checkpoint",
            "--local-dir",
            destino.as_str(),
            "--exclude",
            ".gitattributes",
            "--exclude",
            "*.md",
            "--exclude",
            ".DS_Store",
            "--exclude",
            "*.png",
            "--exclude",
            "*.jpg",
            "--exclude",
            "*.gif",
            "--exclude",
            "original/*",
        ]
    );

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");
    assert_eq!(resultado, Resultado::Descargado);
    assert_eq!(fake.argumentos(), plan.hf_args);
}

// ─────────────────────────────────────────────────────────────────────────────
// El staging (R-MOD-15)
// ─────────────────────────────────────────────────────────────────────────────

/// El staging vive **dentro del destino** (mismo filesystem, para que el
/// `mv` sea atómico) y **se borra** al terminar.
#[test]
fn el_staging_vive_dentro_del_destino_y_se_borra() {
    let fake = Fake::nuevo("staging");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");

    descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    let args = fake.argumentos();
    let staging = PathBuf::from(&args[4]);
    assert!(
        staging.starts_with(&plan.dest_dir),
        "el staging no está dentro del destino: {staging:?}"
    );
    let nombre = staging
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    assert!(
        nombre.starts_with(".rp-dl."),
        "el staging no lleva el prefijo del contrato: {nombre}"
    );
    // Y al terminar no queda rastro del staging.
    assert_eq!(
        fake.entradas(&plan.dest_dir),
        vec!["final.gguf".to_string()],
        "el staging no se ha borrado"
    );
}

/// Un GGUF repetido **no deja restos**: el staging de la segunda vez se
/// borra igual.
#[test]
fn una_descarga_repetida_no_deja_restos() {
    let fake = Fake::nuevo("staging-repetido");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");

    descarga(&plan, &env, Modo::Desatendido).expect("primera");
    // Se borra el resultado para volver a descargar.
    std::fs::remove_file(&plan.dest).expect("borrar");
    descarga(&plan, &env, Modo::Desatendido).expect("segunda");

    assert_eq!(
        fake.entradas(&plan.dest_dir),
        vec!["final.gguf".to_string()],
        "la segunda descarga ha dejado restos"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La secuencia de R-MOD-16
// ─────────────────────────────────────────────────────────────────────────────

/// (1) El nombre final existe y **no está vacío** → «ya descargado», y
/// `hf` **no** se llama.
#[test]
fn un_fichero_final_ya_descargado_no_llama_a_hf() {
    let fake = Fake::nuevo("ya-descargado");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");
    std::fs::create_dir_all(&plan.dest_dir).expect("directorio");
    std::fs::write(&plan.dest, "contenido\n").expect("fichero");

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    assert_eq!(resultado, Resultado::YaDescargado);
    assert!(
        fake.argumentos().is_empty(),
        "hf no debía llamarse: {:?}",
        fake.argumentos()
    );
}

/// Un fichero final **vacío** no cuenta como descargado: se vuelve a bajar.
#[test]
fn un_fichero_vacio_no_cuenta_como_descargado() {
    let fake = Fake::nuevo("vacio");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");
    std::fs::create_dir_all(&plan.dest_dir).expect("directorio");
    std::fs::write(&plan.dest, "").expect("fichero vacío");

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    assert_eq!(resultado, Resultado::Descargado);
    assert!(
        !fake.argumentos().is_empty(),
        "hf no se ha llamado: el fichero vacío no cuenta"
    );
    let contenido = std::fs::read_to_string(&plan.dest).expect("leer");
    assert!(!contenido.is_empty(), "el fichero sigue vacío");
}

/// (2) Si el final no existe pero está **con el nombre del repo**, se
/// renombra y se da por buena (sin llamar a `hf`).
#[test]
fn el_fichero_con_el_nombre_del_repo_se_renombra() {
    let fake = Fake::nuevo("renombrar");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");
    std::fs::create_dir_all(&plan.dest_dir).expect("directorio");
    let origen = plan.dest_dir.join("en-el-repo.gguf");
    std::fs::write(&origen, "contenido\n").expect("fichero del repo");

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    assert_eq!(resultado, Resultado::YaDescargado);
    assert!(
        fake.argumentos().is_empty(),
        "hf no debía llamarse: el fichero ya estaba"
    );
    assert!(plan.dest.is_file(), "el fichero no se ha renombrado");
    assert!(!origen.exists(), "el fichero del repo sigue en su sitio");
}

/// (3) **`hf` se comprueba ANTES de preguntar**: sin `hf` en el PATH el
/// error sale **sin haber preguntado**.
#[test]
fn sin_hf_no_se_pregunta() {
    let fake = Fake::nuevo("sin-hf");
    let env = fake.entorno_sin_hf();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");
    let mut pregunto = false;

    let resultado = descarga_con(&plan, &env, Modo::Interactivo, |_| {
        pregunto = true;
        true
    });

    assert!(
        matches!(resultado, Err(Error::FaltaHf { .. })),
        "el error no es el de `hf` ausente: {resultado:?}"
    );
    assert!(
        !pregunto,
        "se ha preguntado antes de comprobar que hay `hf`: \
         prometer una descarga y luego confesar que no se puede es la peor secuencia posible"
    );
}

/// (4) En modo **desatendido** no se pregunta; en modo **interactivo** se
/// pregunta **con el tamaño** y el default es **no**.
#[test]
fn el_modo_desatendido_no_pregunta_y_el_interactivo_si() {
    let fake = Fake::nuevo("pregunta");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");

    // Desatendido: no hay pregunta.
    let mut pregunto = false;
    let resultado = descarga_con(&plan, &env, Modo::Desatendido, |_| {
        pregunto = true;
        false
    })
    .expect("descarga");
    assert_eq!(resultado, Resultado::Descargado);
    assert!(!pregunto, "en modo desatendido no se pregunta");

    // Interactivo: la pregunta lleva el tamaño (R-MOD-20: es informativo).
    // El tamaño lleva la forma única de la salida: `~` y dos decimales.
    let pregunta = pregunta_de_descarga(&plan);
    assert!(
        pregunta.contains("~4.00 GB"),
        "la pregunta no dice el tamaño: {pregunta}"
    );
    assert!(
        pregunta.contains("[s/N]"),
        "la pregunta no dice que el default es no: {pregunta}"
    );
}

/// El tamaño que no se sabe sale como `—`, como en el resto de la salida.
///
/// El `?` era otro idioma: el sistema usa `—` para lo que no se sabe
/// (doc CLI v2 §2.1).
#[test]
fn la_pregunta_de_descarga_dice_el_tamano_desconocido() {
    let fake = Fake::nuevo("tamano-desconocido");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF_SIN_TAMANO), &env).expect("plan");

    let pregunta = pregunta_de_descarga(&plan);
    assert!(pregunta.contains("(—)"), "pregunta={pregunta}");
    assert!(!pregunta.contains("(?)"), "pregunta={pregunta}");
}

/// El default de la pregunta es **no**: si el usuario no contesta, el
/// modelo se omite (y no es un error).
#[test]
fn el_default_de_la_pregunta_es_no() {
    let fake = Fake::nuevo("default-no");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");

    let resultado = descarga_con(&plan, &env, Modo::Interactivo, |_| false).expect("descarga");

    assert_eq!(resultado, Resultado::Omitido);
    assert!(
        !plan.dest.exists(),
        "no se ha descargado nada: el usuario no lo confirmó"
    );
    assert!(fake.argumentos().is_empty(), "hf no debía llamarse");
}

/// (5) Si `hf` sale con 0 pero no deja el fichero, es un error: un exit 0
/// de un downloader no es un modelo instalado.
#[test]
fn hf_que_no_deja_el_fichero_es_error() {
    let fake = Fake::nuevo("hf-nada");
    fake.modo("nada");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF), &env).expect("plan");

    let resultado = descarga(&plan, &env, Modo::Desatendido);

    match &resultado {
        Err(Error::HfSinFichero {
            modelo,
            file,
            destino,
        }) => {
            assert_eq!(modelo, "modelo");
            assert_eq!(file, "en-el-repo.gguf");
            assert_eq!(*destino, plan.dest.to_string_lossy());
        }
        otro => panic!("el error no es el de `hf` sin fichero: {otro:?}"),
    }
    // El wording es el de R-MOD-19: «hf terminó pero <destino> está
    // incompleto».
    let texto = resultado.unwrap_err().to_string();
    assert!(
        texto.contains("hf terminó pero") && texto.contains("está incompleto"),
        "el mensaje no dice que el destino está incompleto: {texto}"
    );
    assert!(
        !plan.dest.exists(),
        "no puede haber un modelo instalado sin fichero"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La verificación por contenido (R-MOD-18, R-MOD-19)
// ─────────────────────────────────────────────────────────────────────────────

/// Aunque `hf` salga con 0, se **re-verifica**: si al checkpoint le falta
/// algo, el error lo dice y el estado no queda «instalado».
#[test]
fn hf_que_sale_con_cero_pero_deja_el_checkpoint_incompleto_es_error() {
    let fake = Fake::nuevo("incompleto");
    fake.modo("nada");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS), &env).expect("plan");

    let resultado = descarga(&plan, &env, Modo::Desatendido);

    match resultado {
        Err(Error::Incompleto {
            modelo,
            destino,
            falta,
        }) => {
            assert_eq!(modelo, "checkpoint");
            assert_eq!(destino, plan.dest.to_string_lossy());
            assert!(
                falta.iter().any(|f| f.contains("pesos")),
                "el error no dice qué falta: {falta:?}"
            );
            assert!(
                falta.iter().any(|f| f.contains("config.json")),
                "el error no dice qué falta: {falta:?}"
            );
        }
        otro => panic!("el error no es el de checkpoint incompleto: {otro:?}"),
    }
    assert!(
        !plan.dest.join(".download_complete").exists(),
        "el marcador no se deja cuando el checkpoint está incompleto"
    );
}

/// Un checkpoint **completo** es un modelo instalado: los pesos y el
/// `config.json` están.
#[test]
fn un_checkpoint_completo_esta_descargado() {
    let fake = Fake::nuevo("checkpoint-ok");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS), &env).expect("plan");

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    assert_eq!(resultado, Resultado::Descargado);
    assert!(plan.dest.join("model.safetensors").is_file());
    assert!(plan.dest.join("config.json").is_file());
    assert!(plan.dest.join(".download_complete").is_file());
}

/// Un shard suelto **no cuenta**: con más de un shard se exige el índice
/// (R-MOD-18: *«un shard suelto NO cuenta»*).
#[test]
fn un_shard_suelto_no_cuenta() {
    let fake = Fake::nuevo("shard-suelto");
    fake.modo("nada");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS), &env).expect("plan");
    std::fs::create_dir_all(&plan.dest).expect("directorio");
    std::fs::write(plan.dest.join("model-00001-of-00002.safetensors"), "x").expect("shard");
    std::fs::write(plan.dest.join("model-00002-of-00002.safetensors"), "x").expect("shard");
    std::fs::write(plan.dest.join("config.json"), "{}").expect("config");

    let resultado = descarga(&plan, &env, Modo::Desatendido);

    assert!(
        matches!(resultado, Err(Error::Incompleto { .. })),
        "un shard suelto no es un modelo instalado: {resultado:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La reanudación (R-MOD-17)
// ─────────────────────────────────────────────────────────────────────────────

/// La descarga de un checkpoint **no usa staging**: el `--local-dir` es el
/// destino fijo, así que una descarga cortada **continúa** en vez de
/// empezar de cero.
#[test]
fn la_reanudacion_de_safetensors_usa_el_mismo_local_dir() {
    let fake = Fake::nuevo("reanudacion");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS), &env).expect("plan");
    std::fs::create_dir_all(&plan.dest).expect("directorio");
    let parcial = plan.dest.join("model-00001-of-00002.safetensors");
    std::fs::write(&parcial, "descarga a medias").expect("parcial");

    descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    let args = fake.argumentos();
    let local = PathBuf::from(&args[3]);
    assert_eq!(
        local, plan.dest,
        "el checkpoint no se reanuda: el --local-dir no es el destino"
    );
    assert!(
        parcial.is_file(),
        "la descarga a medias se ha perdido: no se reanuda"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los auxiliares
// ─────────────────────────────────────────────────────────────────────────────

/// Los auxiliares (`[download.aux]`) se descargan también, a su destino.
#[test]
fn los_auxiliares_se_descargan() {
    let fake = Fake::nuevo("aux");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF_CON_AUX), &env).expect("plan");
    assert_eq!(plan.aux.len(), 1, "el plan no ve el auxiliar");

    descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    let aux = &plan.aux[0];
    assert!(aux.dest.is_file(), "el auxiliar no se ha descargado");
    assert_eq!(
        std::fs::read_to_string(&aux.dest).unwrap_or_default(),
        "contenido falso\n",
        "el auxiliar está vacío"
    );
    // El auxiliar vive en su repo, que puede ser otro.
    let args = fake.argumentos();
    assert!(
        args.iter().any(|a| a == "otro/repo"),
        "el auxiliar no se ha bajado de su repo: {args:?}"
    );
}

/// Un auxiliar que ya está no se vuelve a bajar.
#[test]
fn un_auxiliar_ya_descargado_no_se_vuelve_a_bajar() {
    let fake = Fake::nuevo("aux-ya");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(GGUF_CON_AUX), &env).expect("plan");
    std::fs::create_dir_all(plan.aux[0].dest.parent().expect("dir")).expect("directorio");
    std::fs::write(&plan.aux[0].dest, "ya está\n").expect("auxiliar");

    let resultado = descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    assert_eq!(resultado, Resultado::Descargado);
    let args = fake.argumentos();
    assert!(
        !args.iter().any(|a| a == "otro/repo"),
        "el auxiliar ya estaba y no se ha vuelto a bajar: {args:?}"
    );
}

/// Los auxiliares de un **checkpoint** también se bajan: van al
/// directorio del GGUF, que es donde los referencia el arranque
/// (R-MOD-23).
#[test]
fn los_auxiliares_de_un_checkpoint_se_descargan() {
    let fake = Fake::nuevo("aux-checkpoint");
    let env = fake.entorno();
    let plan = plan_de_descarga(&spec(SAFETENSORS_CON_AUX), &env).expect("plan");
    assert_eq!(plan.aux.len(), 1, "el plan no ve el auxiliar");

    descarga(&plan, &env, Modo::Desatendido).expect("descarga");

    let aux = &plan.aux[0];
    assert!(aux.dest.is_file(), "el auxiliar no se ha descargado");
    // Y va al directorio del GGUF, no al del checkpoint.
    assert!(
        aux.dest
            .starts_with(env.env_path("GGUF_DIR", "models/gguf")),
        "el auxiliar no va al directorio del GGUF: {aux:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Las rutas del catálogo, acotadas al directorio de modelos
// ─────────────────────────────────────────────────────────────────────────────

/// El error de una ruta que se sale: dice **qué campo** es y **qué valor**
/// tenía (P-13).
fn ruta_fuera(error: &Error) -> (&str, &str) {
    match error {
        Error::RutaFuera { campo, valor, .. } => (campo, valor),
        otro => panic!("el error no es el de ruta fuera: {otro:?}"),
    }
}

/// Un `[download].file` con `..` **sube**: con `Path::join` el `mv` de
/// R-MOD-16.2 movería un fichero de fuera del directorio del modelo.
///
/// Es un defecto del catálogo: el error nombra el campo y el valor.
#[test]
fn un_file_con_puntos_suspensivos_es_error() {
    let fake = Fake::nuevo("file-fuera");
    let env = fake.entorno();
    let toml = GGUF.replace(
        "file = \"en-el-repo.gguf\"",
        "file = \"../../../secreto.txt\"",
    );
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[download].file");
    assert_eq!(valor, "../../../secreto.txt");
    // El mensaje dice **qué hacer**.
    let texto = error.to_string();
    assert!(
        texto.contains("[download].file") && texto.contains("../../../secreto.txt"),
        "el mensaje no dice el campo y el valor: {texto}"
    );
    assert!(
        error.pista().is_some_and(|p| p.contains("[download].file")),
        "la pista no dice qué arreglar: {error:?}"
    );
}

/// Un `[download].file` **absoluto** absorbe el base del `join`: también
/// se sale del directorio del modelo.
#[test]
fn un_file_absoluto_es_error() {
    let fake = Fake::nuevo("file-absoluto");
    let env = fake.entorno();
    let toml = GGUF.replace("file = \"en-el-repo.gguf\"", "file = \"/etc/secreto.txt\"");
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[download].file");
    assert_eq!(valor, "/etc/secreto.txt");
}

/// Un `[download].name` que se sale tampoco vale: es el nombre **final**
/// del fichero en disco.
#[test]
fn un_name_que_se_sale_es_error() {
    let fake = Fake::nuevo("name-fuera");
    let env = fake.entorno();
    let toml = GGUF.replace(
        "file = \"en-el-repo.gguf\"",
        "file = \"en-el-repo.gguf\"\nname = \"../../../secreto.txt\"",
    );
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[download].name");
    assert_eq!(valor, "../../../secreto.txt");
}

/// Un auxiliar con un `name` fuera del directorio **sobrescribe** el
/// fichero de fuera: también es un error.
#[test]
fn un_aux_que_se_sale_es_error() {
    let fake = Fake::nuevo("aux-fuera");
    let env = fake.entorno();
    let toml = GGUF_CON_AUX.replace(
        "name = \"chat_template.jinja\"",
        "name = \"../../../victima.txt\"",
    );
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[download.aux.chat_template].name");
    assert_eq!(valor, "../../../victima.txt");
}

/// Un `[model].path` con `..` saca el checkpoint del directorio de los
/// checkpoints.
#[test]
fn un_model_path_que_se_sale_es_error() {
    let fake = Fake::nuevo("path-fuera");
    let env = fake.entorno();
    let toml = SAFETENSORS.replace(
        "path = \"ejemplo/checkpoint\"",
        "path = \"../../../victima\"",
    );
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[model].path");
    assert_eq!(valor, "../../../victima");
}

/// El fichero de fuera **no se toca**: la ruta se comprueba **antes** de
/// mover nada (el `mv` de R-MOD-16.2 no puede sacar un fichero de fuera).
#[test]
fn una_ruta_fuera_no_toca_el_fichero_de_fuera() {
    let fake = Fake::nuevo("file-fuera-no-toca");
    let env = fake.entorno();
    // La «víctima»: un fichero de fuera del directorio del modelo.
    let victima = fake.base.join("secreto.txt");
    std::fs::write(&victima, "VICTIMA ORIGINAL").expect("víctima");
    let toml = GGUF.replace("file = \"en-el-repo.gguf\"", "file = \"../secreto.txt\"");

    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    assert!(matches!(error, Error::RutaFuera { .. }), "{error:?}");
    assert_eq!(
        std::fs::read_to_string(&victima).expect("leer la víctima"),
        "VICTIMA ORIGINAL",
        "el fichero de fuera se ha tocado"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El GGUF va donde el runtime lo busca (R-MOD-6)
// ─────────────────────────────────────────────────────────────────────────────

/// Un `[files].gguf` **con ruta** se descarga en esa ruta: el runtime lo
/// busca en `$GGUF_DIR/<modelo>/<[files].gguf>` (R-MOD-6), no en el
/// `basename`.
#[test]
fn el_gguf_con_ruta_propia_va_a_su_subdirectorio() {
    let fake = Fake::nuevo("gguf-subdir");
    let env = fake.entorno();
    let toml = GGUF.replace("gguf = \"final.gguf\"", "gguf = \"carpeta/final.gguf\"");
    let plan = plan_de_descarga(&spec(&toml), &env).expect("plan");

    let esperado = env
        .env_path("GGUF_DIR", "models/gguf")
        .join("modelo")
        .join("carpeta")
        .join("final.gguf");
    assert_eq!(
        plan.dest, esperado,
        "el GGUF no va donde lo busca el runtime: {plan:?}"
    );

    descarga(&plan, &env, Modo::Desatendido).expect("descarga");
    assert!(plan.dest.is_file(), "no se ha descargado: {plan:?}");
}

/// Un `[files].gguf` que se sale del directorio del modelo es un error.
#[test]
fn un_gguf_que_se_sale_es_error() {
    let fake = Fake::nuevo("gguf-fuera");
    let env = fake.entorno();
    let toml = GGUF.replace("gguf = \"final.gguf\"", "gguf = \"../../../secreto.gguf\"");
    let error = plan_de_descarga(&spec(&toml), &env).expect_err("debe fallar");
    let (campo, valor) = ruta_fuera(&error);
    assert_eq!(campo, "[files].gguf");
    assert_eq!(valor, "../../../secreto.gguf");
}
