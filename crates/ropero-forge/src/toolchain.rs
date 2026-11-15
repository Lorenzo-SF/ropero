//! La **precondición del motor** (R-GGUF-54…57): sin llama.cpp no hay
//! conversión, ni cuantización, ni unión de shards.
//!
//! ## Qué busca y dónde
//!
//! El motor se busca en **`$LLAMA_DIR`** (por defecto `~/llama.cpp`, el
//! fork) y en **`$LLAMA_OFFICIAL_DIR`** (por defecto
//! `~/llama.cpp-official`, el oficial): los dos motores no dejan el clon
//! en el mismo sitio, y la v2 mira **los dos** (R-GGUF-57). De ahí salen:
//!
//! | Qué | Dónde |
//! |---|---|
//! | el conversor | `<dir>/convert_hf_to_gguf.py` (**sin alternativa**) |
//! | el cuantizador | el `PATH` → si no, `<dir>/build/bin/llama-quantize` |
//! | el unidor de shards | el `PATH` → si no, `<dir>/build/bin/llama-gguf-split` |
//!
//! ## El conversor no admite sustituto (R-GGUF-55)
//!
//! Sin `convert_hf_to_gguf.py` **no hay conversión**, y por tanto no hay
//! GGUF — con o sin quant. El cuantizador y el unidor sí tienen
//! alternativa en el `PATH`; el conversor, no.
//!
//! ## La precondición va ANTES de descargar (R-GGUF-56, T-29)
//!
//! La función [`ensure_toolchain`] es **pura**: entra lo que hay, sale el
//! motor o el error — que **nombra el motor que falta** y **la variable
//! que lo apunta**. El mensaje falso de la v1 («faltan ficheros en el
//! clon: la descarga está incompleta») empujaba a borrar 60 GB por un
//! problema que no era de la descarga (H-27); este mensaje no.

use std::path::{Path, PathBuf};

use crate::Error;

/// Qué motor se encontró.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motor {
    /// El fork: `$LLAMA_DIR` (por defecto `~/llama.cpp`).
    Fork { dir: PathBuf },
    /// El oficial: `$LLAMA_OFFICIAL_DIR` (por defecto
    /// `~/llama.cpp-official`).
    Oficial { dir: PathBuf },
}

impl Motor {
    /// Su nombre, para los mensajes.
    pub fn nombre(&self) -> &'static str {
        match self {
            Motor::Fork { .. } => "llama.cpp (el fork)",
            Motor::Oficial { .. } => "llama.cpp (el oficial)",
        }
    }

    /// Su directorio.
    pub fn dir(&self) -> &Path {
        match self {
            Motor::Fork { dir } | Motor::Oficial { dir } => dir,
        }
    }
}

/// El motor que la tirada va a usar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    /// Qué motor es.
    pub motor: Motor,
    /// El conversor: `<motor>/convert_hf_to_gguf.py`.
    pub convert: PathBuf,
    /// El cuantizador: el `PATH` o `<motor>/build/bin/llama-quantize`.
    pub quantize: PathBuf,
    /// El unidor de shards, si está (no es imprescindible: sólo une).
    pub split: Option<PathBuf>,
}

/// Lo que el entorno ofrece al [`ensure_toolchain`]: candidatos ya
/// **comprobados** contra el disco por quien llama.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Busqueda {
    /// El directorio del fork (`$LLAMA_DIR`), si está.
    pub dir_fork: Option<PathBuf>,
    /// El directorio del oficial (`$LLAMA_OFFICIAL_DIR`), si está.
    pub dir_oficial: Option<PathBuf>,
    /// El conversor que se encontró en el fork, si está.
    pub convert_fork: Option<PathBuf>,
    /// El conversor que se encontró en el oficial, si está.
    pub convert_oficial: Option<PathBuf>,
    /// El cuantizador que se encontró en el `PATH`, si está.
    pub quantize_en_path: Option<PathBuf>,
    /// El cuantizador del build del fork, si está.
    pub quantize_fork: Option<PathBuf>,
    /// El cuantizador del build del oficial, si está.
    pub quantize_oficial: Option<PathBuf>,
    /// El unidor de shards que se encontró en el `PATH`, si está.
    pub split_en_path: Option<PathBuf>,
    /// El unidor de shards del build del motor, si está (se rellena
    /// después de elegir el motor, por eso es una sola ruta).
    pub split_en_build: Option<PathBuf>,
}

/// La precondición del motor: **pura**, sin tocar el disco.
///
/// Con la búsqueda hecha ([`Busqueda`]), decide qué motor se usa y qué
/// falta. El orden es: el fork primero (es el que ropero instala por
/// defecto), el oficial después (R-GGUF-57).
pub fn ensure_toolchain(busqueda: &Busqueda) -> Result<Toolchain, Error> {
    // ── el conversor: sin sustituto (R-GGUF-55) ────────────────────────
    let (motor, convert) = if let Some(convert) = busqueda.convert_fork.clone() {
        (
            Motor::Fork {
                dir: busqueda.dir_fork.clone().unwrap_or_default(),
            },
            convert,
        )
    } else if let Some(convert) = busqueda.convert_oficial.clone() {
        (
            Motor::Oficial {
                dir: busqueda.dir_oficial.clone().unwrap_or_default(),
            },
            convert,
        )
    } else {
        return Err(Error::SinMotor {
            falta: "convert_hf_to_gguf.py (el conversor de pesos a GGUF)".to_string(),
            donde: format!(
                "se buscó en $LLAMA_DIR ({}) y en $LLAMA_OFFICIAL_DIR ({})",
                describe_dir(busqueda.dir_fork.as_deref(), "~/llama.cpp"),
                describe_dir(busqueda.dir_oficial.as_deref(), "~/llama.cpp-official"),
            ),
        });
    };

    // ── el cuantizador: el PATH primero, luego el build del motor ──────
    let quantize = if let Some(quantize) = busqueda.quantize_en_path.clone() {
        quantize
    } else {
        let del_build = match motor {
            Motor::Fork { .. } => busqueda.quantize_fork.clone(),
            Motor::Oficial { .. } => busqueda.quantize_oficial.clone(),
        };
        match del_build {
            Some(quantize) => quantize,
            None => {
                return Err(Error::MotorIncompleto {
                    falta: "llama-quantize (el cuantizador)".to_string(),
                    donde: format!(
                        "no está en el PATH ni en {}/build/bin",
                        motor.dir().to_string_lossy()
                    ),
                });
            }
        }
    };

    // ── el unidor de shards: útil, no imprescindible ──────────────────
    let split = busqueda
        .split_en_path
        .clone()
        .or_else(|| busqueda.split_en_build.clone());

    Ok(Toolchain {
        motor,
        convert,
        quantize,
        split,
    })
}

/// Describe un directorio para el mensaje: lo que hay, o su default.
fn describe_dir(dir: Option<&Path>, defecto: &str) -> String {
    match dir {
        Some(dir) => dir.to_string_lossy().into_owned(),
        None => defecto.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// La búsqueda en el disco (la parte de E/S de la precondición)
// ─────────────────────────────────────────────────────────────────────────────

/// Busca el motor en el disco y llama a [`ensure_toolchain`].
///
/// Es la parte de E/S de la precondición: comprueba los candidatos contra
/// el disco (que es lo que [`ensure_toolchain`] no hace) y le pasa el
/// resultado. `env` trae `$LLAMA_DIR`, `$LLAMA_OFFICIAL_DIR`, `$HOME` y
/// el `PATH`.
pub fn buscar(env: &ropero_core::Environment) -> Result<Toolchain, Error> {
    let dir_fork = dir_del_motor(env, "LLAMA_DIR", "llama.cpp");
    let dir_oficial = dir_del_motor(env, "LLAMA_OFFICIAL_DIR", "llama.cpp-official");

    let mut busqueda = Busqueda {
        dir_fork: dir_fork.clone(),
        dir_oficial: dir_oficial.clone(),
        convert_fork: dir_fork
            .as_ref()
            .map(|dir| dir.join("convert_hf_to_gguf.py"))
            .filter(|candidato| candidato.is_file()),
        convert_oficial: dir_oficial
            .as_ref()
            .map(|dir| dir.join("convert_hf_to_gguf.py"))
            .filter(|candidato| candidato.is_file()),
        quantize_en_path: buscar_en_path(env, "llama-quantize"),
        split_en_path: buscar_en_path(env, "llama-gguf-split"),
        ..Busqueda::default()
    };
    // El cuantizador del build del motor que toque: se comprueba en los
    // dos, y [`ensure_toolchain`] elige el del motor que salga.
    busqueda.quantize_fork = dir_fork
        .as_ref()
        .map(|dir| dir.join("build/bin/llama-quantize"))
        .filter(|candidato| candidato.is_file());
    busqueda.quantize_oficial = dir_oficial
        .as_ref()
        .map(|dir| dir.join("build/bin/llama-quantize"))
        .filter(|candidato| candidato.is_file());
    busqueda.split_en_build = dir_fork
        .as_ref()
        .map(|dir| dir.join("build/bin/llama-gguf-split"))
        .filter(|candidato| candidato.is_file())
        .or_else(|| {
            dir_oficial
                .as_ref()
                .map(|dir| dir.join("build/bin/llama-gguf-split"))
                .filter(|candidato| candidato.is_file())
        });

    ensure_toolchain(&busqueda)
}

/// El directorio de un motor: la variable de entorno, o el default bajo
/// `$HOME`.
fn dir_del_motor(
    env: &ropero_core::Environment,
    variable: &str,
    default_bajo_home: &str,
) -> Option<PathBuf> {
    if let Some(dir) = env.get(variable) {
        return Some(PathBuf::from(dir));
    }
    if env.home.as_os_str().is_empty() {
        return None;
    }
    let dir = env.home.join(default_bajo_home);
    dir.is_dir().then_some(dir)
}

/// Busca un ejecutable en el `PATH` del entorno.
pub fn buscar_en_path(env: &ropero_core::Environment, herramienta: &str) -> Option<PathBuf> {
    let path = env.get("PATH")?;
    std::env::split_paths(path)
        .map(|dir| dir.join(herramienta))
        .find(|candidato| es_ejecutable(candidato))
}

/// Si una ruta es un fichero ejecutable.
pub fn es_ejecutable(ruta: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(ruta)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// El informe del `--diagnostico`: qué motor se encuentra, dónde, qué
/// herramientas hay y qué falta.
///
/// Es **sólo lectura**: un diagnóstico no toca nada (doc CLI v2 §11.2).
pub fn diagnostico(env: &ropero_core::Environment) -> Vec<String> {
    let dir_fork = dir_del_motor(env, "LLAMA_DIR", "llama.cpp");
    let dir_oficial = dir_del_motor(env, "LLAMA_OFFICIAL_DIR", "llama.cpp-official");
    let mut lineas = vec![
        format!(
            "  $LLAMA_DIR:           {}",
            describe_dir(dir_fork.as_deref(), "~/llama.cpp (no está)")
        ),
        format!(
            "  $LLAMA_OFFICIAL_DIR:  {}",
            describe_dir(dir_oficial.as_deref(), "~/llama.cpp-official (no está)")
        ),
    ];
    for (dir, nombre) in [
        (dir_fork.as_ref(), "el fork"),
        (dir_oficial.as_ref(), "el oficial"),
    ] {
        let Some(dir) = dir else { continue };
        let convert = dir.join("convert_hf_to_gguf.py");
        let quantize = dir.join("build/bin/llama-quantize");
        let split = dir.join("build/bin/llama-gguf-split");
        lineas.push(format!("  motor {nombre} ({}):", dir.to_string_lossy()));
        lineas.push(format!(
            "    conversor convert_hf_to_gguf.py: {}",
            si_esta(&convert)
        ));
        lineas.push(format!(
            "    cuantizador llama-quantize:      {}",
            si_esta(&quantize)
        ));
        lineas.push(format!(
            "    unidor llama-gguf-split:        {}",
            si_esta(&split)
        ));
    }
    lineas.push(format!(
        "  en el PATH: llama-quantize {}, llama-gguf-split {}",
        buscar_en_path(env, "llama-quantize")
            .map(|r| format!("({})", r.to_string_lossy()))
            .unwrap_or_else(|| "no está".to_string()),
        buscar_en_path(env, "llama-gguf-split")
            .map(|r| format!("({})", r.to_string_lossy()))
            .unwrap_or_else(|| "no está".to_string()),
    ));
    match buscar(env) {
        Ok(toolchain) => lineas.push(format!(
            "  => se usaría {} ({}), con el cuantizador {}",
            toolchain.motor.nombre(),
            toolchain.motor.dir().to_string_lossy(),
            toolchain.quantize.to_string_lossy()
        )),
        Err(error) => {
            lineas.push("  => falta motor:".to_string());
            for linea in error.to_string().lines() {
                lineas.push(format!("     {linea}"));
            }
            if let Some(pista) = error.pista() {
                lineas.push(format!("     qué hacer: {pista}"));
            }
        }
    }
    lineas
}

/// «está» / «NO está», para el diagnóstico.
fn si_esta(ruta: &Path) -> String {
    if ruta.is_file() {
        format!("está ({})", ruta.to_string_lossy())
    } else {
        "NO está".to_string()
    }
}
