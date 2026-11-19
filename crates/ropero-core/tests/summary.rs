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
            serve: None,
            install: None,
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

/// Un modelo cuyo artefacto es un **tag** del almacén del motor (FLM).
///
/// No tiene fichero: su artefacto no es un GGUF, es una etiqueta.
fn modelo_tag(nombre: &str, motor: EngineId) -> ModelState {
    let mut estado = modelo(nombre, motor, true);
    estado.spec.model.format = ropero_core::ModelFormat::Q4nx;
    estado.spec.files.gguf = None;
    if let Some(descarga) = estado.spec.download.as_mut() {
        descarga.file = String::new();
    }
    estado
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

/// Un TOML de modelo **con alias**.
const TOML_CON_ALIAS: &str = r#"
[model]
name = "groot"
engine = "llama-server"
context = 1024
alias = "g"

[files]
gguf = "g.gguf"
"#;

/// Un detector que dice que todo está descargado.
struct TodoDescargado;

impl ropero_core::snapshot::Detect for TodoDescargado {
    fn engine_installed(&self, _manifest: &EngineManifest) -> bool {
        true
    }

    fn model_downloaded(&self, _spec: &ModelSpec) -> bool {
        true
    }
}

/// El resumen cuenta **los TOML del catálogo**, no las filas: un alias no
/// es un modelo más (doc CLI v2 §6.5).
///
/// El catálogo materializa el alias como una fila extra, así que el mismo
/// modelo aparece dos veces; los "modelos disponibles" son los TOML, y un
/// TOML es uno.
#[test]
fn el_resumen_no_cuenta_el_alias_como_otro_modelo() {
    let spec = ropero_core::parse_model(TOML_CON_ALIAS, std::path::Path::new("models/groot.toml"))
        .expect("parsear el modelo");
    let catalogo = ropero_core::build_catalog(vec![spec], vec![]).expect("construir el catálogo");
    // El alias se materializa como una fila extra...
    assert_eq!(catalogo.models.len(), 2, "filas={}", catalogo.models.len());
    // ...pero es el mismo modelo: el resumen dice uno.
    let foto = ropero_core::snapshot::build_snapshot(&[], &catalogo.models, &TodoDescargado);
    let c = counts(&foto);
    assert_eq!(c.modelos_disponibles, 1, "el alias no es otro modelo");
    assert_eq!(c.modelos_instalados, 1, "el alias no se cuenta dos veces");
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

/// Un motor con integración parcial no se presenta como utilizable aunque
/// figure instalado y el artefacto del modelo exista.
#[test]
fn no_disponible_si_el_motor_tiene_integracion_parcial() {
    let mut foto = todo_instalado();
    let air = foto
        .engines
        .iter_mut()
        .find(|engine| engine.manifest.id == EngineId::AirLlm)
        .expect("debe estar airllm");
    air.presence = EnginePresence::Installed;
    air.manifest.maturity = ropero_core::Maturity::Partial;

    assert!(
        available_models(&foto)
            .iter()
            .all(|model| model.spec.model.engine != EngineId::AirLlm)
    );

    let groups = engine_groups(&foto, true, &BTreeMap::new());
    let air = groups
        .iter()
        .find(|group| group.manifest.id == EngineId::AirLlm)
        .expect("debe estar airllm");
    assert_eq!(air.models.len(), 1, "--all conserva el catálogo");
    let output = ropero_core::listing::list_lines(&foto, true, &Palette::plain(), &BTreeMap::new())
        .join("\n");
    assert!(output.contains("integración parcial"), "{output}");
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
    // AirLLM no está instalado y, por defecto, no se muestra.
    assert!(!grupos.iter().any(|g| g.manifest.id == EngineId::AirLlm));
}

#[test]
fn motor_instalado_sin_modelos_disponibles_se_indica() {
    let mut foto = todo_instalado();
    for model in &mut foto.models {
        if model.spec.model.engine == EngineId::LlamaServer {
            model.presence = ModelPresence::Missing;
        }
    }
    let lineas =
        ropero_core::listing::list_lines(&foto, false, &Palette::plain(), &BTreeMap::new());
    let texto = lineas.join("\n");
    assert!(texto.contains("llama-server"), "{texto}");
    assert!(texto.contains("sin modelos disponibles"), "{texto}");
    assert!(!texto.contains("faltante"), "{texto}");
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
    let grupos = engine_groups(&foto, true, &razones);
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
    let grupos = engine_groups(&foto, true, &razones);
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
    // La leyenda usa el fondo naranja de la paleta hexadecimal del proyecto.
    assert!(
        cabecera.contains("\x1b[48;2;253;186;116m\x1b[38;2;0;0;0m"),
        "cabecera={cabecera:?}"
    );
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

// ─────────────────────────────────────────────────────────────────────────────
// El formato del listado (doc CLI v2 §2)
// ─────────────────────────────────────────────────────────────────────────────

/// Las líneas del listado de una foto, sin color.
fn listado(foto: &Snapshot, todos: bool) -> Vec<String> {
    let razones = BTreeMap::new();
    ropero_core::listing::list_lines(foto, todos, &Palette::plain(), &razones)
}

/// Las filas de los modelos (las que llevan el tamaño).
fn filas_de_modelo(lineas: &[String]) -> Vec<&String> {
    lineas.iter().filter(|l| l.contains("GB")).collect()
}

/// La columna del tamaño se alinea a la **derecha**: es un número.
///
/// La "GB" de todos los tamaños cae en la misma columna.
#[test]
fn el_listado_alinea_el_tamano_a_la_derecha() {
    let lineas = listado(&todo_instalado(), false);
    let filas = filas_de_modelo(&lineas);
    assert!(filas.len() >= 3, "lineas={lineas:?}");
    let columnas: Vec<usize> = filas
        .iter()
        .map(|l| l.find("GB").expect("la fila tiene tamaño"))
        .collect();
    assert!(
        columnas.windows(2).all(|w| w[0] == w[1]),
        "los tamaños no se alinean: {filas:?}"
    );
}

/// El artefacto va en **su propia** columna: su nombre largo no empuja al
/// tamaño.
#[test]
fn el_listado_separa_el_tamano_del_artefacto() {
    let lineas = listado(&todo_instalado(), false);
    let filas = filas_de_modelo(&lineas);
    let artefactos = ["alfa.gguf", "beta.gguf", "gamma.gguf"];
    let posiciones: Vec<usize> = filas
        .iter()
        .zip(artefactos)
        .map(|(l, artefacto)| l.find(artefacto).expect("la fila tiene artefacto"))
        .collect();
    assert!(
        posiciones.windows(2).all(|w| w[0] == w[1]),
        "los artefactos no se alinean: {filas:?}"
    );
}

/// La cabecera del listado sale **una sola vez**: el motor es una fila de
/// grupo, no otra tabla.
#[test]
fn el_listado_pone_una_sola_cabecera() {
    let lineas = listado(&todo_instalado(), false);
    let cabeceras: Vec<&String> = lineas
        .iter()
        .filter(|l| l.trim_start().starts_with("modelo"))
        .collect();
    assert_eq!(cabeceras.len(), 1, "lineas={lineas:?}");
}

/// Los grupos del listado se separan con una **regla**.
#[test]
fn los_grupos_del_listado_se_separan_con_una_regla() {
    let lineas = listado(&todo_instalado(), false);
    let reglas: Vec<&String> = lineas
        .iter()
        .filter(|l| l.trim_start().starts_with('─'))
        .collect();
    // La regla de la cabecera y una por grupo menos el primero.
    assert_eq!(reglas.len(), 2, "lineas={lineas:?}");
}

/// El tamaño tiene **una sola forma**: `~N.NN UNIDAD`.
#[test]
fn el_tamano_tiene_una_sola_forma() {
    assert_eq!(
        ropero_core::tamano_declarado("~3.5 GB").as_deref(),
        Some("~3.50 GB")
    );
    assert_eq!(
        ropero_core::tamano_declarado("18.90 GB").as_deref(),
        Some("~18.90 GB")
    );
    assert_eq!(
        ropero_core::tamano_declarado("13.6 GB").as_deref(),
        Some("~13.60 GB")
    );
    assert_eq!(
        ropero_core::tamano_declarado("~4 GB").as_deref(),
        Some("~4.00 GB")
    );
}

/// Un tamaño que **no se sabe** no se inventa: un cero no es un tamaño.
#[test]
fn un_tamano_desconocido_no_se_inventa() {
    assert_eq!(ropero_core::tamano_declarado(""), None);
    assert_eq!(ropero_core::tamano_declarado("0 GB"), None);
}

/// Un modelo sin fichero declara su artefacto por **clase**: un `tag` del
/// almacén de FLM (no es un GGUF).
#[test]
fn el_artefacto_sin_fichero_dice_la_clase_del_artefacto() {
    let spec = modelo_tag("d3mini", EngineId::FastFlowLm).spec;
    assert_eq!(ropero_core::listing::artefacto_de(&spec), "tag");
}

/// El tag declarado sale tal cual: es el nombre del artefacto.
#[test]
fn el_artefacto_muestra_el_tag_declarado() {
    let mut spec = modelo_tag("lfm3", EngineId::FastFlowLm).spec;
    spec.model.path = "lfm3:1.7b".to_string();
    assert_eq!(ropero_core::listing::artefacto_de(&spec), "lfm3:1.7b");
}

/// Ninguna línea del listado lleva espacios al final.
#[test]
fn el_listado_no_deja_espacios_al_final() {
    for linea in listado(&todo_instalado(), true) {
        assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
    }
}

/// Con el `--all`, lo que no está disponible va en **su propia columna**.
#[test]
fn el_listado_marca_los_no_disponibles_en_una_columna() {
    let lineas = listado(&todo_instalado(), true);
    let epsilon = lineas
        .iter()
        .find(|l| l.contains("epsilon"))
        .expect("debe salir epsilon");
    assert!(epsilon.contains("no disponible"), "linea={epsilon:?}");
    // Y la columna existe: la marca no se come el artefacto.
    let cabecera = lineas
        .iter()
        .find(|l| l.contains("estado"))
        .expect("debe salir la columna del estado");
    assert!(cabecera.contains("estado"), "cabecera={cabecera:?}");
}
