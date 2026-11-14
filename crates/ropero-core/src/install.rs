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
use std::path::PathBuf;

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

impl Via {
    /// La vía que dice una **respuesta** del menú.
    ///
    /// Se acepta la primera letra: `d` (download) o `b` (build).
    pub fn de_respuesta(texto: &str) -> Option<Self> {
        match texto.trim().chars().next() {
            Some('d') | Some('D') => Some(Self::Download),
            Some('b') | Some('B') => Some(Self::Build),
            _ => None,
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

/// Un token de la invocación de `install`: un modelo o un `--prio`.
///
/// El **orden** de los tokens es lo que decide el ámbito de cada `--prio`
/// (doc B §4.10.7), así que la invocación se conserva tal y como la
/// escribió el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Un modelo de la lista (puede traer varios separados por comas).
    Modelo(String),
    /// Un `--prio` con su valor.
    Prio(String),
}

/// Asocia los `--prio` de una invocación completa.
///
/// La lista puede traer **varios nombres en un token** (separados por
/// comas): cada nombre es un modelo, y el `--prio` que viene detrás se
/// aplica al **último** de ellos (su ámbito termina en el siguiente
/// modelo, doc B §4.10.7).
pub fn asocia_tokens(tokens: &[Token]) -> Result<Prios, Error> {
    let mut modelos = Vec::new();
    let mut prios = Vec::new();
    let mut posicion = 0usize;
    for token in tokens {
        match token {
            Token::Prio(valor) => {
                prios.push(PrioEnPosicion {
                    posicion,
                    valor: valor.clone(),
                });
                posicion += 1;
            }
            Token::Modelo(nombre) => {
                for n in parse_lista(std::slice::from_ref(nombre)) {
                    modelos.push(ModeloEnPosicion {
                        posicion,
                        nombre: n,
                    });
                    posicion += 1;
                }
            }
        }
    }
    asocia_prios(&modelos, &prios)
}

/// Los nombres de modelo de una invocación, en orden y sin repetidos.
pub fn nombres_de(tokens: &[Token]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        if let Token::Modelo(nombre) = token {
            for n in parse_lista(std::slice::from_ref(nombre)) {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// Los motores que hay que instalar, ya resueltos.
#[derive(Debug, Clone, PartialEq)]
pub struct MotoresElegidos<'a> {
    /// Los motores que se instalan.
    pub instalar: Vec<&'a crate::EngineRow>,
    /// Los avisos de los que se **omiten** (incompatibles, R-INST-26).
    pub avisos: Vec<String>,
}

/// Resuelve la lista de motores: ids o alias legacy.
///
/// Un motor **desconocido** es un error de uso; uno **incompatible** con
/// la máquina se **omite con aviso** (R-INST-26).
pub fn motores_elegidos<'a>(
    filas: &'a [crate::EngineRow],
    nombres: &[String],
) -> Result<MotoresElegidos<'a>, Error> {
    let mut instalar = Vec::new();
    let mut avisos = Vec::new();
    for nombre in nombres {
        let motor = resuelve_motor(nombre)?;
        let Some(fila) = filas.iter().find(|f| f.manifest.id == motor) else {
            return Err(Error::MotorDesconocido {
                nombre: nombre.clone(),
            });
        };
        if !fila.compatible {
            avisos.push(format!(
                "'{}' no encaja con esta máquina ({}); se omite",
                motor.as_str(),
                fila.reasons.join("; ")
            ));
            continue;
        }
        instalar.push(fila);
    }
    Ok(MotoresElegidos { instalar, avisos })
}

/// Un modelo de la lista, con su `--prio` y el aviso si se ignora.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeloElegido<'a> {
    /// El nombre con el que lo escribió el usuario.
    pub nombre: String,
    /// El modelo del catálogo.
    pub spec: &'a ModelSpec,
    /// El `--prio` que le toca a este modelo.
    pub prio: Option<Via>,
    /// El aviso de un `--prio` que se ignora.
    pub aviso: Option<String>,
}

/// Resuelve la lista de modelos con su `--prio`.
///
/// Un modelo **desconocido** es un error de uso; un `--prio` que se
/// ignora **no** lo es: sale en el aviso (doc B §4.10.7).
pub fn modelos_elegidos<'a>(
    cat: &'a Catalog,
    nombres: &[String],
    prios: &Prios,
) -> Result<Vec<ModeloElegido<'a>>, Error> {
    let mut out = Vec::new();
    for nombre in nombres {
        let spec = resuelve_modelo(cat, nombre)?;
        let prio = prios.de(nombre);
        let aviso =
            prio_ignorado(spec, prio).map(|ignorado| aviso_prio_ignorado(nombre, &ignorado));
        out.push(ModeloElegido {
            nombre: nombre.clone(),
            spec,
            prio,
            aviso,
        });
    }
    Ok(out)
}

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
///
/// Y sólo salen los que **no** son de descarga manual (R-DATA-3): un
/// `[download].manual = true` es un checkpoint enorme que el instalador no
/// baja por sorpresa (pero que sí se baja si se pide a mano).
pub fn menu_modelos(cat: &Catalog, motores: &[MotorDelMenu<'_>], paleta: &Palette) -> Menu {
    let mut menu = Menu::default();
    for motor in motores.iter().filter(|m| m.instalado) {
        let id = motor.manifest.id;
        let color = motor.manifest.color;
        let filas: Vec<&ModelSpec> = crate::catalog::catalog_rows(cat, false)
            .into_iter()
            .map(|(spec, _)| spec)
            .filter(|spec| spec.model.engine == id)
            .filter(|spec| !es_manual(spec))
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

/// Si un modelo es de **descarga manual** (R-DATA-3).
///
/// `manual = true` saca el modelo de la oferta de `ropero install
/// models`: son checkpoints enormes que el instalador no baja por
/// sorpresa. Si el usuario lo pide a mano, se baja igual.
pub fn es_manual(spec: &ModelSpec) -> bool {
    spec.download.as_ref().is_some_and(|d| d.manual)
}

/// Separa un **texto** en nombres.
///
/// Los separadores son los espacios, las tabulaciones y las comas; los
/// huecos vacíos se ignoran. Es lo que se usa para la **respuesta** de un
/// menú (una línea) y para el texto que el usuario escribe.
pub fn parse_texto(texto: &str) -> Vec<String> {
    texto
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|parte| parte.trim())
        .filter(|parte| !parte.is_empty())
        .map(|parte| parte.to_string())
        .collect()
}

/// Convierte la respuesta de un menú en la lista de nombres elegidos.
///
/// Admite los **números** del menú (1, 2, 3…) y los **nombres** (ids o
/// alias), separados por espacios o comas.
pub fn resuelve_seleccion(respuesta: &str, opciones: &[String]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    for token in parse_texto(respuesta) {
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
// Los mensajes
// ─────────────────────────────────────────────────────────────────────────────

/// El mensaje de `install engines` **sin terminal**.
///
/// Sin TTY no hay menú: un menú que se traga un EOF acabaría eligiendo
/// por su cuenta (R-INST-26). El mensaje dice **qué hacer**: pasar la
/// lista.
pub fn falta_tty_engines() -> String {
    "install engines necesita una terminal para el menú\n  \
     (pasa la lista: ropero install engines llama-server turbo-llama)"
        .to_string()
}

/// El mensaje de `install models` **sin terminal**.
pub fn falta_tty_models() -> String {
    "install models necesita una terminal para el menú\n  \
     (pasa la lista: ropero install models nemotron qwen)"
        .to_string()
}

/// El mensaje de un modelo con **las dos vías** sin `--prio` y sin terminal.
///
/// El mensaje dice **qué hacer**: pasar `--prio`, o dar una lista de
/// modelos en la que ninguno tenga las dos vías (doc B §4.10.7).
pub fn falta_prio(modelo: &str) -> String {
    format!(
        "el modelo '{modelo}' declara las dos vías y no hay terminal para preguntar\n  \
         (pasa --prio=download o --prio=build, o usa una lista de modelos en la que \
         ninguno tenga las dos vías)"
    )
}

/// La pregunta de qué vía se usa para un modelo.
pub fn pregunta_de_via(modelo: &str) -> String {
    format!("'{modelo}' se puede descargar o fabricar. ¿Cuál? [d]ownload / [b]uild: ")
}

/// El aviso de un `--prio` **ignorado**.
///
/// Ignorarlo no es un error y no aborta la instalación de los demás
/// modelos: sólo se dice (doc B §4.10.7).
pub fn aviso_prio_ignorado(modelo: &str, ignorado: &Ignorado) -> String {
    let motivo = match ignorado.motivo {
        Motivo::SinCamino => format!("'{modelo}' no tiene esa vía"),
        Motivo::YaEsElDefecto => format!("'{modelo}' sólo tiene esa vía: ya es su defecto"),
    };
    format!(
        "aviso: --prio={} se ignora: {motivo}",
        ignorado.via.as_str()
    )
}

/// Lo que pasó al instalar un motor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoMotor {
    /// Ya estaba enlazado en `$BIN_DIR`: **no se toca nada**.
    YaEnlazado(PathBuf),
    /// Se acaba de crear el enlace.
    Enlazado {
        /// El binario real.
        origen: PathBuf,
        /// El enlace creado.
        destino: PathBuf,
    },
    /// En el destino hay un fichero real: no se toca.
    Ocupado(PathBuf),
    /// No se encontró el binario del motor.
    SinBinario,
}

/// El mensaje y el **código de salida** de un motor instalado.
///
/// El mensaje va a stdout cuando la instalación fue bien (0) y a stderr
/// cuando no (1).
pub fn mensaje_motor(motor: &str, estado: &EstadoMotor) -> (String, i32) {
    match estado {
        EstadoMotor::YaEnlazado(destino) => {
            (motor_ya_enlazado(motor, &destino.to_string_lossy()), 0)
        }
        EstadoMotor::Enlazado { origen, destino } => (
            motor_enlazado(motor, &origen.to_string_lossy(), &destino.to_string_lossy()),
            0,
        ),
        EstadoMotor::Ocupado(destino) => (motor_ocupado(motor, &destino.to_string_lossy()), 1),
        EstadoMotor::SinBinario => (motor_sin_binario(motor), 1),
    }
}

/// El mensaje de un motor que **ya estaba enlazado**.
pub fn motor_ya_enlazado(motor: &str, destino: &str) -> String {
    format!("{motor}: ya está enlazado en {destino} (no se toca nada)")
}

/// El mensaje de un motor que **se acaba de enlazar**.
pub fn motor_enlazado(motor: &str, origen: &str, destino: &str) -> String {
    format!("{motor}: enlace creado · {destino} → {origen}")
}

/// El mensaje de un motor cuyo **binario no se encuentra**.
///
/// El mensaje dice **qué hacer** (P-13): el instalador sólo crea enlaces,
/// así que el binario lo pone el instalador del motor.
pub fn motor_sin_binario(motor: &str) -> String {
    format!(
        "no encuentro el binario de '{motor}': ni en el PATH, ni en el directorio de \
         binarios, ni en las rutas declaradas\n  \
         (instálalo con el instalador de su motor y vuelve a intentarlo: \
         ropero install engines sólo crea enlaces)"
    )
}

/// El mensaje de un motor cuyo destino **está ocupado** por un fichero real.
pub fn motor_ocupado(motor: &str, destino: &str) -> String {
    format!("{motor}: hay un fichero real en {destino}: no lo toco (quítalo o renómbralo)")
}

/// El mensaje de la vía `build`: todavía **no** se fabrica.
///
/// No dice "instalado" porque no lo está: la fabricación de un GGUF es
/// `ropero-forge`, que aún no existe (doc B §4.10.4). La vía `download`,
/// en cambio, **sí** baja el modelo de verdad.
pub fn via_build_pendiente(modelo: &str) -> String {
    format!("{modelo}: vía build · error: fabricar el GGUF todavía no está implementado")
}

/// La pista de la vía `build`: **qué hacer** (P-13).
pub fn pista_build_pendiente() -> String {
    "  (la fabricación de GGUFs es `ropero-forge`, que aún no existe: \
     usa la vía download, o fabrica el GGUF a mano con `ropero gguffer`)"
        .to_string()
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
