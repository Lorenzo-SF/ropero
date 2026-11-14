//! La **ejecución** de la descarga: `hf`, el staging y la verificación.
//!
//! ## La secuencia
//!
//! Es la de **R-MOD-16**, y es **exactamente**:
//!
//! 1. si el nombre final existe y no está vacío → «ya descargado»;
//! 2. si no existe el final pero existe **con el nombre del repo** → `mv`
//!    y se da por buena;
//! 3. **`hf` ANTES de preguntar**;
//! 4. la confirmación, con el tamaño (el default es **no**);
//! 5. `hf download` al staging;
//! 6. si `hf` dice OK pero el fichero no aparece, se busca por `basename`;
//! 7. el rename, comparando **rutas absolutas**;
//! 8. `rm -rf` del staging.
//!
//! ## Staging: sí para el GGUF, no para el checkpoint
//!
//! - **GGUF** (R-MOD-15): staging propio **dentro del destino**, para que
//!   el `mv` sea atómico y para que dos descargas con el mismo nombre de
//!   fichero no se pisen.
//! - **Checkpoint** (R-MOD-17): **sin** staging. `hf` es reanudable con
//!   `--local-dir` fijo, y un staging mataría la reanudación: una
//!   descarga de 12 GB cortada a medias debe **continuar**.
//!
//! ## La salida de `hf`
//!
//! Va a **stderr** (R-MOD-22): stdout es para los datos, y un install con
//! selección no debe parecer que escupe basura en vez de un modelo.

use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ropero_core::Environment;
use ropero_core::install::Via;

use crate::Error;
use crate::plan::{AuxPlan, Formato, MARCADOR, Plan, STAGING};
use crate::rutas::ruta_dentro;

/// Cómo se hace la descarga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// **Desatendido**: el usuario dio la lista, así que no se pregunta.
    Desatendido,
    /// **Interactivo**: se pregunta con el tamaño; el default es **no**.
    Interactivo,
}

/// Lo que pasó con un modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    /// Ya estaba: no se ha descargado nada.
    YaDescargado,
    /// Se ha descargado ahora.
    Descargado,
    /// El usuario no lo confirmó: no se descarga, y no es un error.
    Omitido,
}

/// El estado de un fichero final.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    /// Está y no está vacío.
    Presente,
    /// Está con el nombre que tiene en el repo: falta el renombrado.
    Renombrar,
    /// Falta.
    Falta,
}

/// Baja un modelo según su plan.
///
/// Es la entrada de la E/S: lanza `hf`, mueve ficheros y verifica lo que
/// hay en disco. Lo que se baja y con qué argumentos lo decide el
/// [`crate::plan`].
pub fn descarga(plan: &Plan, env: &Environment, modo: Modo) -> Result<Resultado, Error> {
    descarga_con(plan, env, modo, pregunta_en_terminal)
}

/// Baja un modelo según su plan, con la **pregunta** que se le pase.
///
/// El parámetro `confirmar` es lo que permite probar el modo interactivo
/// sin terminal: si devuelve `false`, el modelo se **omite** (y no es un
/// error).
pub fn descarga_con(
    plan: &Plan,
    env: &Environment,
    modo: Modo,
    mut confirmar: impl FnMut(&Plan) -> bool,
) -> Result<Resultado, Error> {
    // (1) El estado de cada cosa que hay que bajar: el artefacto principal
    // y los auxiliares (R-MOD-23).
    let mut principal = estado_principal(plan)?;
    let mut aux: Vec<(AuxPlan, Estado)> = Vec::new();
    for a in &plan.aux {
        let estado = estado_fichero(
            &a.dest,
            &plan.aux_dir,
            &a.file,
            &format!("[download.aux.{}].file", a.kind),
            &plan.modelo,
        )?;
        aux.push((a.clone(), estado));
    }

    // (2) El que está con el nombre del repo se renombra y se da por
    // bueno: no hace falta `hf` ni preguntar.
    if principal == Estado::Renombrar {
        renombra(
            &plan.dest_dir,
            &plan.file,
            &plan.dest,
            &plan.modelo,
            "[download].file",
        )?;
        principal = Estado::Presente;
    }
    for (a, estado) in aux.iter_mut() {
        if *estado == Estado::Renombrar {
            renombra(
                &plan.aux_dir,
                &a.file,
                &a.dest,
                &plan.modelo,
                &format!("[download.aux.{}].file", a.kind),
            )?;
            *estado = Estado::Presente;
        }
    }

    let falta = principal == Estado::Falta || aux.iter().any(|(_, e)| *e == Estado::Falta);
    if !falta {
        return Ok(Resultado::YaDescargado);
    }

    // R-MOD-21: un marcador sin pesos es «un directorio roto, no un modelo
    // instalado»: se borra y se vuelve a descargar.
    let marcador = plan.dest.join(MARCADOR);
    if plan.formato == Formato::Safetensors && marcador.is_file() && !tiene_pesos(&plan.dest) {
        let _ = std::fs::remove_file(&marcador);
    }

    // (3) `hf` ANTES de preguntar: prometer una descarga y luego confesar
    // que no se puede es la peor secuencia posible.
    let hf = buscar_hf(env).ok_or_else(|| Error::FaltaHf {
        modelo: plan.modelo.clone(),
    })?;

    // (4) La confirmación, con el tamaño. En modo desatendido no se
    // pregunta: el usuario ya dio la lista.
    if modo == Modo::Interactivo && !confirmar(plan) {
        return Ok(Resultado::Omitido);
    }

    // (5) `hf download`: el GGUF al staging y el checkpoint al destino
    // fijo (sin staging: es reanudable, R-MOD-17).
    if principal == Estado::Falta {
        match plan.formato {
            Formato::Gguf => baja_fichero(
                &hf,
                plan,
                &plan.file,
                &plan.dest,
                &plan.hf_args,
                "[download].file",
            )?,
            Formato::Safetensors => baja_checkpoint(&hf, plan)?,
        }
    }
    // Los auxiliares van al directorio del GGUF, también en un
    // checkpoint (R-MOD-23).
    for (a, estado) in &aux {
        if *estado == Estado::Falta {
            baja_fichero(
                &hf,
                plan,
                &a.file,
                &a.dest,
                &a.hf_args,
                &format!("[download.aux.{}].file", a.kind),
            )?;
        }
    }
    Ok(Resultado::Descargado)
}

/// El estado del artefacto principal.
///
/// - **GGUF**: el fichero final (presente y no vacío).
/// - **Checkpoint**: los pesos **y** el `config.json` (R-MOD-18).
fn estado_principal(plan: &Plan) -> Result<Estado, Error> {
    match plan.formato {
        Formato::Gguf => estado_fichero(
            &plan.dest,
            &plan.dest_dir,
            &plan.file,
            "[download].file",
            &plan.modelo,
        ),
        Formato::Safetensors => {
            if tiene_pesos(&plan.dest) && tiene_config(&plan.dest) {
                Ok(Estado::Presente)
            } else {
                Ok(Estado::Falta)
            }
        }
    }
}

/// Baja un checkpoint: `hf download` al **destino fijo**, sin staging.
///
/// Sin staging a propósito: `hf` es reanudable con `--local-dir` fijo, y
/// un staging mataría precisamente la reanudación (una descarga de
/// 12 GB cortada a medias debe **continuar**, no empezar de cero)
/// (R-MOD-17).
fn baja_checkpoint(hf: &Path, plan: &Plan) -> Result<(), Error> {
    let marcador = plan.dest.join(MARCADOR);
    std::fs::create_dir_all(&plan.dest).map_err(|e| Error::Disco {
        modelo: plan.modelo.clone(),
        que: "crear el directorio del checkpoint".to_string(),
        detalle: e.to_string(),
    })?;
    hf_descarga(hf, &plan.hf_args, &plan.modelo)?;

    // La **re-verificación**: aunque `hf` salga con 0, se comprueba que
    // el checkpoint está de verdad (R-MOD-19). Un exit 0 de un downloader
    // no es un modelo instalado.
    let mut falta = Vec::new();
    if !tiene_pesos(&plan.dest) {
        falta.push("faltan los pesos (*.safetensors / pytorch_model*.bin / *.npz)".to_string());
    }
    if !tiene_config(&plan.dest) {
        falta.push("falta config.json (¿repo equivocado?)".to_string());
    }
    if !falta.is_empty() {
        let _ = std::fs::remove_file(&marcador);
        return Err(Error::Incompleto {
            modelo: plan.modelo.clone(),
            destino: plan.dest.to_string_lossy().to_string(),
            falta,
        });
    }

    // El marcador, sólo si los pesos están de verdad.
    let _ = std::fs::write(&marcador, "");
    Ok(())
}

/// El estado de un fichero final.
///
/// - **Presente**: está y no está vacío.
/// - **Renombrar**: no está el final, pero sí el del repo (una descarga a
///   medio renombrar): con un `mv` se da por buena.
/// - **Falta**: no está ninguno de los dos.
///
/// La ruta del repo **se acota** al directorio de destino: un `..` o una
/// ruta absoluta podrían sacar el `mv` de fuera (ver [`crate::rutas`]).
fn estado_fichero(
    dest: &Path,
    dest_dir: &Path,
    file: &str,
    campo: &str,
    modelo: &str,
) -> Result<Estado, Error> {
    if es_fichero_no_vacio(dest) {
        return Ok(Estado::Presente);
    }
    let del_repo = ruta_dentro(dest_dir, file, campo, modelo)?;
    if del_repo != dest && es_fichero_no_vacio(&del_repo) {
        return Ok(Estado::Renombrar);
    }
    Ok(Estado::Falta)
}

/// Renombra el fichero que tiene el nombre del repo por el final.
fn renombra(
    dest_dir: &Path,
    file: &str,
    dest: &Path,
    modelo: &str,
    campo: &str,
) -> Result<(), Error> {
    let origen = ruta_dentro(dest_dir, file, campo, modelo)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Disco {
            modelo: modelo.to_string(),
            que: "crear el directorio de destino".to_string(),
            detalle: e.to_string(),
        })?;
    }
    std::fs::rename(&origen, dest).map_err(|e| Error::Disco {
        modelo: modelo.to_string(),
        que: format!("renombrar {origen:?} a {dest:?}"),
        detalle: e.to_string(),
    })
}

/// Baja un fichero con `hf` y lo deja en su ruta final.
///
/// El staging es **propio** y vive **dentro del destino** (R-MOD-15), así
/// que el `mv` es atómico. Se borra **siempre**, incluso si algo falla
/// (R-MOD-16.8).
fn baja_fichero(
    hf: &Path,
    plan: &Plan,
    file: &str,
    dest: &Path,
    args: &[String],
    campo: &str,
) -> Result<(), Error> {
    let dest_dir = dest.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dest_dir).map_err(|e| Error::Disco {
        modelo: plan.modelo.clone(),
        que: "crear el directorio de destino".to_string(),
        detalle: e.to_string(),
    })?;
    let staging = crea_staging(dest_dir, &plan.modelo)?;
    let resultado = baja_en_staging(hf, plan, file, dest, args, &staging, campo);
    // (8) `rm -rf` del staging: siempre, incluso en error.
    let _ = std::fs::remove_dir_all(&staging);
    resultado
}

/// La bajada en sí, dentro del staging.
fn baja_en_staging(
    hf: &Path,
    plan: &Plan,
    file: &str,
    dest: &Path,
    args: &[String],
    staging: &Path,
    campo: &str,
) -> Result<(), Error> {
    // (5) `hf download` al staging.
    hf_descarga(hf, &args_del_staging(args, staging), &plan.modelo)?;

    // (6) Si `hf` dice OK pero el fichero no aparece, se busca por
    // `basename`: hf a veces normaliza el nombre.
    let esperado = ruta_dentro(staging, file, campo, &plan.modelo)?;
    let descargado = if es_fichero_no_vacio(&esperado) {
        Some(esperado)
    } else {
        buscar_por_basename(staging, basename(file))
    };
    let Some(descargado) = descargado else {
        return Err(Error::HfSinFichero {
            modelo: plan.modelo.clone(),
            file: file.to_string(),
        });
    };

    // (7) El rename, comparando **rutas absolutas**: `mv X X` con cadenas
    // distintas (relativa vs absoluta) no es un no-op limpio.
    if !misma_ruta(&descargado, dest) {
        std::fs::rename(&descargado, dest).map_err(|e| Error::Disco {
            modelo: plan.modelo.clone(),
            que: format!("renombrar {descargado:?} a {dest:?}"),
            detalle: e.to_string(),
        })?;
    }

    // La re-verificación: un exit 0 de un downloader no es un modelo
    // instalado (R-MOD-19).
    if !es_fichero_no_vacio(dest) {
        return Err(Error::Incompleto {
            modelo: plan.modelo.clone(),
            destino: dest.to_string_lossy().to_string(),
            falta: vec![format!("falta {file}")],
        });
    }
    Ok(())
}

/// Los argumentos de `hf` con el staging real en lugar del marcador.
fn args_del_staging(args: &[String], staging: &Path) -> Vec<String> {
    args.iter()
        .map(|a| {
            if a == STAGING {
                staging.to_string_lossy().to_string()
            } else {
                a.clone()
            }
        })
        .collect()
}

/// Crea el staging: un directorio **dentro** del destino, con el prefijo
/// del contrato y un nombre único.
fn crea_staging(dest_dir: &Path, modelo: &str) -> Result<PathBuf, Error> {
    for intento in 0..64u32 {
        let sello = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let nombre = format!(".rp-dl.{}{sello}{intento}", std::process::id());
        let dir = dest_dir.join(nombre);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(Error::Disco {
                    modelo: modelo.to_string(),
                    que: "crear el staging".to_string(),
                    detalle: e.to_string(),
                });
            }
        }
    }
    Err(Error::Disco {
        modelo: modelo.to_string(),
        que: "crear el staging".to_string(),
        detalle: "no se ha encontrado un nombre libre".to_string(),
    })
}

/// Lanza `hf` con los argumentos del plan.
///
/// La salida de `hf` va a **stderr** (R-MOD-22): stdout es para los
/// datos del comando.
fn hf_descarga(hf: &Path, args: &[String], modelo: &str) -> Result<(), Error> {
    let mut cmd = Command::new(hf);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());
    let mut hijo = cmd.spawn().map_err(|e| Error::Disco {
        modelo: modelo.to_string(),
        que: "lanzar hf".to_string(),
        detalle: e.to_string(),
    })?;
    // Lo que `hf` escribe en stdout se copia a nuestro stderr.
    let salida = hijo.stdout.take();
    let manejador = std::thread::spawn(move || {
        if let Some(mut salida) = salida {
            let mut buffer = [0u8; 8192];
            let mut err = std::io::stderr();
            loop {
                match salida.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = err.write_all(&buffer[..n]);
                        let _ = err.flush();
                    }
                    Err(_) => break,
                }
            }
        }
    });
    let estado = hijo.wait().map_err(|e| Error::Disco {
        modelo: modelo.to_string(),
        que: "esperar a hf".to_string(),
        detalle: e.to_string(),
    })?;
    let _ = manejador.join();
    if estado.success() {
        Ok(())
    } else {
        Err(Error::HfFalló {
            modelo: modelo.to_string(),
            codigo: estado.code(),
        })
    }
}

/// Busca `hf` en el `PATH` del entorno.
fn buscar_hf(env: &Environment) -> Option<PathBuf> {
    let path = env.get("PATH")?;
    std::env::split_paths(path)
        .map(|dir| dir.join("hf"))
        .find(|candidato| es_ejecutable(candidato))
}

/// Si una ruta es un fichero ejecutable.
fn es_ejecutable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Si una ruta es un fichero **no vacío**.
fn es_fichero_no_vacio(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Si el directorio tiene **pesos** (R-MOD-18).
///
/// Los tres formatos que un engine sabe cargar (`*.safetensors`,
/// `pytorch_model*.bin`, `*.npz`) y, si hay más de un shard, el índice:
/// *un shard suelto NO cuenta*.
pub fn tiene_pesos(dir: &Path) -> bool {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut shards = 0usize;
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().to_string();
        let es_peso = nombre.ends_with(".safetensors")
            || (nombre.starts_with("pytorch_model") && nombre.ends_with(".bin"))
            || nombre.ends_with(".npz");
        if es_peso && entrada.path().is_file() {
            shards += 1;
        }
    }
    if shards == 0 {
        return false;
    }
    if shards > 1 {
        return dir.join("model.safetensors.index.json").is_file()
            || dir.join("pytorch_model.bin.index.json").is_file();
    }
    true
}

/// Si el directorio tiene el `config.json` (R-MOD-18).
fn tiene_config(dir: &Path) -> bool {
    dir.join("config.json").is_file()
}

/// El `basename` de un camino.
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Busca un fichero por su nombre dentro de un directorio (y sus
/// subdirectorios): hf a veces normaliza el nombre de lo que baja.
fn buscar_por_basename(dir: &Path, nombre: &str) -> Option<PathBuf> {
    let entradas = std::fs::read_dir(dir).ok()?;
    let mut subdirectorios = Vec::new();
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            subdirectorios.push(ruta);
        } else if ruta.file_name().map(|n| n == nombre).unwrap_or(false) {
            return Some(ruta);
        }
    }
    for sub in subdirectorios {
        if let Some(encontrado) = buscar_por_basename(&sub, nombre) {
            return Some(encontrado);
        }
    }
    None
}

/// Si dos rutas son **la misma** (comparadas en absoluto).
fn misma_ruta(a: &Path, b: &Path) -> bool {
    absoluta(a).components().eq(absoluta(b).components())
}

/// La ruta absoluta de un camino (sin tocar el disco).
fn absoluta(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// La pregunta de la descarga, con el tamaño.
///
/// El tamaño es **informativo** (R-MOD-20): no se verifica nada con él, sólo
/// se dice para que el usuario sepa lo que va a bajar.
pub fn pregunta_de_descarga(plan: &Plan) -> String {
    let tamano = if plan.size.is_empty() {
        "?"
    } else {
        &plan.size
    };
    format!("¿Descargar {} (~{tamano})? [s/N]: ", plan.modelo)
}

/// La pregunta en el terminal: el default es **no**.
///
/// Contesta `s`, `si`, `sí`, `y` o `yes` (sin distinguir mayúsculas). Lo
/// demás —incluido un EOF— es **no**.
fn pregunta_en_terminal(plan: &Plan) -> bool {
    let pregunta = pregunta_de_descarga(plan);
    let mut err = std::io::stderr();
    let _ = err.write_all(pregunta.as_bytes());
    let _ = err.flush();
    let mut linea = String::new();
    if std::io::stdin().lock().read_line(&mut linea).is_err() {
        return false;
    }
    matches!(
        linea.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    )
}

/// La línea de un modelo: nombre, vía y qué pasó.
pub fn linea(modelo: &str, via: Via, resultado: Resultado) -> String {
    let que = match resultado {
        Resultado::YaDescargado => "ya descargado",
        Resultado::Descargado => "descargado",
        Resultado::Omitido => "omitido por el usuario",
    };
    format!("{modelo}: vía {via} · {que}")
}

/// La línea de un modelo que **no** se pudo instalar.
pub fn linea_error(modelo: &str, via: Via, error: &Error) -> String {
    format!("{modelo}: vía {via} · error: {error}")
}
