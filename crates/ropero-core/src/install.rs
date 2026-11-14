//! El contrato de `--prio` y la decisión de vía de instalación.
//!
//! ## Qué es este módulo
//!
//! Es el **núcleo puro** de `ropero install` (doc B §4.10.7): el valor de
//! `--prio`, su ámbito (global o por modelo), su precedencia, cuándo se
//! ignora, y la decisión de qué vía se usa para cada modelo.
//!
//! ## Por qué es puro
//!
//! Nada de aquí lee disco, consulta el entorno ni lanza procesos
//! (doc B §3.2.3): son **decisiones**, y una decisión se puede probar con
//! entradas. Por eso el contrato de `--prio` tiene un test que lo recorre
//! entero sin tocar el sistema.
//!
//! ## El contrato, en corto (doc B §4.10.7)
//!
//! - El **valor** es `download` o `build`; cualquier otro es error de uso
//!   (código 2) y el mensaje dice el valor recibido.
//! - El `--prio` **global** va **antes** de la lista; el **por modelo** va
//!   **detrás** del modelo y su ámbito **termina en el siguiente modelo**.
//! - Con los dos, **manda el por modelo**; el global sólo aplica a los
//!   modelos que no tienen prio propio.
//! - Un `--prio` **se ignora** (no es error y no aborta) si el modelo no
//!   tiene ese camino, o si sólo tiene un camino y el `--prio` es ese.
//! - Sin `--prio` y con las dos vías, se **pregunta**; sin TTY es error de
//!   uso (código 2).

use std::collections::BTreeMap;
use std::fmt;

use crate::EngineId;
use crate::EngineManifest;
use crate::ModelSpec;
use crate::Palette;
use crate::catalog::Catalog;
use crate::catalog::real_name;

// ─────────────────────────────────────────────────────────────────────────────
// La vía
// ─────────────────────────────────────────────────────────────────────────────

/// La vía por la que se instala un modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Via {
    /// La vía de **descarga**: el modelo ya está publicado y se baja.
    Download,
    /// La vía de **fabricación**: se construye el GGUF desde los pesos.
    Build,
}

impl Via {
    /// El nombre de la vía, tal y como lo escribe el `--prio`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::Build => "build",
        }
    }

    /// Parsea el valor de un `--prio`.
    ///
    /// Sólo valen `download` y `build`. Cualquier otra cosa es un **error
    /// de uso** cuyo mensaje lleva el **valor recibido** (doc B §4.10.7).
    pub fn parse(texto: &str) -> Result<Self, Error> {
        match texto.trim() {
            "download" => Ok(Self::Download),
            "build" => Ok(Self::Build),
            otro => Err(Error::PrioInvalido {
                valor: otro.to_string(),
            }),
        }
    }
}

impl fmt::Display for Via {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Las vías que declara un modelo.
///
/// Un modelo puede declarar **las dos**: eso es un estado válido del TOML
/// (doc B §4.1.1, R-MOD-29), y es justo el caso en el que hay que elegir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vias {
    /// El modelo declara `[download]`.
    pub download: bool,
    /// El modelo declara `[build]`.
    pub build: bool,
}

impl Vias {
    /// Si el modelo no declara ninguna vía.
    pub const fn ninguna(self) -> bool {
        !self.download && !self.build
    }

    /// Si el modelo declara esa vía.
    pub const fn tiene(self, via: Via) -> bool {
        match via {
            Via::Download => self.download,
            Via::Build => self.build,
        }
    }

    /// La **única** vía del modelo, si tiene exactamente una.
    pub const fn unica(self) -> Option<Via> {
        match (self.download, self.build) {
            (true, false) => Some(Via::Download),
            (false, true) => Some(Via::Build),
            _ => None,
        }
    }
}

/// Las vías que declara un modelo, según su TOML.
pub fn vias_de(spec: &ModelSpec) -> Vias {
    Vias {
        download: spec.download.is_some(),
        build: spec.build.is_some(),
    }
}

/// La decisión de vía de un modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Se usa esta vía.
    Via(Via),
    /// El modelo declara las dos vías: hay que **preguntar**.
    Pregunta,
}

/// Decide la vía con la que se instala un modelo.
///
/// - Un modelo con **un solo** camino usa ese camino, y el `--prio` **se
///   ignora** (doc B §4.10.7: ya es su comportamiento por defecto).
/// - Un modelo con **las dos** vías usa la del `--prio`; sin `--prio`, hay
///   que preguntar ([`Decision::Pregunta`]).
/// - Un modelo **sin** ninguna vía es un error: no se puede instalar.
pub fn elegir_via(spec: &ModelSpec, prio: Option<Via>) -> Result<Decision, Error> {
    let vias = vias_de(spec);
    if vias.ninguna() {
        return Err(Error::SinVias {
            modelo: real_name(spec),
        });
    }
    if let Some(unica) = vias.unica() {
        return Ok(Decision::Via(unica));
    }
    match prio {
        Some(via) => Ok(Decision::Via(via)),
        None => Ok(Decision::Pregunta),
    }
}

/// Por qué se ha ignorado un `--prio`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// El modelo **no tiene** esa vía.
    SinCamino,
    /// El modelo **sólo tiene** esa vía: el `--prio` ya es su defecto.
    YaEsElDefecto,
}

/// Un `--prio` que se ha ignorado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ignorado {
    /// La vía que se pidió y no se aplicó.
    pub via: Via,
    /// Por qué se ignoró.
    pub motivo: Motivo,
}

/// El `--prio` que se ignora para este modelo, si hay alguno.
///
/// Ignorarlo **no** es un error y **no** aborta la instalación de los demás
/// modelos (doc B §4.10.7): sólo cambia la línea de aviso.
pub fn prio_ignorado(spec: &ModelSpec, prio: Option<Via>) -> Option<Ignorado> {
    let prio = prio?;
    let vias = vias_de(spec);
    if !vias.tiene(prio) {
        return Some(Ignorado {
            via: prio,
            motivo: Motivo::SinCamino,
        });
    }
    if vias.unica().is_some() {
        return Some(Ignorado {
            via: prio,
            motivo: Motivo::YaEsElDefecto,
        });
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// La asociación de los `--prio` con los modelos
// ─────────────────────────────────────────────────────────────────────────────

/// Un modelo de la línea de comandos, con **su posición**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloEnPosicion {
    /// La posición del token en la invocación.
    pub posicion: usize,
    /// El nombre con el que lo escribió el usuario.
    pub nombre: String,
}

/// Un `--prio` de la línea de comandos, con **su posición** y su valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrioEnPosicion {
    /// La posición del token en la invocación.
    pub posicion: usize,
    /// El valor crudo, tal y como lo escribió el usuario.
    pub valor: String,
}

/// Los `--prio` de una invocación, ya asociados a los modelos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prios {
    /// El `--prio` **global**: el que va antes de la lista de modelos.
    pub global: Option<Via>,
    /// Los `--prio` **por modelo**.
    pub por_modelo: BTreeMap<String, Via>,
}

impl Prios {
    /// La vía que se pide para un modelo.
    ///
    /// **Manda la por modelo**: el global sólo aplica a los modelos que no
    /// tienen prio propio (doc B §4.10.7).
    pub fn de(&self, modelo: &str) -> Option<Via> {
        self.por_modelo.get(modelo).copied().or(self.global)
    }
}

/// Asocia los `--prio` a los modelos según **su posición**.
///
/// - Un `--prio` **antes del primer modelo** es **global**.
/// - Un `--prio` **después** de algún modelo se aplica al **último modelo
///   que lo precede**: su ámbito termina en el siguiente modelo.
/// - Con los dos a la vez, **manda el por modelo** ([`Prios::de`]).
/// - Un valor que no es `download` ni `build` es un **error de uso** con el
///   valor recibido.
pub fn asocia_prios(
    modelos: &[ModeloEnPosicion],
    prios: &[PrioEnPosicion],
) -> Result<Prios, Error> {
    let mut orden: Vec<&PrioEnPosicion> = prios.iter().collect();
    orden.sort_by_key(|p| p.posicion);
    let mut global = None;
    let mut por_modelo: BTreeMap<String, Via> = BTreeMap::new();
    for prio in orden {
        let via = Via::parse(&prio.valor)?;
        let anterior = modelos
            .iter()
            .filter(|m| m.posicion < prio.posicion)
            .max_by_key(|m| m.posicion);
        match anterior {
            None => global = Some(via),
            Some(modelo) => {
                por_modelo.insert(modelo.nombre.clone(), via);
            }
        }
    }
    Ok(Prios { global, por_modelo })
}

// ─────────────────────────────────────────────────────────────────────────────
// La lista de ids y alias
// ─────────────────────────────────────────────────────────────────────────────

/// Separa una lista de ids o alias en sus nombres.
///
/// Los separadores son los **espacios** y las **comas**; los huecos vacíos
/// se ignoran (doc CLI v2 §9.1).
pub fn parse_lista(tokens: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for token in tokens {
        for parte in token.split(',') {
            let nombre = parte.trim();
            if !nombre.is_empty() {
                out.push(nombre.to_string());
            }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// La resolución de nombres
// ─────────────────────────────────────────────────────────────────────────────

/// Resuelve un nombre —id, alias o nombre real— a un modelo del catálogo.
///
/// Un nombre lo puede reclamar un modelo de cuatro maneras:
///
/// - por su **id** (`[model].name`),
/// - por su **alias** (`[model].alias`, que el catálogo materializa como
///   fila extra),
/// - por su **nombre real** (el del TOML de origen),
/// - por una clave de su `[aliases]`.
///
/// Si **dos** modelos reclaman el mismo nombre, es un error que **nombra el
/// conflicto** (R-MOD-9).
pub fn resuelve_modelo<'a>(cat: &'a Catalog, nombre: &str) -> Result<&'a ModelSpec, Error> {
    let candidatos: Vec<&ModelSpec> = cat
        .models
        .iter()
        .chain(cat.fired.iter())
        .filter(|spec| reclama(spec, nombre))
        .collect();
    let mut reales: Vec<String> = Vec::new();
    for spec in &candidatos {
        let real = real_name(spec);
        if !reales.contains(&real) {
            reales.push(real);
        }
    }
    match reales.len() {
        0 => Err(Error::ModeloDesconocido {
            nombre: nombre.to_string(),
        }),
        1 => Ok(elegido(&candidatos, &reales[0])),
        _ => Err(Error::AliasEnConflicto {
            alias: nombre.to_string(),
            modelos: reales,
        }),
    }
}

/// Si el modelo reclama ese nombre.
fn reclama(spec: &ModelSpec, nombre: &str) -> bool {
    spec.model.name == nombre
        || spec.model.alias.as_deref() == Some(nombre)
        || real_name(spec) == nombre
        || spec.aliases.contains_key(nombre)
}

/// El modelo que se devuelve: la fila **real**, no la del alias.
fn elegido<'a>(candidatos: &[&'a ModelSpec], real: &str) -> &'a ModelSpec {
    candidatos
        .iter()
        .find(|spec| spec.model.name == real)
        .copied()
        .or_else(|| candidatos.first().copied())
        .unwrap_or_else(|| unreachable!("un modelo resuelto tiene al menos un candidato"))
}

/// Resuelve un nombre de motor —id o alias legacy— a su id canónico.
pub fn resuelve_motor(nombre: &str) -> Result<EngineId, Error> {
    EngineId::from_str_legacy(nombre)
        .map(|(motor, _)| motor)
        .map_err(|_| Error::MotorDesconocido {
            nombre: nombre.to_string(),
        })
}

// ─────────────────────────────────────────────────────────────────────────────
// Los menús
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor del menú de instalación.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotorDelMenu<'a> {
    /// El manifiesto del motor.
    pub manifest: &'a EngineManifest,
    /// Si el motor está instalado en esta máquina.
    pub instalado: bool,
}

/// Un menú: las líneas que se enseñan y los nombres, en el mismo orden.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Menu {
    /// Las líneas del menú, ya numeradas.
    pub lineas: Vec<String>,
    /// Los nombres de las opciones, en el orden de la numeración.
    pub nombres: Vec<String>,
}

/// El menú de los motores.
///
/// Sólo salen los motores **compatibles** con la máquina (R-CTX-1) y el
/// color es el de la leyenda (doc CLI v2 §4).
pub fn menu_engines(motores: &[MotorDelMenu<'_>], paleta: &Palette) -> Menu {
    let mut menu = Menu::default();
    for (indice, motor) in motores.iter().enumerate() {
        let manifest = motor.manifest;
        let nombre = paleta.paint(manifest.color, manifest.id.as_str());
        let estado = if motor.instalado {
            "  ya instalado"
        } else {
            ""
        };
        menu.lineas.push(format!(
            "  {:>2}  {:<14} {:<12} {:<10}{}",
            indice + 1,
            nombre,
            manifest.color.as_str(),
            manifest.port_range.to_string(),
            estado
        ));
        menu.nombres.push(manifest.id.as_str().to_string());
    }
    menu
}

/// El menú de los modelos.
///
/// Sólo salen los modelos de los motores **instalados** (T-25): un modelo
/// de un motor que no está en la máquina no se puede instalar.
pub fn menu_modelos(cat: &Catalog, motores: &[MotorDelMenu<'_>], paleta: &Palette) -> Menu {
    let mut menu = Menu::default();
    for motor in motores.iter().filter(|m| m.instalado) {
        let id = motor.manifest.id;
        let color = motor.manifest.color;
        let filas: Vec<&ModelSpec> = crate::catalog::catalog_rows(cat, false)
            .into_iter()
            .map(|(spec, _)| spec)
            .filter(|spec| spec.model.engine == id)
            .collect();
        if filas.is_empty() {
            continue;
        }
        menu.lineas
            .push(format!("  {}", paleta.paint(color, id.as_str())));
        for spec in filas {
            let indice = menu.nombres.len() + 1;
            let descripcion = crate::catalog::model_description(spec);
            let descripcion = if descripcion.is_empty() {
                "-".to_string()
            } else {
                descripcion
            };
            menu.lineas.push(format!(
                "  {:>2}  {:<28}{}",
                indice, spec.model.name, descripcion
            ));
            menu.nombres.push(spec.model.name.clone());
        }
        menu.lineas.push(String::new());
    }
    while menu.lineas.last().is_some_and(|l| l.is_empty()) {
        menu.lineas.pop();
    }
    menu
}

/// Convierte la respuesta de un menú en la lista de nombres elegidos.
///
/// Admite los **números** del menú (1, 2, 3…) y los **nombres** (ids o
/// alias), separados por espacios o comas.
pub fn resuelve_seleccion(respuesta: &str, opciones: &[String]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    for token in parse_lista(&[respuesta.to_string()]) {
        match token.parse::<usize>() {
            Ok(numero) if numero >= 1 && numero <= opciones.len() => {
                out.push(opciones[numero - 1].clone());
            }
            Ok(numero) => {
                return Err(Error::SeleccionFueraDeRango {
                    valor: numero.to_string(),
                });
            }
            Err(_) => out.push(token),
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Los errores
// ─────────────────────────────────────────────────────────────────────────────

/// Los errores del contrato de instalación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// El valor de un `--prio` no es `download` ni `build`.
    PrioInvalido {
        /// El valor recibido.
        valor: String,
    },
    /// El modelo no declara ni `[download]` ni `[build]`.
    SinVias {
        /// El modelo que no declara ninguna vía.
        modelo: String,
    },
    /// El modelo no está en el catálogo.
    ModeloDesconocido {
        /// El nombre que se pidió.
        nombre: String,
    },
    /// El motor no existe (ni es id canónico ni alias legacy).
    MotorDesconocido {
        /// El nombre que se pidió.
        nombre: String,
    },
    /// Dos modelos reclaman el mismo nombre (R-MOD-9).
    AliasEnConflicto {
        /// El nombre en conflicto.
        alias: String,
        /// Los modelos que lo reclaman.
        modelos: Vec<String>,
    },
    /// La respuesta del menú no señala ninguna opción.
    SeleccionFueraDeRango {
        /// Lo que se escribió.
        valor: String,
    },
}

impl Error {
    /// El código de salida del error: **2** (uso) o **1** (ejecución).
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::SinVias { .. } => 1,
            _ => 2,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PrioInvalido { valor } => write!(
                f,
                "valor de --prio desconocido: '{valor}' (usa download o build)"
            ),
            Error::SinVias { modelo } => write!(
                f,
                "el modelo '{modelo}' no declara ni [download] ni [build]: \
                 no hay forma de instalarlo"
            ),
            Error::ModeloDesconocido { nombre } => {
                write!(f, "modelo desconocido: '{nombre}' (ropero list)")
            }
            Error::MotorDesconocido { nombre } => {
                write!(f, "motor desconocido: '{nombre}' (ropero engines)")
            }
            Error::AliasEnConflicto { alias, modelos } => write!(
                f,
                "el alias '{alias}' lo reclaman dos modelos: {}",
                modelos.join(" y ")
            ),
            Error::SeleccionFueraDeRango { valor } => write!(
                f,
                "selección fuera de rango: '{valor}' (elige un número de la lista)"
            ),
        }
    }
}

impl std::error::Error for Error {}
