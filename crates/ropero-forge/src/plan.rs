//! El **plan** de fabricación: qué rutas, qué fases y qué se puede hacer
//! **antes** de bajar un byte.
//!
//! ## La viabilidad, antes de descargar (R-GGUF-8)
//!
//! Bajar los pesos es la parte cara e irreversible de la tirada (una hora
//! y decenas de GB). Si el quant pedido es distinto del que ya traen los
//! pesos, el motor no puede re-cuantizar (*requantizing from type X is
//! disabled*) y la tirada entera es trabajo para nada: por eso la
//! comprobación ocurre **antes** de la descarga, y el error **sugiere el
//! repo hermano sin cuantizar**.
//!
//! ## Las cuatro situaciones (R-GGUF-7, R-GGUF-9)
//!
//! 1. El GGUF ya está en el quant pedido → no se cuantiza. Ni imatrix, ni
//!    horas de CPU: el fichero ya está bien, se coloca y se sale 0.
//! 2. El GGUF está en **otro** quant → no se puede re-cuantizar: error
//!    **antes de descargar**, con el repo hermano sugerido.
//! 3. El GGUF no está cuantizado (F16/BF16) y el pedido es un quant de
//!    verdad → se cuantiza, y hace falta el imatrix.
//! 4. El GGUF no está cuantizado y el pedido es *plano* (F16/F32/BF16) →
//!    es una simple conversión de dtype: no hay quant que hacer, y por
//!    tanto no hace falta imatrix.
//!
//! ## De dónde sale la clase de los pesos
//!
//! La [`Clase`] se lee de la metadata del repo (`config.json`,
//! `hf_quant_config.json`): es lo que dice si los pesos vienen ya
//! cuantizados (NVFP4, MXFP4), si el conversor los dequantiza (FP8, AWQ,
//! GPTQ) o si vienen en claro. Para un `--repo` se baja **sólo** la
//! metadata (un par de KB), nunca los pesos.

use std::path::{Path, PathBuf};

use ropero_core::Environment;

use crate::Error;
use crate::peticion::{Fuente, Peticion};
use crate::quants;

/// Qué traen los pesos de fábrica, según su metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clase {
    /// No vienen cuantizados: el intermedio es F16/BF16 y se cuantiza de
    /// verdad (si se pide).
    #[default]
    Ninguno,
    /// NVFP4 (los de NVIDIA): el conversor **conserva** el quant.
    Nvfp4,
    /// MXFP4 (el gpt-oss): el conversor lo repackea a `MXFP4_MOE`.
    Mxfp4,
    /// Mezcla de quants de fábrica que el motor no sabe conservar.
    Mixto,
    /// FP8, AWQ, GPTQ: el conversor los **dequantiza** a F16. Se puede
    /// cuantizar desde ahí, avisando de que el quant de fábrica se pierde.
    Dequant,
}

impl Clase {
    /// El `llama_ftype` que se espera en el GGUF convertido.
    ///
    /// Es lo que el plan usa para saber si el quant pedido ya está en los
    /// pesos: NVFP4 → 39, MXFP4 → 38, y el resto lo que diga el
    /// intermedio (F16 = 1, BF16 = 32).
    pub fn ftype_esperado(self, intermedio: crate::peticion::Intermedio) -> u32 {
        match self {
            Clase::Nvfp4 | Clase::Mixto => 39,
            Clase::Mxfp4 => 38,
            Clase::Ninguno | Clase::Dequant => match intermedio {
                crate::peticion::Intermedio::F16 => 1,
                crate::peticion::Intermedio::Bf16 => 32,
            },
        }
    }

    /// El sufijo del intermedio: lo que lleva **dentro**, no lo que se
    /// pidió con `--intermedio` (un GGUF NVFP4 no es un F16, y llamarlo
    /// F16 hace que se crea que se puede reutilizar para otro quant).
    pub fn sufijo_intermedio(self, intermedio: crate::peticion::Intermedio) -> &'static str {
        match self {
            Clase::Nvfp4 | Clase::Mixto => "NVFP4",
            Clase::Mxfp4 => "MXFP4_MOE",
            Clase::Ninguno | Clase::Dequant => intermedio.sufijo(),
        }
    }

    /// El nombre canónico del quant que traen los pesos (R-GGUF-20: el
    /// mensaje final dice el quant **real**).
    pub fn nombre_quant(self, intermedio: crate::peticion::Intermedio) -> String {
        quants::nombre_de(self.ftype_esperado(intermedio))
            .unwrap_or("desconocido")
            .to_string()
    }
}

/// Qué se puede hacer con los pesos y los quants pedidos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viabilidad {
    /// Hay que cuantizar de verdad (alguno de los quants no está ya en los
    /// pesos).
    pub cuantizar: bool,
    /// Los quants que **sí** se cuantizan.
    pub faltan: Vec<String>,
    /// Los quants que **no** se tocan (ya están en ese quant).
    pub omitidos: Vec<String>,
    /// Los quants **planos** (F32/F16/BF16): es conversión de dtype, no
    /// cuantización (no exigen imatrix).
    pub planos: Vec<String>,
}

impl Viabilidad {
    /// Los quants que hacen falta, contados.
    pub fn falta_imatrix(&self) -> bool {
        self.cuantizar
    }
}

/// La viabilidad de la tirada, **calculada sin tocar el disco**.
///
/// Es el preflight de la fase 0 (R-GGUF-8): se llama con la clase de los
/// pesos y los quants pedidos. Si devuelve un [`Error::NoViable`], la
/// tirada muere **sin descargar nada**.
pub fn viabilidad(
    clase: Clase,
    intermedio: crate::peticion::Intermedio,
    quants_pedidos: &[String],
) -> Result<Viabilidad, Error> {
    let ftype = clase.ftype_esperado(intermedio);
    let tienen = clase.nombre_quant(intermedio);
    let mut plan = Viabilidad {
        cuantizar: false,
        faltan: Vec::new(),
        omitidos: Vec::new(),
        planos: Vec::new(),
    };
    for quant in quants_pedidos {
        let num = match quants::numero_de(quant) {
            Some(num) => num,
            None => {
                return Err(Error::QuantInvalido {
                    quant: quant.clone(),
                });
            }
        };
        if !quants::es_quantizado(num) {
            // (4) Un quant plano no es una cuantización: no hay imatrix
            // que traer. Pero tampoco se puede aplicar sobre pesos ya
            // cuantizados (eso sería dequantizar y re-cuantizar).
            if ftype != num && quants::es_quantizado(ftype) {
                return Err(no_viable(quant, &tienen));
            }
            plan.planos.push(quant.clone());
            continue;
        }
        // (1) Mismo quant: el GGUF ya está así. No se cuantiza otra vez.
        if ftype == num {
            plan.omitidos.push(quant.clone());
            continue;
        }
        // (2) Otro quant y el de partida ya es un quant: imposible.
        if quants::es_quantizado(ftype) {
            return Err(no_viable(quant, &tienen));
        }
        // (3) De verdad hay que cuantizar: hace falta el imatrix.
        plan.cuantizar = true;
        plan.faltan.push(quant.clone());
    }
    Ok(plan)
}

/// El error de una cuantización no viable (R-GGUF-8).
fn no_viable(pedido: &str, tienen: &str) -> Error {
    Error::NoViable {
        pedido: pedido.to_string(),
        tienen: tienen.to_string(),
        // La sugerencia la completa [`sugerencia_hermanos`], que necesita
        // el nombre del repo: aquí no está.
        sugerencia: Vec::new(),
    }
}

/// Los **repos hermanos sin cuantizar** de un repo (R-GGUF-8): si el repo
/// se llama `acme/modelo-NVFP4`, los hermanos son `acme/modelo-BF16` y
///
/// `acme/modelo-F16`.
///
/// Es una deducción del **nombre** (la que hacen los repos de
/// cuantizaciones, que se llaman `<modelo>-<QUANT>`); no hay llamada de
/// red. Sirve para sugerir la receta en el error: el repo hermano que sí
/// serviría.
pub fn sugerencia_hermanos(repo: &str) -> Vec<String> {
    let (owner, name) = match repo.rsplit_once('/') {
        Some((owner, name)) => (owner, name),
        None => return Vec::new(),
    };
    // Se quita el sufijo del quant (`-NVFP4`, `-BF16`, `-GGUF`, `-MXFP4`…)
    // y se proponen los dos formatos sin quant.
    let mut base = name.to_string();
    for sufijo in [
        "-GGUF",
        "-NVFP4",
        "-MXFP4",
        "-MXFP4_MOE",
        "-FP8",
        "-AWQ",
        "-GPTQ",
        "-F16",
        "-BF16",
    ] {
        if let Some(resto) = base.strip_suffix(sufijo) {
            base = resto.to_string();
            break;
        }
    }
    vec![
        format!("{owner}/{base}-BF16"),
        format!("{owner}/{base}-F16"),
    ]
}

/// El nombre base de un modelo, según de dónde salga.
///
/// - un **repo**: la hoja del repo (`acme/modelo` → `modelo`);
/// - un **directorio**: su nombre;
/// - un **GGUF**: su nombre sin la extensión `.gguf`, sin el sufijo
///   `-GGUF` de los repos de cuantizaciones, y sin el `.QUANT` final si
///   lo lleva (`modelo.Q4_K_M.gguf` → `modelo`).
///
/// Es el nombre con el que salen los GGUFs (`<nombre>-<QUANT>.gguf`) y el
/// que se busca para saber **qué hay publicado** de ese modelo.
pub fn nombre_de_fuente(fuente: &Fuente) -> String {
    match fuente {
        Fuente::Repo(repo) => repo.rsplit('/').next().unwrap_or(repo).to_string(),
        Fuente::Path(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        Fuente::Gguf(gguf) => {
            let fichero = gguf.rsplit('/').next().unwrap_or(gguf);
            let nombre = fichero
                .trim_end_matches(".gguf")
                .trim_end_matches("-GGUF")
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .trim_end_matches(['-', '.'])
                .to_string();
            nombre
                .rsplit_once('.')
                .filter(|(_, sufijo)| quants::numero_de(sufijo).is_some())
                .map(|(base, _)| base.to_string())
                .unwrap_or(nombre)
        }
    }
}

/// La ruta de la salida de un quant: `<nombre>-<QUANT>.gguf`, **sin
/// repetir** el sufijo si el nombre ya lo trae: pedir NVFP4 del repo
/// `acme/modelo-NVFP4` no produce `modelo-NVFP4-NVFP4` (R-GGUF-9: el
/// GGUF ya está en ese quant, se **coloca** bajo su propio nombre).
fn ruta_de_salida(dir: &Path, nombre: &str, quant: &str) -> PathBuf {
    if nombre.ends_with(&format!("-{quant}")) {
        dir.join(format!("{nombre}.gguf"))
    } else {
        dir.join(format!("{nombre}-{quant}.gguf"))
    }
}

/// Qué output se espera para cada quant de una tirada (R-GGUF-4/5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Salida {
    /// El quant, por **nombre**.
    pub quant: String,
    /// Su número de `llama_ftype`.
    pub numero: u32,
    /// La ruta exacta del GGUF de salida.
    pub ruta: PathBuf,
    /// Si es la salida **principal** (la del `--out-principal`).
    pub principal: bool,
}

/// El plan de la tirada: las rutas de todo lo que va a pasar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// El nombre base del GGUF final (el del repo, o el de `--out`).
    pub nombre: String,
    /// El directorio de trabajo (el clon de los pesos).
    pub work: PathBuf,
    /// Donde están los pesos.
    pub source_dir: PathBuf,
    /// El directorio de salida de los GGUFs.
    pub out_dir: PathBuf,
    /// La ruta del GGUF intermedio.
    pub intermedio: PathBuf,
    /// La salida de cada quant, en el orden pedido.
    pub salidas: Vec<Salida>,
    /// El sentinel del clon completo.
    pub sentinel: PathBuf,
}

impl Plan {
    /// El plan de la tirada, **puro**: no toca el disco.
    ///
    /// `clase` es la clase de los pesos (que para `--repo` se baja de la
    /// metadata antes de llamar aquí; para `--path` se lee del clon; para
    /// `--gguf` no hay conversión, así que vale la default).
    pub fn de(peticion: &Peticion, env: &Environment, clase: Clase) -> Result<Plan, Error> {
        // ── dónde vive cada cosa ───────────────────────────────────────
        let work_root = env
            .env_path("GGUF_WORK_DIR", ".cache/ropero/gguffer")
            .to_path_buf();
        let out_dir_base = env.env_path("GGUF_DIR", "models/gguf").to_path_buf();

        // ── el nombre base y el directorio de trabajo ─────────────────
        let mut nombre = nombre_de_fuente(&peticion.fuente);
        let (work, source_dir) = match &peticion.fuente {
            Fuente::Repo(repo) => {
                let work = work_root.join(repo.replace('/', "__"));
                (work.clone(), work)
            }
            Fuente::Path(path) => (path.clone(), path.clone()),
            Fuente::Gguf(gguf) => {
                // Un GGUF ya hecho no se clona: su work es un
                // directorio **propio** dentro del work root (el
                // checkpoint y la caché del imatrix viven ahí, y el
                // directorio del usuario no se toca nunca).
                let work = work_root.join(format!("gguf_{}", gguf.replace('/', "__")));
                (work.clone(), work)
            }
        };
        // `--out <nombre>` manda: el GGUF final se llama como se pidió.
        if let Some(out) = peticion.out.as_deref()
            && !out.ends_with(".gguf")
            && !out.contains('/')
        {
            nombre = out.to_string();
        }

        // ── el directorio de salida ────────────────────────────────────
        // `--out` con ruta de fichero: el directorio es el de esa ruta.
        // `--out` con directorio: ese. `--out-principal`: el del principal
        // (así el GGUF cae en la carpeta del modelo, que es lo que espera
        // el TOML).
        let out_dir = if let Some(principal) = peticion.out_principal.as_deref() {
            Path::new(principal)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| out_dir_base.clone())
        } else if let Some(out) = peticion.out.as_deref() {
            if out.ends_with(".gguf") {
                Path::new(out)
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| out_dir_base.clone())
            } else if out.contains('/') {
                PathBuf::from(out.trim_end_matches('/'))
            } else {
                out_dir_base.clone()
            }
        } else {
            out_dir_base.clone()
        };

        // ── el intermedio ──────────────────────────────────────────────
        // Se llama como lo que lleva dentro (ver [`Clase::sufijo_intermedio`]).
        let mut nombre_inter = nombre.clone();
        let sufijo = clase.sufijo_intermedio(peticion.intermedio);
        if !nombre_inter.ends_with(sufijo) {
            nombre_inter = format!("{nombre_inter}-{sufijo}");
        }
        let intermedio = out_dir.join(format!("{nombre_inter}.gguf"));

        // ── las salidas, una por quant ─────────────────────────────────
        let mut salidas = Vec::new();
        for (i, quant) in peticion.quants.iter().enumerate() {
            let numero = quants::numero_de(quant).unwrap_or(0);
            let ruta = if i == 0 {
                if let Some(principal) = peticion.out_principal.as_deref() {
                    PathBuf::from(principal)
                } else if let Some(out) = peticion.out.as_deref() {
                    if out.ends_with(".gguf") {
                        PathBuf::from(out)
                    } else if out.contains('/') {
                        ruta_de_salida(&PathBuf::from(out.trim_end_matches('/')), &nombre, quant)
                    } else if peticion.quants.len() == 1 {
                        // Un solo quant y `--out` como nombre: ese es el
                        // nombre del fichero.
                        out_dir.join(format!("{out}.gguf"))
                    } else {
                        ruta_de_salida(&out_dir, &nombre, quant)
                    }
                } else {
                    ruta_de_salida(&out_dir, &nombre, quant)
                }
            } else {
                out_dir.join(format!("{nombre}-{quant}.gguf"))
            };
            salidas.push(Salida {
                quant: quant.clone(),
                numero,
                ruta,
                principal: i == 0 && peticion.out_principal.is_some(),
            });
        }
        // Sin quants, la salida es el propio intermedio: se convierte
        // directamente ahí, en vez de convertir a un F16 aparte y luego
        // renombrar.
        if salidas.is_empty() {
            salidas.push(Salida {
                quant: clase.nombre_quant(peticion.intermedio),
                numero: clase.ftype_esperado(peticion.intermedio),
                ruta: intermedio.clone(),
                principal: false,
            });
        }

        Ok(Plan {
            sentinel: work.join(".ropero-forge-clon-completo"),
            nombre,
            work,
            source_dir,
            out_dir,
            intermedio,
            salidas,
        })
    }
}
