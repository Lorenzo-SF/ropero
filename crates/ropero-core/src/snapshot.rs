//! La vista del **resumen** (`ropero` a secas) y del **listado por motor**
//! (`ropero list`).
//!
//! Es la parte del dominio que decide **qué se ve** y **cómo se ve**. La
//! E/S (buscar un binario en el `PATH`, mirar si un fichero existe) vive
//! fuera: aquí entra como **dato**, ya capturado.
//!
//! ## La distinción entre "disponible" e "instalado"
//!
//! Son dos preguntas distintas y el doc CLI v2 §6 las separa:
//!
//! - **Disponible**: lo que se *puede* instalar aquí. Para un motor, que su
//!   manifiesto esté cargado; para un modelo, que su TOML esté en el
//!   catálogo.
//! - **Instalado**: lo que *ya está* en la máquina. Para un motor, que su
//!   `detect` lo encuentre (binario en el `PATH` o ruta de instalación); para
//!   un modelo, que sus ficheros `[files]` existan en disco.
//!
//! La detección es E/S y vive en la CLI y en el runtime; aquí sólo se
//! **compone** el texto.

use std::collections::BTreeMap;

use crate::EngineId;
use crate::EngineManifest;
use crate::ModelSpec;
use crate::catalog;

/// El estado de un motor en la máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnginePresence {
    /// El motor está instalado: su `detect` lo encontró.
    Installed,
    /// El motor no está instalado.
    Missing,
}

/// El estado de un modelo en la máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPresence {
    /// El modelo está descargado: sus ficheros `[files]` existen.
    Downloaded,
    /// Al modelo le faltan ficheros.
    Missing,
}

/// Un motor, con lo que se sabe de él.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineState {
    /// El motor.
    pub manifest: EngineManifest,
    /// Si está instalado.
    pub presence: EnginePresence,
}

/// Un modelo, con lo que se sabe de él.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelState {
    /// El modelo del catálogo.
    pub spec: ModelSpec,
    /// Si sus ficheros están en disco.
    pub presence: ModelPresence,
}

/// La foto de la máquina que el resumen y el listado necesitan.
///
/// Es un **dato**, no una fuente: quien la construye es la capa de
/// presentación, que es la que sabe mirar el `PATH` y el disco.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Los motores, con su presencia.
    pub engines: Vec<EngineState>,
    /// Los modelos, con su presencia.
    pub models: Vec<ModelState>,
}

impl Snapshot {
    /// Construye la foto a partir de los motores y los modelos.
    pub fn new(engines: Vec<EngineState>, models: Vec<ModelState>) -> Self {
        Snapshot { engines, models }
    }

    /// Los motores instalados.
    pub fn engines_installed(&self) -> Vec<&EngineState> {
        self.engines
            .iter()
            .filter(|e| e.presence == EnginePresence::Installed)
            .collect()
    }

    /// Los modelos cuyos ficheros están en disco.
    pub fn models_downloaded(&self) -> Vec<&ModelState> {
        self.models
            .iter()
            .filter(|m| m.presence == ModelPresence::Downloaded)
            .collect()
    }
}

/// La respuesta a "¿está esto instalado?".
///
/// Es la pregunta de E/S, y por eso **entra por parámetro**: el dominio
/// decide qué hacer con la respuesta, pero no la pide.
pub trait Detect {
    /// Si el motor está instalado.
    fn engine_installed(&self, manifest: &EngineManifest) -> bool;
    /// Si el modelo está descargado.
    fn model_downloaded(&self, spec: &ModelSpec) -> bool;
}

/// Construye la foto de la máquina.
///
/// `detect` es la frontera de E/S: aquí sólo se **mapea** su respuesta al
/// dominio. La decisión de qué es "instalado" y qué es "descargado" es de
/// la capa de E/S; la de qué **significa** es de aquí.
pub fn build_snapshot(
    manifiestos: &[EngineManifest],
    modelos: &[ModelSpec],
    detect: &dyn Detect,
) -> Snapshot {
    let engines = manifiestos
        .iter()
        .map(|manifest| EngineState {
            presence: if detect.engine_installed(manifest) {
                EnginePresence::Installed
            } else {
                EnginePresence::Missing
            },
            manifest: manifest.clone(),
        })
        .collect();
    let models = modelos
        .iter()
        .map(|spec| ModelState {
            presence: if detect.model_downloaded(spec) {
                ModelPresence::Downloaded
            } else {
                ModelPresence::Missing
            },
            spec: spec.clone(),
        })
        .collect();
    Snapshot::new(engines, models)
}

/// Las razones de incompatibilidad de los motores, por motor.
///
/// Vienen del informe de compatibilidad (F2): un motor que no encaja con
/// esta máquina dice **por qué** en vez de callarse.
pub fn razones_por_motor(
    manifiestos: &[EngineManifest],
    sistema: &crate::SystemInfo,
) -> BTreeMap<EngineId, String> {
    let informe = crate::evaluate_manifest_set(manifiestos, sistema);
    let mut razones: BTreeMap<EngineId, String> = BTreeMap::new();
    for r in informe.incompatibles {
        let motivos = r.reasons_text();
        if !motivos.is_empty() {
            razones.insert(r.manifest.id, motivos.join("; "));
        }
    }
    razones
}

/// Los modelos **disponibles**: los que se pueden servir ahora mismo.
///
/// "Disponible" son las dos cosas a la vez (doc CLI v2 §15): el motor está
/// **instalado** y el modelo está **descargado**.
pub fn available_models(snapshot: &Snapshot) -> Vec<&ModelState> {
    let instalados: Vec<EngineId> = snapshot
        .engines_installed()
        .iter()
        .map(|e| e.manifest.id)
        .collect();
    snapshot
        .models_downloaded()
        .into_iter()
        .filter(|m| instalados.contains(&m.spec.model.engine))
        .collect()
}

/// Los grupos del listado: un motor y los modelos que le sirven.
///
/// Los grupos salen en el **orden canónico** de la leyenda (ADR-004), no en
/// el orden del disco: el listado es estable.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineGroup {
    /// El motor del grupo.
    pub manifest: EngineManifest,
    /// Si el motor está instalado.
    pub installed: bool,
    /// Por qué el motor no se puede usar aquí (`None` si está instalado).
    ///
    /// La razón la aporta el informe de compatibilidad (F2) cuando el motor
    /// es incompatible con la máquina; si es compatible pero no está
    /// instalado, la razón es la ausencia.
    pub reason: Option<String>,
    /// Los modelos del grupo.
    pub models: Vec<ModelState>,
}

/// Los grupos del listado, con los modelos **disponibles**.
///
/// `todos` incluye también los modelos que **no** están disponibles (el
/// `--all`), marcados con su estado. Un motor que no está instalado no
/// aporta modelos disponibles: su grupo sale **plegado** (vacío), y el
/// listado dice por qué.
///
/// `razones` son las incompatibilidades conocidas (F2): si un motor no
/// encaja con esta máquina, su grupo plegado dice por qué, y no sólo que no
/// está instalado.
pub fn engine_groups(
    snapshot: &Snapshot,
    todos: bool,
    razones: &BTreeMap<EngineId, String>,
) -> Vec<EngineGroup> {
    let disponibles = available_models(snapshot);
    let mut grupos: Vec<EngineGroup> = Vec::new();
    for engine in &snapshot.engines {
        let installed = engine.presence == EnginePresence::Installed;
        let mut models: Vec<ModelState> = disponibles
            .iter()
            .filter(|m| m.spec.model.engine == engine.manifest.id)
            .map(|m| (*m).clone())
            .collect();
        if todos {
            for modelo in &snapshot.models {
                if modelo.spec.model.engine == engine.manifest.id
                    && !models
                        .iter()
                        .any(|m| m.spec.model.name == modelo.spec.model.name)
                {
                    models.push(modelo.clone());
                }
            }
        }
        let reason = if installed {
            None
        } else {
            Some(
                razones
                    .get(&engine.manifest.id)
                    .cloned()
                    .unwrap_or_else(|| "no está instalado".to_string()),
            )
        };
        grupos.push(EngineGroup {
            manifest: engine.manifest.clone(),
            installed,
            reason,
            models,
        });
    }
    grupos
}

/// Los grupos con **al menos un** modelo, ordenados por el id canónico.
pub fn groups_with_models(grupos: &[EngineGroup]) -> Vec<&EngineGroup> {
    grupos.iter().filter(|g| !g.models.is_empty()).collect()
}

/// Los grupos **plegados**: los que no aportan ningún modelo disponible.
pub fn collapsed_groups(grupos: &[EngineGroup]) -> Vec<&EngineGroup> {
    grupos.iter().filter(|g| g.models.is_empty()).collect()
}

/// El número de modelos de un grupo.
pub fn group_count(grupo: &EngineGroup) -> usize {
    grupo.models.len()
}

/// El mapa de motor → modelo, para agrupar por motor sin repetir el
/// `match` en varios sitios.
pub fn by_engine(modelos: &[ModelState]) -> BTreeMap<EngineId, Vec<&ModelState>> {
    let mut mapa: BTreeMap<EngineId, Vec<&ModelState>> = BTreeMap::new();
    for modelo in modelos {
        mapa.entry(modelo.spec.model.engine)
            .or_default()
            .push(modelo);
    }
    mapa
}

// ─────────────────────────────────────────────────────────────────────────────
// El resumen (`ropero` a secas)
// ─────────────────────────────────────────────────────────────────────────────

/// Los cuatro números del resumen: motores y modelos, disponibles e
/// instalados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Motores con manifiesto cargado.
    pub motores_disponibles: usize,
    /// Motores instalados.
    pub motores_instalados: usize,
    /// Modelos del catálogo.
    pub modelos_disponibles: usize,
    /// Modelos descargados.
    pub modelos_instalados: usize,
}

/// Cuenta lo que hay, a partir de la foto.
///
/// Los modelos se cuentan **una vez**: un alias no es un modelo más (doc CLI
/// v2 §6.5: los modelos disponibles son los TOML del catálogo, y los
/// instalados los descargados).
pub fn counts(snapshot: &Snapshot) -> Counts {
    Counts {
        motores_disponibles: snapshot.engines.len(),
        motores_instalados: snapshot.engines_installed().len(),
        modelos_disponibles: catalog::cuenta_unicos(snapshot.models.iter().map(|m| &m.spec)),
        modelos_instalados: catalog::cuenta_unicos(
            snapshot.models_downloaded().into_iter().map(|m| &m.spec),
        ),
    }
}

/// El texto del resumen de `ropero` a secas.
///
/// Los rótulos y el esqueleto son de [`crate::surface::resumen_esqueleto`];
/// aquí sólo se rellenan los números. La función es **pura**: recibe los
/// datos y la versión, y devuelve el texto.
pub fn resumen_text(snapshot: &Snapshot, version: &str, build_date: &str, author: &str) -> String {
    let c = counts(snapshot);
    crate::surface::resumen_esqueleto()
        .replace("{version}", version)
        .replace("{fecha}", build_date)
        .replace("{autor}", author)
        .replace("{motores_disponibles}", &c.motores_disponibles.to_string())
        .replace("{motores_instalados}", &c.motores_instalados.to_string())
        .replace("{modelos_disponibles}", &c.modelos_disponibles.to_string())
        .replace("{modelos_instalados}", &c.modelos_instalados.to_string())
}
