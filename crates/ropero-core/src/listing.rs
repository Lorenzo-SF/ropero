//! La salida de `ropero models`: los modelos **disponibles**, agrupados por
//! motor, con la leyenda de colores.
//!
//! ## La forma
//!
//! ```text
//! MODELOS
//!   modelo      tamaño   artefacto
//!   ─────────────────────────────────────────
//!   fastflowllm
//!   d3mini      ~3.50 GB  d3:mini
//!   ─────────────────────────────────────────
//!   llama-server
//!   groot-o     ~13.60 GB  Groot-19B-A3B-261023-NVFP4.gguf
//!   nemotron-o  ~18.90 GB  NVIDIA-Nemotron-3.5-Lightning-30B-A…
//!   ─────────────────────────────────────────
//!   turbo-llama
//!   qwen38      ~12.10 GB  Qwen3.8-27B-GSQ-RCO-IQ3_S-mtp.gguf
//! ```
//!
//! La cabecera sale **una sola vez**: los motores son filas de grupo, y una
//! regla los separa. Así el listado se lee como una tabla y no como una
//! lista de tablas.
//!
//! ## Los grupos plegados
//!
//! Un motor instalado sin modelos disponibles sale **plegado**, con esa nota
//! debajo. Los motores no instalados sólo se incluyen con `--all`.
//!
//! ```text
//!   airllm
//!     no está instalado
//!   mlx-serve
//!     sólo funciona en macOS; necesita Apple Silicon con Metal
//! ```
//!
//! Los motivos de todos los grupos caen en la misma columna: se leen como
//! una columna, no como un texto pegado a cada nombre.
//!
//! ## El artefacto es el que dice el formato
//!
//! El `d3mini` no tiene fichero: su artefacto es el **tag** del almacén de
//! FLM (`d3:mini`), y eso es lo que sale. El formato dice dónde vive cada
//! artefacto (R-MOD-6): el fichero (`gguf`), el directorio del checkpoint
//! (`safetensors`), el tag del almacén (`q4nx`) o el paquete del catálogo
//! (`strata`).
//!
//! ## Por qué el tamaño y el artefacto van en columnas distintas
//!
//! El catálogo declara las dos cosas, pero no siempre: un modelo con
//! `[download]` declara un **tamaño** y uno con `[build]` declara el
//! **nombre del fichero**. Metidos en la misma columna, el nombre largo del
//! artefacto **empujaba** al tamaño y las dos dejaban de alinearse
//! (doc CLI v2 §2). Separadas, cada una tiene su ancho y su alineación: el
//! tamaño a la derecha (es un número), el artefacto a la izquierda.
//!
//! ## La estética
//!
//! Es la del doc CLI v2 §2 y la **comparte** con el resto de comandos: el
//! sistema de formato vive en [`crate::estilo`] y aquí sólo se decide *qué*
//! va en cada columna.

use std::collections::BTreeMap;

use crate::EngineId;
use crate::ModelSpec;
use crate::estilo::{ANCHO_MAXIMO, Celda, Columna, SIN_DATO, Seccion, Tabla};
use crate::format::ArtifactLocation;
use crate::palette::Palette;
use crate::snapshot::{EngineGroup, ModelPresence, ModelState, Snapshot, engine_groups};

/// El texto de un recurso que no se puede usar.
const NO_DISPONIBLE: &str = "no disponible";

/// El título de la sección de los modelos.
const TITULO: &str = "MODELOS";

/// Las líneas de `ropero models`.
///
/// `todos` es el `--all`: añade los modelos que **no** están disponibles
/// (marcados) y despliega los grupos plegados.
pub fn list_lines(
    snapshot: &Snapshot,
    todos: bool,
    palette: &Palette,
    razones: &BTreeMap<EngineId, String>,
) -> Vec<String> {
    list_lines_lang(snapshot, todos, palette, razones, false)
}

/// Variante localizada para la salida del CLI.
pub fn list_lines_lang(
    snapshot: &Snapshot,
    todos: bool,
    palette: &Palette,
    razones: &BTreeMap<EngineId, String>,
    english: bool,
) -> Vec<String> {
    let grupos = engine_groups(snapshot, todos, razones);
    // Una tabla para todo el listado: el motor es una fila de grupo y una
    // regla separa un grupo del siguiente.
    let mut tabla = Tabla::nivel(1)
        .columna(Columna::texto(if english { "model" } else { "modelo" }).maximo(ANCHO_MAXIMO))
        .columna(Columna::numero(if english { "size" } else { "tamaño" }))
        .columna(
            Columna::texto(if english { "artifact" } else { "artefacto" }).maximo(ANCHO_MAXIMO),
        );
    if todos {
        tabla = tabla.columna(Columna::texto(if english { "status" } else { "estado" }));
    }
    for grupo in &grupos {
        if grupo.models.is_empty() {
            continue;
        }
        tabla.grupo(Celda::motor(
            grupo.manifest.id.as_str(),
            grupo.manifest.color,
        ));
        for modelo in &grupo.models {
            tabla.fila(celdas_de_modelo(
                modelo,
                todos,
                grupo.installed,
                grupo.manifest.maturity == crate::Maturity::Partial,
                english,
            ));
        }
    }
    // Los grupos plegados van al final: no interrumpen la lista de lo que
    // sí se puede usar.
    let plegados: Vec<&EngineGroup> = grupos.iter().filter(|g| g.models.is_empty()).collect();
    if tabla.esta_vacia() && plegados.is_empty() {
        return Vec::new();
    }
    let mut seccion = Seccion::nueva(if english { "MODELS" } else { TITULO });
    seccion.cuerpo(tabla.lineas(palette));
    if !plegados.is_empty() && !tabla.esta_vacia() {
        // Una línea en blanco: los motores plegados no son parte de la tabla.
        seccion.linea("");
    }
    seccion.cuerpo(lineas_de_plegados(&plegados, palette, english));
    let mut out = seccion.lineas();
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// Las celdas de un modelo: nombre, tamaño, artefacto y estado.
fn celdas_de_modelo(
    modelo: &ModelState,
    con_estado: bool,
    engine_instalado: bool,
    engine_parcial: bool,
    english: bool,
) -> Vec<Celda> {
    let spec = &modelo.spec;
    let mut celdas = vec![
        Celda::plana(nombre_de_modelo(spec)),
        Celda::plana(tamano_de(spec)),
        Celda::plana(artefacto_de(spec)),
    ];
    if con_estado {
        let disponible =
            engine_instalado && !engine_parcial && modelo.presence == ModelPresence::Downloaded;
        let estado = if disponible {
            String::new()
        } else if engine_parcial {
            if english {
                "partial integration"
            } else {
                "integración parcial"
            }
            .to_string()
        } else {
            if english {
                "unavailable"
            } else {
                NO_DISPONIBLE
            }
            .to_string()
        };
        celdas.push(Celda::plana(estado));
    }
    celdas
}

/// El nombre del modelo, con su alias si lo tiene.
pub fn nombre_de_modelo(spec: &ModelSpec) -> String {
    match &spec.model.alias {
        Some(alias) => format!("{} ({alias})", spec.model.name),
        None => spec.model.name.clone(),
    }
}

/// El tamaño declarado del modelo: el de la descarga o el de la forja.
///
/// Un modelo puede declarar las dos vías (doc B §4.10.7): se teach el
/// tamaño de la que se vaya a usar. Sin ninguna, no hay tamaño.
pub fn tamano_de(spec: &ModelSpec) -> String {
    let declarado = spec
        .download
        .as_ref()
        .map(|d| d.size.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| spec.build.as_ref().map(|b| b.size.clone()))
        .unwrap_or_default();
    crate::output::tamano_declarado(&declarado).unwrap_or_else(|| SIN_DATO.to_string())
}

/// El artefacto del modelo: el fichero local o el que se descarga.
///
/// Es el **nombre** del artefacto, no su ruta: la ruta se enseña en
/// `ropero info`, que es donde las rutas están.
///
/// El artefacto es el que dice el **formato** (R-MOD-6), y cada formato tiene
/// el suyo:
///
/// | Formato | El artefacto |
/// |---|---|
/// | `gguf` | el fichero (`[files].gguf`) |
/// | `safetensors` | el directorio del checkpoint (`[model].path`) |
/// | `q4nx` | el **tag** del almacén (FLM) |
/// | `strata` | el paquete del catálogo (`[model].path`) |
///
/// Un modelo que **no** tiene artefacto no sale «no disponible»: sale el
/// tag (que es su artefacto) o, si tampoco se sabe, la **clase** del
/// artefacto (`tag`, `gguf`…).
pub fn artefacto_de(spec: &ModelSpec) -> String {
    match spec.model.format.artifact_location() {
        ArtifactLocation::GgufDir => primero(&[
            spec.files
                .gguf
                .as_ref()
                .map(|g| g.to_string_lossy().to_string()),
            spec.download.as_ref().map(|d| d.file.clone()),
            spec.download.as_ref().map(|d| d.name.clone()),
            no_vacia(&spec.model.path),
        ])
        .unwrap_or_else(|| "gguf".to_string()),
        ArtifactLocation::SafetensorsDir => primero(&[
            no_vacia(&spec.model.path),
            spec.download.as_ref().map(|d| d.file.clone()),
            spec.download.as_ref().map(|d| d.name.clone()),
        ])
        .unwrap_or_else(|| "safetensors".to_string()),
        ArtifactLocation::FlmTag => primero(&[no_vacia(&spec.model.path), tag_de_decision(spec)])
            .unwrap_or_else(|| "tag".to_string()),
        ArtifactLocation::StrataCatalog => {
            primero(&[no_vacia(&spec.model.path)]).unwrap_or_else(|| "strata".to_string())
        }
        ArtifactLocation::NinferDir => primero(&[
            spec.download.as_ref().map(|d| d.repo.clone()),
            no_vacia(&spec.model.path),
            spec.download.as_ref().map(|d| d.file.clone()),
        ])
        .unwrap_or_else(|| "ninfer".to_string()),
        ArtifactLocation::BasaltFile => primero(&[
            no_vacia(&spec.model.path),
            spec.download.as_ref().map(|d| d.file.clone()),
            spec.download.as_ref().map(|d| d.name.clone()),
        ])
        .unwrap_or_else(|| "basalt".to_string()),
    }
}

/// El primer valor que no está vacío.
fn primero(valores: &[Option<String>]) -> Option<String> {
    valores
        .iter()
        .flatten()
        .find(|valor| !valor.is_empty())
        .cloned()
}

/// Un texto como valor, si no está vacío.
fn no_vacia(texto: &str) -> Option<String> {
    if texto.is_empty() {
        None
    } else {
        Some(texto.to_string())
    }
}

/// El tag del motor de decisión: el valor de `--decision` en `[args].base`.
///
/// El tag de un modelo de FLM puede venir de dos sitios (doc B, R-MOD-28):
/// de `[model].path` —el modelo que se sirve como principal— y de
/// `--decision` —el motor de decisión que el servidor expone en
/// `/v1/systemone`—. El D3 (`d3mini`) es el segundo caso: su path va
/// **vacío a propósito** y su único tag es el de `--decision`.
fn tag_de_decision(spec: &ModelSpec) -> Option<String> {
    let mut args = spec.args.base.iter();
    while let Some(arg) = args.next() {
        if arg == "--decision" {
            return args.next().filter(|tag| !tag.is_empty()).cloned();
        }
    }
    None
}

/// Las líneas de los grupos **plegados**: el motor y el motivo.
///
/// Un grupo plegado es un motor sin modelos disponibles. Si está instalado,
/// lo indica; si no, el motivo viene del informe de compatibilidad (F2) o
/// aclara que falta instalarlo.
///
/// El motivo va **debajo** del motor, como una **nota** (doc CLI v2 §2.1):
/// es lo que va debajo de una fila, y no rompe la rejilla. Así los motivos
/// de todos los grupos caen en la misma columna y se leen como una columna.
fn lineas_de_plegados(plegados: &[&EngineGroup], paleta: &Palette, english: bool) -> Vec<String> {
    let mut tabla = Tabla::nivel(1);
    for grupo in plegados {
        tabla.grupo(Celda::motor(
            grupo.manifest.id.as_str(),
            grupo.manifest.color,
        ));
        tabla.nota(Celda::plana(motivo_de(grupo, english)));
    }
    tabla.lineas(paleta)
}

/// El motivo de un grupo plegado: por qué no se puede usar.
fn motivo_de(grupo: &EngineGroup, english: bool) -> String {
    if grupo.installed {
        return if english {
            "no available models"
        } else {
            "sin modelos disponibles"
        }
        .to_string();
    }
    match &grupo.reason {
        Some(r) => r.clone(),
        None => if english {
            "not installed"
        } else {
            "no está instalado"
        }
        .to_string(),
    }
}

/// Lista los modelos de `fired/`, sin depender del estado de instalación del engine.
pub fn fired_lines(
    catalog: &crate::Catalog,
    engines: &[crate::EngineManifest],
    palette: &Palette,
    english: bool,
) -> Vec<String> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut seen = BTreeSet::new();
    let mut by_engine = BTreeMap::<crate::EngineId, Vec<&ModelSpec>>::new();
    for model in &catalog.fired {
        if seen.insert(model.source.clone()) {
            by_engine.entry(model.model.engine).or_default().push(model);
        }
    }
    if by_engine.is_empty() {
        return Vec::new();
    }
    let mut table = Tabla::nivel(1)
        .columna(Columna::texto(if english { "model" } else { "modelo" }).maximo(ANCHO_MAXIMO))
        .columna(Columna::numero(if english { "size" } else { "tamaño" }))
        .columna(
            Columna::texto(if english { "artifact" } else { "artefacto" }).maximo(ANCHO_MAXIMO),
        );
    for engine in engines {
        let Some(models) = by_engine.get_mut(&engine.id) else {
            continue;
        };
        models.sort_by_key(|model| model.model.name.as_str());
        table.grupo(Celda::motor(engine.id.as_str(), engine.color));
        for model in models.iter() {
            table.fila(vec![
                Celda::plana(nombre_de_modelo(model)),
                Celda::plana(tamano_de(model)),
                Celda::plana(artefacto_de(model)),
            ]);
        }
    }
    let mut section = Seccion::nueva(if english {
        "FIRED MODELS"
    } else {
        "MODELOS DESPEDIDOS"
    });
    section.cuerpo(table.lineas(palette));
    section.lineas()
}

/// El resumen de una línea de `ropero models` cuando no hay nada que listar.
pub fn empty_notice() -> String {
    "aviso: no hay modelos disponibles (instala motores y descarga modelos)".to_string()
}

pub fn empty_notice_lang(english: bool, fired: bool) -> String {
    match (english, fired) {
        (true, true) => "no fired models".to_string(),
        (true, false) => "no available models (install engines and download models)".to_string(),
        (false, true) => "no hay modelos despedidos".to_string(),
        (false, false) => empty_notice(),
    }
}
