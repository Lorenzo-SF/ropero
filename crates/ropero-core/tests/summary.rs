//! El resumen (`ropero` a secas) y el listado (`ropero list`).
//!
//! Las dos salidas se prueban con la **foto inyectada**: los tests no
//! preguntan al disco, así que el resultado no depende de la máquina que
//! los ejecuta. La foto se construye a mano (motores instalados o no,
//! modelos descargados o no) y se le pasa al dominio.

use std::collections::BTreeMap;

use ropero_core::palette::Palette;
use ropero_core::snapshot::{
    EngineGroup, EnginePresence, EngineState, ModelPresence, ModelState, Snapshot,
    available_models, counts, engine_groups,
};
use ropero_core::{EngineId, EngineManifest, ModelSpec};

/// Un motor de prueba, con su manifiesto.
fn motor(id: EngineId, color: ropero_core::EngineColor, instalado: bool) -> EngineState {
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
            maturity: ropero_core::Maturity::Integrated,
            formats: vec!["gguf".to_string()],
            port_range: ropero_core::PortRange::new(8100, 8199),
            deps: Vec::new(),
            detect: ropero_core::DetectSpec::default(),
            compat: Vec::new(),
        },
    }
}

/// Un modelo de prueba.
fn modelo(nombre: &str, motor: EngineId, descargado: bool) -> ModelState {
    ModelState {
        presence: if descargado {
            ModelPresence::Downloaded
        } else {
            ModelPresence::Missing
        },
        spec: ModelSpec {
            model: ropero_core::ModelMeta {
                name: nombre.to_string(),
                engine: motor,
                context: 4096,
                alias: None,
                port: None,
                format: ropero_core::ModelFormat::Gguf,
                path: String::new(),
            },
            files: ropero_core::Files {
                gguf: Some(format!("{nombre}.gguf").into()),
                mtp: None,
                mmproj: None,
                chat_template: None,
            },
            download: Some(ropero_core::Download {
                repo: "ejemplo/repo".to_string(),
                gguf: false,
                file: format!("{nombre}.gguf"),
                name: String::new(),
                size: "~1 GB".to_string(),
                manual: false,
                excludes: Vec::new(),
                aux: BTreeMap::new(),
            }),
            build: None,
            levels: ropero_core::LevelSpec::default(),
            flags: ropero_core::FlagsSpec::default(),
            options: ropero_core::OptionsSpec::default(),
            aliases: BTreeMap::new(),
            limits: ropero_core::Limits::default(),
            args: ropero_core::Args::default(),
            source: format!("models/{nombre}.toml").into(),
            notices: Vec::new(),
            real_name: None,
        },
    }
}

/// Una foto con todo instalado.
fn todo_instalado() -> Snapshot {
    Snapshot::new(
        vec![
            motor(
                EngineId::LlamaServer,
                ropero_core::EngineColor::Naranja,
                true,
            ),
            motor(EngineId::Vllm, ropero_core::EngineColor::Morado, true),
            motor(EngineId::AirLlm, ropero_core::EngineColor::Blanco, false),
        ],
        vec![
            modelo("alfa", EngineId::LlamaServer, true),
            modelo("beta", EngineId::LlamaServer, true),
            modelo("gamma", EngineId::Vllm, true),
            modelo("delta", EngineId::AirLlm, true),
            modelo("epsilon", EngineId::Vllm, false),
        ],
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// El resumen
// ─────────────────────────────────────────────────────────────────────────────

/// El resumen muestra las **cuatro** secciones del doc (doc CLI v2 §6).
#[test]
fn el_resumen_muestra_las_cuatro_secciones() {
    let texto = ropero_core::snapshot::resumen_text(
        &todo_instalado(),
        "0.1.0",
        "2026-11-14",
        "Lorenzo Sánchez Fraile",
    );
    assert!(texto.contains("MOTORES"), "texto={texto}");
    assert!(texto.contains("MODELOS"), "texto={texto}");
    assert!(texto.contains("0.1.0"), "texto={texto}");
    assert!(texto.contains("2026-11-14"), "texto={texto}");
    assert!(texto.contains("Lorenzo Sánchez Fraile"), "texto={texto}");
}

/// Los números del resumen: los disponibles y los instalados se distinguen.
#[test]
fn el_resumen_distingue_lo_disponible_de_lo_instalado() {
    let c = counts(&todo_instalado());
    // Tres motores con manifiesto: dos instalados.
    assert_eq!(c.motores_disponibles, 3);
    assert_eq!(c.motores_instalados, 2);
    // Cinco modelos en el catálogo: cuatro descargados (`delta` está
    // descargado, pero su motor no: disponible e instalado son cosas
    // distintas).
    assert_eq!(c.modelos_disponibles, 5);
    assert_eq!(c.modelos_instalados, 4);
}

/// El resumen sugiere la ayuda.
#[test]
fn el_resumen_sugiere_la_ayuda() {
    let texto =
        ropero_core::snapshot::resumen_text(&todo_instalado(), "0.1.0", "2026-11-14", "autor");
    assert!(texto.contains("ropero --help"), "texto={texto}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Los modelos disponibles
// ─────────────────────────────────────────────────────────────────────────────

/// "Disponible" son las dos cosas a la vez: motor instalado **y** modelo
/// descargado (doc CLI v2 §15).
#[test]
fn disponible_es_motor_instalado_y_modelo_descargado() {
    let foto = todo_instalado();
    let disponibles = available_models(&foto);
    let nombres: Vec<&str> = disponibles
        .iter()
        .map(|m| m.spec.model.name.as_str())
        .collect();
    // `alfa` y `beta` (llama-server instalado, descargados) y `gamma` (vllm).
    // `delta` es de un motor NO instalado; `epsilon` no está descargado.
    assert!(nombres.contains(&"alfa"), "nombres={nombres:?}");
    assert!(nombres.contains(&"beta"), "nombres={nombres:?}");
    assert!(nombres.contains(&"gamma"), "nombres={nombres:?}");
    assert!(!nombres.contains(&"delta"), "nombres={nombres:?}");
    assert!(!nombres.contains(&"epsilon"), "nombres={nombres:?}");
}

/// El listado agrupa por motor y sólo enseña lo disponible.
#[test]
fn el_listado_agrupa_por_motor_y_solo_enseña_lo_disponible() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let grupos = engine_groups(&foto, false, &razones);
    let llama = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::LlamaServer)
        .expect("debe estar llama-server");
    assert_eq!(llama.models.len(), 2, "modelos={:?}", llama.models.len());
    let vllm = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::Vllm)
        .expect("debe estar vllm");
    assert_eq!(vllm.models.len(), 1);
    // AirLLM no está instalado: su grupo no trae modelos disponibles.
    let air = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::AirLlm)
        .expect("debe estar airllm");
    assert!(air.models.is_empty());
}

/// Con `--all` salen también los que no están disponibles, marcados.
#[test]
fn con_all_salen_tambien_los_no_disponibles() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let grupos = engine_groups(&foto, true, &razones);
    let vllm = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::Vllm)
        .expect("debe estar vllm");
    // `gamma` (disponible) y `epsilon` (no descargado).
    assert_eq!(vllm.models.len(), 2, "modelos={:?}", vllm.models.len());
    let marcado = vllm
        .models
        .iter()
        .find(|m| m.spec.model.name == "epsilon")
        .expect("debe salir epsilon");
    assert_eq!(marcado.presence, ModelPresence::Missing);
}

/// El grupo de un motor que no está instalado dice **por qué**.
#[test]
fn el_grupo_plegado_dice_la_razon() {
    let foto = todo_instalado();
    let mut razones = BTreeMap::new();
    razones.insert(
        EngineId::AirLlm,
        "sólo funciona en Linux; necesita una NPU AMD XDNA".to_string(),
    );
    let grupos = engine_groups(&foto, false, &razones);
    let air = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::AirLlm)
        .expect("debe estar airllm");
    let razon = air.reason.as_deref().expect("debe haber razón");
    assert!(razon.contains("NPU AMD XDNA"), "razon={razon}");
}

/// Sin razón conocida, el grupo plegado dice que no está instalado.
#[test]
fn el_grupo_plegado_sin_razon_dice_que_no_esta_instalado() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let grupos = engine_groups(&foto, false, &razones);
    let air = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::AirLlm)
        .expect("debe estar airllm");
    let razon = air.reason.as_deref().expect("debe haber razón");
    assert!(razon.contains("no está instalado"), "razon={razon}");
}

// ─────────────────────────────────────────────────────────────────────────────
// La salida del listado
// ─────────────────────────────────────────────────────────────────────────────

/// El listado con color: el nombre del motor va en el color de la leyenda.
#[test]
fn el_listado_pinta_el_motor_con_su_color() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let lineas = ropero_core::listing::list_lines(&foto, false, &Palette::color(), &razones);
    let cabecera = lineas
        .iter()
        .find(|l| l.contains("llama-server"))
        .expect("debe salir la cabecera de llama-server");
    // El naranja de la leyenda es el 208 de la paleta de 256.
    assert!(cabecera.contains("\x1b[38;5;208m"), "cabecera={cabecera:?}");
    // Y el texto del motor sigue ahí, legible.
    assert!(cabecera.contains("llama-server"), "cabecera={cabecera:?}");
}

/// Sin color, el listado es el mismo texto sin secuencias ANSI.
#[test]
fn sin_color_no_hay_secuencias_ansi() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let lineas = ropero_core::listing::list_lines(&foto, false, &Palette::plain(), &razones);
    for linea in &lineas {
        assert!(!linea.contains('\x1b'), "linea={linea:?}");
    }
}

/// Los modelos no disponibles salen marcados.
#[test]
fn los_modelos_no_disponibles_salen_marcados() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let lineas = ropero_core::listing::list_lines(&foto, true, &Palette::plain(), &razones);
    let epsilon = lineas
        .iter()
        .find(|l| l.contains("epsilon"))
        .expect("debe salir epsilon");
    assert!(epsilon.contains("no disponible"), "linea={epsilon:?}");
}

/// El listado no inventa motores que no están en la foto.
#[test]
fn el_listado_no_inventa_motores() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let lineas = ropero_core::listing::list_lines(&foto, false, &Palette::plain(), &razones);
    let texto = lineas.join("\n");
    assert!(!texto.contains("tensorrt"), "texto={texto}");
    assert!(!texto.contains("mlx"), "texto={texto}");
}

/// Un grupo vacío (sin modelos) no despliega nada.
#[test]
fn un_grupo_vacio_no_despliega_modelos() {
    let foto = Snapshot::new(vec![], vec![]);
    let razones = BTreeMap::new();
    let lineas = ropero_core::listing::list_lines(&foto, false, &Palette::plain(), &razones);
    assert!(lineas.is_empty(), "lineas={lineas:?}");
}

/// El recuento de un grupo.
#[test]
fn el_recuento_de_un_grupo_es_el_numero_de_modelos() {
    let foto = todo_instalado();
    let razones = BTreeMap::new();
    let grupos = engine_groups(&foto, false, &razones);
    let llama: &EngineGroup = grupos
        .iter()
        .find(|g| g.manifest.id == EngineId::LlamaServer)
        .expect("debe estar llama-server");
    assert_eq!(llama.models.len(), 2);
}
