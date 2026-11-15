//! La petición de `ropero gguffer` y sus **validaciones duras**.
//!
//! Las validaciones se comprueban **antes de nada** (R-GGUF-6: *antes de
//! nada* es antes de descargar, antes de mirar el motor, antes de todo):
//! bajan 60 GB tontos con la misma facilidad que un error de uso.
//!
//! El orden de la comprobación es el del doc (R-GGUF-3/5/6/7): la fuente
//! primero, después el imatrix, después las salidas.

use std::path::{Path, PathBuf};

use ropero_core::Environment;

use crate::Error;
use crate::quants;

/// De dónde salen los pesos: **una de tres**, excluyentes (R-GGUF-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fuente {
    /// Un repo de HuggingFace, a descargar: `owner/name`.
    Repo(String),
    /// Un directorio de pesos ya clonado.
    Path(PathBuf),
    /// Un GGUF existente: un fichero local o `owner/repo`.
    Gguf(String),
}

impl Fuente {
    /// Cómo se escribió, para los mensajes.
    pub fn describe(&self) -> String {
        match self {
            Fuente::Repo(repo) => format!("--repo {repo}"),
            Fuente::Path(path) => format!("--path {}", path.to_string_lossy()),
            Fuente::Gguf(gguf) => format!("--gguf {gguf}"),
        }
    }
}

/// El formato del GGUF **intermedio** (doc CLI v2 §11.2): `f16` o `bf16`,
/// con `f16` por defecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intermedio {
    /// El intermedio sale en F16 (el default).
    #[default]
    F16,
    /// El intermedio sale en BF16.
    Bf16,
}

impl Intermedio {
    /// El nombre que se le pasa al conversor (`--outtype`).
    pub fn outtype(self) -> &'static str {
        match self {
            Intermedio::F16 => "f16",
            Intermedio::Bf16 => "bf16",
        }
    }

    /// El sufijo que lleva el fichero del intermedio.
    pub fn sufijo(self) -> &'static str {
        match self {
            Intermedio::F16 => "F16",
            Intermedio::Bf16 => "BF16",
        }
    }
}

/// La importance matrix: un **fichero local** (`--imatrix`) o la **pareja
/// remota** (`--imatrix-repo` + `--imatrix-file`). Las dos formas son
/// excluyentes (R-GGUF-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Imatrix {
    /// El fichero, aquí.
    Local(PathBuf),
    /// El fichero, en un repo de HuggingFace.
    Remota { repo: String, file: String },
}

/// La petición del gguffer, tal y como la escribió el usuario.
///
/// Es **dato**, no ejecución (R-GGUF-2): lo que hace falta para planear la
/// tirada sin tocar el disco. Se construye con [`Peticion::de_flags`],
/// que es donde viven las validaciones duras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peticion {
    /// La fuente de los pesos (una de tres).
    pub fuente: Fuente,
    /// Los quants pedidos, ya partidos por coma y en mayúsculas.
    pub quants: Vec<String>,
    /// La importance matrix, si se pidió.
    pub imatrix: Option<Imatrix>,
    /// `--out NOMBRE|RUTA`: nombre, directorio o ruta exacta.
    pub out: Option<String>,
    /// `--out-principal RUTA`: la ruta exacta del primer quant.
    pub out_principal: Option<String>,
    /// El formato del intermedio (default `f16`).
    pub intermedio: Intermedio,
    /// No borra los pesos de origen.
    pub keep_source: bool,
    /// No borra el GGUF intermedio.
    pub keep_intermedio: bool,
    /// Genera el GGUF sin las cabeceras MTP.
    pub no_mtp: bool,
    /// Genera el GGUF sin las cabeceras DFlash.
    pub no_dflash: bool,
    /// Sobrescribe y fuerza.
    pub force: bool,
    /// Lista los quants disponibles y se sale.
    pub list_quants: bool,
    /// Diagnóstico del entorno: enseña qué haría, sin escribir nada.
    pub diagnostico: bool,
}

/// Los flags crudos del `gguffer`, tal y como los leyó la CLI.
///
/// Es el argumento único de [`Peticion::de_flags`]: la CLI los lee de
/// `clap`, los mete aquí y el dominio valida. Ningún flag se interpreta
/// en la CLI (R-ARQ-1: la lógica no vive allí).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flags {
    /// `--repo OWNER/NAME`.
    pub repo: Option<String>,
    /// `--path DIR`.
    pub path: Option<String>,
    /// `--gguf FICHERO|OWNER/REPO`.
    pub gguf: Option<String>,
    /// `--quant TIPO[,TIPO...]` (repetible).
    pub quants: Vec<String>,
    /// `--imatrix FICHERO`.
    pub imatrix: Option<String>,
    /// `--imatrix-repo OWNER/NAME`.
    pub imatrix_repo: Option<String>,
    /// `--imatrix-file NOMBRE`.
    pub imatrix_file: Option<String>,
    /// `--out NOMBRE|RUTA`.
    pub out: Option<String>,
    /// `--out-principal RUTA`.
    pub out_principal: Option<String>,
    /// `--intermedio f16|bf16`.
    pub intermedio: Option<String>,
    /// `--keep-source`.
    pub keep_source: bool,
    /// `--keep-intermedio`.
    pub keep_intermedio: bool,
    /// `--no-mtp`.
    pub no_mtp: bool,
    /// `--no-dflash`.
    pub no_dflash: bool,
    /// `--force`.
    pub force: bool,
    /// `--list-quants`.
    pub list_quants: bool,
    /// `--diagnostico`.
    pub diagnostico: bool,
}

impl Flags {
    /// `--repo OWNER/NAME`.
    pub fn con_repo(mut self, repo: &str) -> Flags {
        self.repo = Some(repo.to_string());
        self
    }

    /// `--path DIR`.
    pub fn con_path(mut self, path: &str) -> Flags {
        self.path = Some(path.to_string());
        self
    }

    /// `--gguf FICHERO|OWNER/REPO`.
    pub fn con_gguf(mut self, gguf: &str) -> Flags {
        self.gguf = Some(gguf.to_string());
        self
    }

    /// Los valores de `--quant` (repetible y con coma).
    pub fn con_quants(mut self, quants: &[&str]) -> Flags {
        self.quants = quants.iter().map(|q| q.to_string()).collect();
        self
    }

    /// `--imatrix FICHERO`.
    pub fn con_imatrix(mut self, imatrix: &str) -> Flags {
        self.imatrix = Some(imatrix.to_string());
        self
    }

    /// La pareja remota: `--imatrix-repo OWNER/NAME` + `--imatrix-file`.
    pub fn con_imatrix_remota(mut self, repo: &str, file: &str) -> Flags {
        self.imatrix_repo = Some(repo.to_string());
        self.imatrix_file = Some(file.to_string());
        self
    }

    /// `--out NOMBRE|RUTA`.
    pub fn con_out(mut self, out: &str) -> Flags {
        self.out = Some(out.to_string());
        self
    }

    /// `--out-principal RUTA`.
    pub fn con_out_principal(mut self, out: &str) -> Flags {
        self.out_principal = Some(out.to_string());
        self
    }

    /// `--intermedio f16|bf16`.
    pub fn con_intermedio(mut self, intermedio: &str) -> Flags {
        self.intermedio = Some(intermedio.to_string());
        self
    }

    /// `--diagnostico`.
    pub fn con_diagnostico(mut self) -> Flags {
        self.diagnostico = true;
        self
    }
}

impl Peticion {
    /// Monta la petición desde los **flags crudos** de la CLI, con las
    /// validaciones duras del doc (R-GGUF-3/5/6/7), **en orden**.
    ///
    /// - la fuente (`--repo`/`--path`/`--gguf`): **una sola**;
    /// - el imatrix: `--imatrix` excluyente con el par, y la pareja junta;
    /// - los quants: repetibles y con coma (R-GGUF-7);
    /// - las salidas: R-GGUF-4 y R-GGUF-5;
    /// - el intermedio: sólo `f16` o `bf16`.
    pub fn de_flags(flags: Flags) -> Result<Peticion, Error> {
        let Flags {
            repo,
            path,
            gguf,
            quants,
            imatrix,
            imatrix_repo,
            imatrix_file,
            out,
            out_principal,
            intermedio,
            keep_source,
            keep_intermedio,
            no_mtp,
            no_dflash,
            force,
            list_quants,
            diagnostico,
        } = flags;

        // ── (1) la fuente: exactamente una (R-GGUF-3) ──────────────────
        let fuentes: Vec<Fuente> = [
            repo.map(Fuente::Repo),
            path.map(|p| Fuente::Path(PathBuf::from(p))),
            gguf.map(Fuente::Gguf),
        ]
        .into_iter()
        .flatten()
        .collect();
        let fuente = match fuentes.len() {
            0 => {
                return Err(Error::Uso(
                    "falta la fuente: pasa --path DIR, --repo OWNER/NAME o \
                     --gguf FICHERO|OWNER/REPO (una sola)"
                        .to_string(),
                ));
            }
            1 => fuentes
                .into_iter()
                .next()
                .unwrap_or(Fuente::Repo(String::new())),
            _ => {
                let escritas: Vec<String> = fuentes.iter().map(Fuente::describe).collect();
                return Err(Error::Uso(format!(
                    "la fuente es una sola: {} son excluyentes",
                    escritas.join(", ")
                )));
            }
        };
        if let Fuente::Repo(repo) = &fuente
            && !repo.contains('/')
        {
            return Err(Error::Uso(format!(
                "--repo tiene que ser OWNER/NAME (recibido: '{repo}')"
            )));
        }

        // ── (2) el imatrix: las reglas duras (R-GGUF-6) ────────────────
        let imatrix = junta_imatrix(imatrix, imatrix_repo, imatrix_file)?;
        // Un imatrix sin `--quant` no tiene sentido: no hay nada que
        // cuantizar. La excepción es `--diagnostico`, que está exenta.
        if imatrix.is_some() && quants.is_empty() && !diagnostico {
            return Err(Error::Uso(
                "el imatrix no hace falta sin --quant: no hay nada que cuantizar. \
                 quítalo, o añade --quant TIPO"
                    .to_string(),
            ));
        }

        // ── (3) los quants: repetibles y con coma (R-GGUF-7) ───────────
        let quants = partir_quants(&quants);
        for quant in &quants {
            if quants::numero_de(quant).is_none() {
                return Err(Error::QuantInvalido {
                    quant: quant.clone(),
                });
            }
        }

        // ── (4) las salidas (R-GGUF-4 y R-GGUF-5) ──────────────────────
        // Con varios quants, `--out` no puede ser la ruta exacta de un
        // fichero: cada quant necesita la suya.
        if quants.len() > 1 && out.as_deref().is_some_and(|out| out.ends_with(".gguf")) {
            return Err(Error::Uso(
                "con varios quants, --out debe ser un nombre o un directorio \
                 (la ruta exacta es de un solo fichero)"
                    .to_string(),
            ));
        }
        // `--out-principal` exige una ruta de fichero `.gguf`.
        if let Some(principal) = out_principal.as_deref()
            && !principal.ends_with(".gguf")
        {
            return Err(Error::Uso(format!(
                "--out-principal necesita la ruta de un fichero .gguf \
                 (recibido: '{principal}')"
            )));
        }

        // ── (5) el intermedio: sólo f16 o bf16 ─────────────────────────
        let intermedio = match intermedio
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None | Some("f16") => Intermedio::F16,
            Some("bf16") => Intermedio::Bf16,
            Some(otro) => {
                return Err(Error::Uso(format!(
                    "--intermedio sólo admite f16 o bf16 (recibido: '{otro}')"
                )));
            }
        };

        Ok(Peticion {
            fuente,
            quants,
            imatrix,
            out,
            out_principal,
            intermedio,
            keep_source,
            keep_intermedio,
            no_mtp,
            no_dflash,
            force,
            list_quants,
            diagnostico,
        })
    }

    /// La petición en su forma **canónica**: lo que la hace ser *esta* tirada.
    ///
    /// Son las partes que cambian lo que se fabrica: la fuente, los quants
    /// **en el orden pedido**, el intermedio, el imatrix, las salidas y los
    /// interruptores que afectan al resultado (`--no-mtp`, `--no-dflash`,
    /// `--force`). Lo que no afecta al resultado (`--keep-source`,
    /// `--keep-intermedio`) no entra: dos peticiones que sólo se diferencian
    /// en la limpieza son la misma tirada.
    ///
    /// El **directorio de salida resuelto** entra (el `$GGUF_DIR` de verdad,
    /// no el texto del flag): dos peticiones que dicen lo mismo pero escriben
    /// en sitios distintos son tiradas distintas, y si la huella no lo dice,
    /// la segunda reutiliza el checkpoint de la primera, no fabrica nada y
    /// muere con «salida faltante».
    ///
    /// El directorio **de trabajo** (donde vive el checkpoint) **no** entra:
    /// es temporal, y si entrara la reanudación dejaría de funcionar.
    pub fn canon(&self, env: &Environment) -> String {
        let imatrix = match &self.imatrix {
            None => "ninguna".to_string(),
            Some(Imatrix::Local(fichero)) => format!("local:{}", fichero.to_string_lossy()),
            Some(Imatrix::Remota { repo, file }) => format!("remota:{repo}/{file}"),
        };
        [
            self.fuente.describe(),
            if self.quants.is_empty() {
                "sin quant".to_string()
            } else {
                self.quants.join(",")
            },
            self.intermedio.sufijo().to_string(),
            imatrix,
            self.out.clone().unwrap_or_else(|| "sin out".to_string()),
            self.out_principal
                .clone()
                .unwrap_or_else(|| "sin principal".to_string()),
            format!("out-dir={}", self.out_dir(env).to_string_lossy()),
            format!(
                "no-mtp={} no-dflash={} force={}",
                self.no_mtp, self.no_dflash, self.force
            ),
        ]
        .join(" | ")
    }

    /// La **huella** de la petición: un número fijo (FNV-1a, 64 bits) de su
    /// forma canónica.
    ///
    /// Es el valor que guarda el checkpoint (ver [`crate::fases::Checkpoint`]):
    /// si la huella del checkpoint no es la de la tirada actual, el checkpoint
    /// **no es de esta petición** y se descarta. Es un hash y no el texto: el
    /// JSON del checkpoint no se rompe con comillas o barras de una ruta, y dos
    /// peticiones distintas no se confunden por un accidente de parsing.
    pub fn huella(&self, env: &Environment) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.canon(env).bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    /// El **directorio de salida resuelto**: dónde van los GGUFs.
    ///
    /// - `--out-principal`: el directorio de esa ruta;
    /// - `--out` con ruta de fichero: el de esa ruta;
    /// - `--out` con directorio: ese;
    /// - `--out` con nombre (o sin `--out`): el de `$GGUF_DIR`.
    ///
    /// Es **resuelto** y no el texto del flag: es lo que decide dónde cae
    /// cada GGUF, y por eso va en la huella (ver [`Peticion::canon`]).
    pub fn out_dir(&self, env: &Environment) -> PathBuf {
        let base = env.env_path("GGUF_DIR", "models/gguf");
        if let Some(principal) = self.out_principal.as_deref() {
            return Path::new(principal)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| base.clone());
        }
        if let Some(out) = self.out.as_deref() {
            if out.ends_with(".gguf") {
                return Path::new(out)
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| base.clone());
            }
            if out.contains('/') {
                return PathBuf::from(out.trim_end_matches('/'));
            }
        }
        base
    }

    /// Si la petición pide **cuantizar de verdad**: hay quants que no son
    /// planos (F32/F16/BF16). Es lo que decide si hace falta el imatrix.
    pub fn pide_cuantizacion(&self) -> bool {
        self.quants.iter().any(|q| {
            quants::numero_de(q)
                .map(quants::es_quantizado)
                .unwrap_or(false)
        })
    }
}

/// La validación de una **consulta**: `--list-quants` y `--diagnostico`.
///
/// Son las mismas reglas duras que la tirada (fuente única, imatrix en su
/// forma, quants que existen, intermedio válido), con **dos excepciones**:
///
/// - **no hace falta fuente**: una consulta no fabrica nada, así que no hay
///   pesos de donde sacar;
/// - el imatrix **no** exige `--quant` (R-GGUF-6): el diagnóstico está
///   exento, porque preguntar por el entorno no es cuantizar.
///
/// Lo que **no** se valida aquí es lo que no tiene sentido en una
/// consulta: que la fuente exista, y que el imatrix esté.
pub fn valida_consulta(flags: &Flags) -> Result<(), Error> {
    // ── la fuente: una sola, pero ninguna vale ──────────────────────────
    let fuentes: Vec<Fuente> = [
        flags.repo.clone().map(Fuente::Repo),
        flags.path.clone().map(|p| Fuente::Path(PathBuf::from(p))),
        flags.gguf.clone().map(Fuente::Gguf),
    ]
    .into_iter()
    .flatten()
    .collect();
    if fuentes.len() > 1 {
        let escritas: Vec<String> = fuentes.iter().map(Fuente::describe).collect();
        return Err(Error::Uso(format!(
            "la fuente es una sola: {} son excluyentes",
            escritas.join(", ")
        )));
    }

    // ── el imatrix: excluyente con el par, y el par junto ─────────────
    junta_imatrix(
        flags.imatrix.clone(),
        flags.imatrix_repo.clone(),
        flags.imatrix_file.clone(),
    )?;

    // ── los quants: tienen que existir ─────────────────────────────────
    for quant in partir_quants(&flags.quants) {
        if quants::numero_de(&quant).is_none() {
            return Err(Error::QuantInvalido { quant });
        }
    }

    // ── el intermedio: sólo f16 o bf16 ──────────────────────────────────
    if let Some(intermedio) = flags.intermedio.as_deref() {
        let ok = intermedio.eq_ignore_ascii_case("f16") || intermedio.eq_ignore_ascii_case("bf16");
        if !ok {
            return Err(Error::Uso(format!(
                "--intermedio sólo admite f16 o bf16 (recibido: '{intermedio}')"
            )));
        }
    }

    Ok(())
}

/// Junta las tres formas del imatrix en la **una** que la petición lleva.
///
/// - `--imatrix` es **excluyente** con el par `--imatrix-repo` +
///   `--imatrix-file` (R-GGUF-6).
/// - El par va **siempre juntos**: usar sólo una de las dos es error.
fn junta_imatrix(
    imatrix: Option<String>,
    imatrix_repo: Option<String>,
    imatrix_file: Option<String>,
) -> Result<Option<Imatrix>, Error> {
    let (local, repo, file) = (
        imatrix.is_some(),
        imatrix_repo.is_some(),
        imatrix_file.is_some(),
    );
    if local && (repo || file) {
        return Err(Error::Uso(
            "--imatrix es excluyente con --imatrix-repo/--imatrix-file: \
             usa una de las dos formas"
                .to_string(),
        ));
    }
    if repo != file {
        return Err(Error::Uso(
            "--imatrix-repo y --imatrix-file van siempre en pareja: falta \
             la otra mitad del par"
                .to_string(),
        ));
    }
    match (imatrix, imatrix_repo, imatrix_file) {
        (Some(fichero), None, None) => Ok(Some(Imatrix::Local(PathBuf::from(fichero)))),
        (None, Some(repo), Some(file)) => Ok(Some(Imatrix::Remota { repo, file })),
        (None, None, None) => Ok(None),
        _ => Ok(None),
    }
}

/// Parte los quants de `--quant` (repetible y con coma, R-GGUF-7):
/// `["IQ2_M,IQ3_XS", "Q4_K_M"]` → `["IQ2_M", "IQ3_XS", "Q4_K_M"]`.
///
/// Los nombres no distinguen mayúsculas: el motor los imprime en
/// mayúsculas y el usuario puede escribir `q4_k_m`.
pub fn partir_quants(valores: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for valor in valores {
        for parte in valor.split(',') {
            let parte = parte.trim();
            if !parte.is_empty() {
                out.push(parte.to_ascii_uppercase());
            }
        }
    }
    out
}
