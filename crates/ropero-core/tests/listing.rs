//! El listado `ropero list` es **una** tabla, no una tabla por motor.
//!
//! Cubre los tres defectos que la salida tenía antes del sistema de formato
//! común:
//! - la cabecera y su regla se repetían para cada motor;
//! - un modelo que es un tag del almacén (FLM) aparecía con el artefacto
//!   «no disponible» (el artefacto sí existe: es el tag);
//! - el tamaño salía con dos formas (`~4 GB` y `18.9 GB`) y la columna se
//!   desalineaba.

use std::collections::BTreeMap;
use std::path::Path;

use ropero_core::ancho_visible;
use ropero_core::palette::Palette;
use ropero_core::snapshot::{EnginePresence, EngineState, ModelPresence, ModelState, Snapshot};
use ropero_core::{
    DetectSpec, EngineColor, EngineId, EngineManifest, Maturity, ModelSpec, PortRange, parse_model,
};

/// Un motor de prueba, con su manifiesto.
fn motor(id: EngineId, color: EngineColor, instalado: bool, puerto: u16) -> EngineState {
    EngineState {
        presence: if instalado {
            EnginePresence::Installed
        } else {
            EnginePresence::Missing
        },
        manifest: EngineManifest {
            id,
            label: id.as_str().to_string(),
            color,
            description: String::new(),
            maturity: Maturity::Integrated,
            formats: vec!["gguf".to_string()],
            port_range: PortRange::new(puerto, puerto + 99),
            deps: Vec::new(),
            detect: DetectSpec::default(),
            serve: None,
            install: None,
            compat: Vec::new(),
        },
    }
}

/// Un modelo de prueba parseado de su TOML. `descargado` decide si el modelo
/// entra en el listado plegado: sin descarga, `ropero list` lo deja fuera.
fn modelo(toml: &str, descargado: bool) -> ModelState {
    let spec: ModelSpec =
        parse_model(toml, Path::new("models/prueba.toml")).expect("el TOML de prueba es válido");
    ModelState {
        presence: if descargado {
            ModelPresence::Downloaded
        } else {
            ModelPresence::Missing
        },
        spec,
    }
}

/// El listado, sin colores y sin razones de motor: se compara como texto.
fn listar(snapshot: &Snapshot, todos: bool) -> Vec<String> {
    ropero_core::list_lines(snapshot, todos, &Palette::plain(), &BTreeMap::new())
}

/// La cabecera se reconoce por sus rótulos, no por un nombre de modelo.
fn es_cabecera(linea: &str) -> bool {
    linea.contains("modelo") && linea.contains("tamaño") && linea.contains("artefacto")
}

/// La línea de separación: sólo guiones, y ningún espacio al final.
fn es_regla(linea: &str) -> bool {
    let linea = linea.trim();
    !linea.is_empty() && linea.chars().all(|c| c == '─')
}

/// La columna visible donde empieza `texto` en `linea`: los códigos de color y
/// los acentos combinantes no cuentan, así que la comparación de columnas vale
/// para filas coloreadas y sin colorear.
fn columna(linea: &str, texto: &str) -> usize {
    let byte = linea.find(texto).expect("debe salir el texto");
    ancho_visible(&linea[..byte])
}

/// Dos motores con modelos: la cabecera y su regla salen una sola vez.
///
/// Se construye el snapshot a mano (no con fixtures) porque el fixture del CLI
/// no tiene modelos disponibles, y plegado cada motor sale en una sola línea.
fn motor_llama() -> EngineState {
    motor(EngineId::LlamaServer, EngineColor::Naranja, true, 8100)
}

fn motor_turbo() -> EngineState {
    motor(EngineId::TurboLlama, EngineColor::Amarillo, true, 8300)
}

/// El motor de FastFlowLM: el del almacén de tags.
fn motor_flm() -> EngineState {
    motor(EngineId::FastFlowLm, EngineColor::Rojo, true, 8400)
}

/// Un modelo GGUF descargable, con el tamaño en dos formas distintas.
const TOML_GGUF: &str = r#"
[model]
name    = "llama-coder"
engine  = "llama-server"
context = 8192

[files]
gguf = "llama-coder-q4_k_m.gguf"

[download]
repo = "ejemplo/llama-coder"
file = "llama-coder-q4_k_m.gguf"
size = "~3.5 GB"
"#;

const TOML_GRANDE: &str = r#"
[model]
name    = "turbo-70b"
engine  = "turbo-llama"
context = 8192

[files]
gguf = "turbo-70b-iq3.gguf"

[download]
repo = "ejemplo/turbo-70b"
file = "turbo-70b-iq3.gguf"
size = "18.9 GB"
"#;

/// Un modelo que **es** un tag del almacén: `[model].path` es el tag.
/// `[files]` vacío y `[download]` sin `file`: no hay fichero, y el path es el
/// tag. El artefacto del modelo es el tag, y el tamaño entra en la regla de
/// `~N.NN` igual que en un GGUF.
const TOML_TAG: &str = r#"
[model]
name    = "d3mini"
engine  = "turbo-llama"
context = 4096
format  = "q4nx"
path    = "d3mini:3.5b"

[files]

[download]
repo = "ejemplo/D3-Mini"
size = "~3.5 GB"
"#;

/// El caso real del D3: `[model].path` va **vacío a propósito** y su tag
/// sale de `--decision` en los args (doc B, R-MOD-28).
///
/// El artefacto de este modelo es el tag `d3:mini`: no es un GGUF y **no**
/// es «no disponible».
const TOML_TAG_DECISION: &str = r#"
[model]
name    = "d3mini"
engine  = "fastflowlm"
context = 32768
format  = "q4nx"
path    = ""

[files]

[download]
repo = "FastFlowLM/D3-Mini-NPU2"
size = "~3.5 GB"

[args]
base = [
  "--decision", "d3:mini",
  "--prefill-chunk-len", "8192",
]
"#;

/// Un modelo de safetensors: su artefacto es el **directorio** del
/// checkpoint, no un fichero.
const TOML_SAFETENSORS: &str = r#"
[model]
name    = "metalqwen"
engine  = "mlx-serve"
context = 32768
format  = "safetensors"
path    = "mlx-community/Qwen3.8-27B-4bit"

[files]

[download]
repo = "mlx-community/Qwen3.8-27B-4bit"
size = "~15 GB"
"#;

/// Un modelo de Strata: su artefacto es el **paquete** del catálogo.
const TOML_STRATA: &str = r#"
[model]
name    = "gyro-m"
engine  = "strata"
context = 8192
format  = "strata"
path    = "gyro/Gyro-M"

[files]
"#;

#[test]
fn la_cabecera_de_la_lista_sale_una_sola_vez() {
    let snapshot = Snapshot::new(
        vec![motor_llama(), motor_turbo()],
        vec![
            modelo(TOML_GGUF, true),
            modelo(TOML_GRANDE, true),
            modelo(TOML_TAG, true),
        ],
    );
    let lineas = listar(&snapshot, false);
    let cabeceras = lineas.iter().filter(|l| es_cabecera(l)).count();
    assert_eq!(cabeceras, 1, "{lineas:#?}");
    let reglas = lineas.iter().filter(|l| es_regla(l)).count();
    assert_eq!(reglas, 2, "una regla por grupo, {lineas:#?}");
    // Dos filas de grupo y tres modelos.
    let grupos = lineas
        .iter()
        .filter(|l| !es_cabecera(l) && (l.contains("llama-server") || l.contains("turbo-llama")))
        .count();
    assert_eq!(grupos, 2, "{lineas:#?}");
    for modelo in ["llama-coder", "turbo-70b", "d3mini"] {
        assert!(
            lineas.iter().any(|l| l.contains(modelo)),
            "falta {modelo}, {lineas:#?}"
        );
    }
    // Ninguna línea con espacios al final.
    for linea in &lineas {
        assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
    }
}

/// Un modelo que es un tag: el artefacto es el tag, no «no disponible».
#[test]
fn el_artefacto_de_un_tag_es_el_tag() {
    let snapshot = Snapshot::new(
        vec![motor_turbo()],
        vec![modelo(TOML_GRANDE, true), modelo(TOML_TAG, true)],
    );
    let lineas = listar(&snapshot, true);
    let cabecera = lineas
        .iter()
        .find(|l| es_cabecera(l))
        .expect("debe haber cabecera");
    let fila = lineas
        .iter()
        .find(|l| l.contains("d3mini"))
        .expect("el modelo sale en la lista");
    // El artefacto es el tag del almacén, y cae en la columna del artefacto.
    assert_eq!(
        columna(fila, "d3mini:3.5b"),
        columna(cabecera, "artefacto"),
        "el tag no cae en la columna del artefacto: {fila}"
    );
    assert!(
        !fila.contains("no disponible"),
        "el artefacto de un modelo con tag no es «no disponible»: {fila}"
    );
    // Y el tamaño del otro modelo cae en la misma columna: la columna numérica
    // alinea por la derecha, así que se compara el final de la columna.
    let fila_gguf = lineas
        .iter()
        .find(|l| l.contains("turbo-70b"))
        .expect("el modelo GGUF sale en la lista");
    let fin = |linea: &str, texto: &str| columna(linea, texto) + ancho_visible(texto);
    assert_eq!(
        fin(fila_gguf, "~18.90 GB"),
        fin(cabecera, "tamaño"),
        "la columna del tamaño no alinea: {fila_gguf}"
    );
}

/// El tamaño tiene una sola forma: `~N.NN UNIDAD`, con y sin `~` de origen.
#[test]
fn la_lista_normaliza_el_tamano() {
    let snapshot = Snapshot::new(vec![motor_llama()], vec![modelo(TOML_GGUF, true)]);
    let lineas = listar(&snapshot, false);
    let fila = lineas
        .iter()
        .find(|l| l.contains("llama-coder"))
        .expect("el modelo sale en la lista");
    assert!(fila.contains("~3.50 GB"), "fila={fila}");
    assert!(!fila.contains("3.5 GB"), "fila={fila}");
}

/// Un modelo sin `[model].path`: su artefacto es el tag de `--decision`.
///
/// Es el caso del D3: su path va vacío a propósito y el tag sale de los args
/// (doc B, R-MOD-28). El artefacto **es** ese tag: nunca «no disponible».
#[test]
fn el_artefacto_de_un_modelo_sin_path_es_el_tag_de_decision() {
    let snapshot = Snapshot::new(vec![motor_flm()], vec![modelo(TOML_TAG_DECISION, true)]);
    let lineas = listar(&snapshot, false);
    let fila = lineas
        .iter()
        .find(|l| l.contains("d3mini"))
        .expect("el modelo sale en la lista");
    assert!(fila.contains("d3:mini"), "fila={fila}");
    assert!(!fila.contains("no disponible"), "fila={fila}");
    // Y el tag cae en la columna del artefacto, no en la del modelo.
    let cabecera = lineas
        .iter()
        .find(|l| es_cabecera(l))
        .expect("debe haber cabecera");
    assert_eq!(
        columna(fila, "d3:mini"),
        columna(cabecera, "artefacto"),
        "el tag no cae en la columna del artefacto: {fila}"
    );
}

/// Cada formato tiene **su** artefacto (R-MOD-6): el fichero, el directorio,
/// el tag del almacén o el paquete del catálogo.
#[test]
fn el_artefacto_de_cada_formato_es_el_suyo() {
    // Un GGUF: el fichero.
    let gguf = parse_model(TOML_GGUF, Path::new("models/llama-coder.toml"))
        .expect("el TOML de prueba es válido");
    assert_eq!(
        ropero_core::listing::artefacto_de(&gguf),
        "llama-coder-q4_k_m.gguf"
    );
    // Un safetensors: el directorio del checkpoint.
    let safetensors = parse_model(TOML_SAFETENSORS, Path::new("models/metalqwen.toml"))
        .expect("el TOML de prueba es válido");
    assert_eq!(
        ropero_core::listing::artefacto_de(&safetensors),
        "mlx-community/Qwen3.8-27B-4bit"
    );
    // Un Strata: el paquete del catálogo.
    let strata = parse_model(TOML_STRATA, Path::new("models/gyro-m.toml"))
        .expect("el TOML de prueba es válido");
    assert_eq!(ropero_core::listing::artefacto_de(&strata), "gyro/Gyro-M");
    // Un q4nx: el tag del almacén, venga de donde venga.
    let tag = parse_model(TOML_TAG, Path::new("models/d3mini.toml"))
        .expect("el TOML de prueba es válido");
    assert_eq!(ropero_core::listing::artefacto_de(&tag), "d3mini:3.5b");
    let decision = parse_model(TOML_TAG_DECISION, Path::new("models/d3mini.toml"))
        .expect("el TOML de prueba es válido");
    assert_eq!(ropero_core::listing::artefacto_de(&decision), "d3:mini");
}

/// Todos los tamaños de la lista tienen **la misma forma**: `~N.NN UNIDAD`.
///
/// Es la regla única de la salida (doc CLI v2 §2): un tamaño con `~` y otro
/// sin él descuadraban la columna.
#[test]
fn todos_los_tamanos_de_la_lista_tienen_la_misma_forma() {
    let snapshot = Snapshot::new(
        vec![motor_llama(), motor_turbo(), motor_flm()],
        vec![
            modelo(TOML_GGUF, true),
            modelo(TOML_GRANDE, true),
            modelo(TOML_TAG, true),
            modelo(TOML_TAG_DECISION, true),
        ],
    );
    let lineas = listar(&snapshot, true);
    let tamanos: Vec<&str> = lineas
        .iter()
        .filter_map(|l| {
            let inicio = l.find('~')?;
            let fin = l[inicio..].find(" GB")? + inicio;
            Some(&l[inicio..fin + 3])
        })
        .collect();
    assert!(tamanos.len() >= 4, "lineas={lineas:#?}");
    for tamano in tamanos {
        assert!(tamano.starts_with('~'), "tamano={tamano:?}");
        let numero = tamano.trim_start_matches('~');
        let decimales = numero
            .split_once(' ')
            .expect("el tamaño lleva unidad")
            .0
            .split('.')
            .nth(1)
            .expect("el tamaño lleva decimales");
        assert_eq!(decimales.len(), 2, "tamano={tamano:?}");
    }
}

/// Los grupos **plegados** dicen el motivo **alineado**: el motivo va
/// debajo del motor, un nivel más dentro, como una nota (doc CLI v2 §2.1).
///
/// Antes el motivo iba pegado al nombre del motor, a dos espacios: cada
/// grupo lo tenía en un sitio y la columna de los motivos no era una
/// columna.
#[test]
fn los_grupos_plegados_dicen_el_motivo_alineado() {
    let snapshot = Snapshot::new(
        vec![
            motor(EngineId::AirLlm, EngineColor::Blanco, false, 8700),
            motor(EngineId::MlxServe, EngineColor::Gris, false, 8300),
        ],
        vec![],
    );
    let lineas = listar(&snapshot, false);
    // El motor plegado sale con su nombre y, debajo, el motivo.
    let indice = lineas
        .iter()
        .position(|l| l.contains("airllm"))
        .expect("el motor plegado sale");
    assert_eq!(lineas[indice], "  airllm");
    assert_eq!(lineas[indice + 1], "    no está instalado");
    // Y todos los motivos empiezan en la misma columna.
    let columnas: Vec<usize> = lineas
        .iter()
        .filter(|l| l.contains("no está instalado"))
        .map(|l| columna(l, "no está"))
        .collect();
    assert_eq!(columnas.len(), 2, "{lineas:#?}");
    assert!(
        columnas.windows(2).all(|w| w[0] == w[1]),
        "los motivos no se alinean: {columnas:?}"
    );
    // Ninguna línea con espacios al final.
    for linea in &lineas {
        assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
    }
}
