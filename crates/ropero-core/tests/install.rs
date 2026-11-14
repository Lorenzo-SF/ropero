//! El contrato de `--prio` y la decisión de vía (doc B §4.10.7).
//!
//! Este test es la puerta ejecutable del contrato normativo de `--prio`:
//! los tres ejemplos canónicos del doc, y los casos límite que de ellos se
//! derivan (precedencia, ignorado, valor inválido, lista con comas y
//! alias).

use std::path::Path;

use ropero_core::install::{
    Decision, Error, ModeloEnPosicion, Motivo, MotorDelMenu, PrioEnPosicion, Via, asocia_prios,
    elegir_via, menu_engines, menu_modelos, parse_lista, prio_ignorado, resuelve_modelo,
    resuelve_seleccion, vias_de,
};
use ropero_core::{ModelSpec, Palette, build_catalog, parse_engine_manifest, parse_model};

/// Un modelo con las dos vías: `[download]` y `[build]`.
const DOS_VIAS: &str = r#"
[model]
name = "nemotron"
engine = "llama-server"

[files]
gguf = "nemotron.gguf"

[download]
repo = "ejemplo/nemotron"
file = "nemotron.gguf"

[build]
repo = "ejemplo/nemotron-pesos"
"#;

/// Un modelo con sólo `[download]`.
const SOLO_DOWNLOAD: &str = r#"
[model]
name = "groot"
engine = "llama-server"

[files]
gguf = "groot.gguf"

[download]
repo = "ejemplo/groot"
file = "groot.gguf"
"#;

/// Un modelo de **descarga manual**: no se ofrece en el menú.
const MANUAL: &str = r#"
[model]
name = "manual"
engine = "llama-server"

[files]
gguf = "manual.gguf"

[download]
repo = "ejemplo/manual"
file = "manual.gguf"
manual = true
"#;

/// Un modelo con sólo `[build]`.
const SOLO_BUILD: &str = r#"
[model]
name = "qwen"
engine = "llama-server"

[files]
gguf = "qwen.gguf"

[build]
repo = "ejemplo/qwen-pesos"
"#;

/// Un modelo sin ninguna vía.
const SIN_VIAS: &str = r#"
[model]
name = "vacio"
engine = "llama-server"

[files]
gguf = "vacio.gguf"
"#;

fn spec(toml: &str) -> ModelSpec {
    spec_en(toml, "modelo.toml")
}

/// Un modelo con **su ruta de origen**: el catálogo deduplica las filas
/// por ruta, así que dos modelos con la misma ruta serían uno.
fn spec_en(toml: &str, ruta: &str) -> ModelSpec {
    parse_model(toml, Path::new(ruta)).expect("parse model")
}

/// Las posiciones de los modelos y de los `--prio` de una invocación.
fn posiciones(tokens: &[&str]) -> (Vec<ModeloEnPosicion>, Vec<PrioEnPosicion>) {
    let mut modelos = Vec::new();
    let mut prios = Vec::new();
    for (posicion, token) in tokens.iter().enumerate() {
        if let Some(valor) = token.strip_prefix("--prio=") {
            prios.push(PrioEnPosicion {
                posicion,
                valor: valor.to_string(),
            });
        } else {
            modelos.push(ModeloEnPosicion {
                posicion,
                nombre: token.to_string(),
            });
        }
    }
    (modelos, prios)
}

/// Asocia los prios de una invocación escrita como tokens.
fn prios_de(tokens: &[&str]) -> Result<ropero_core::install::Prios, Error> {
    let (modelos, prios) = posiciones(tokens);
    asocia_prios(&modelos, &prios)
}

// ─────────────────────────────────────────────────────────────────────────────
// Los tres ejemplos canónicos del doc B §4.10.7
// ─────────────────────────────────────────────────────────────────────────────

/// El `--prio` **global** va antes de la lista y vale para todos.
///
/// `ropero install models --prio=build nemotron groot`
#[test]
fn el_prio_global_se_aplica_a_todos_los_modelos() {
    let prios = prios_de(&["--prio=build", "nemotron", "groot"]).expect("prios");
    assert_eq!(prios.global, Some(Via::Build));
    assert_eq!(prios.de("nemotron"), Some(Via::Build));
    assert_eq!(prios.de("groot"), Some(Via::Build));
    assert!(prios.por_modelo.is_empty(), "no hay prios por modelo");
}

/// El `--prio` **por modelo** va detrás del modelo y su ámbito **termina en
/// el siguiente modelo**.
///
/// `ropero install models nemotron groot --prio=build qwen --prio=download`
/// → nemotron: ninguno; groot: build; qwen: download.
#[test]
fn el_ambito_del_prio_por_modelo_termina_en_el_siguiente_modelo() {
    let prios = prios_de(&[
        "nemotron",
        "groot",
        "--prio=build",
        "qwen",
        "--prio=download",
    ])
    .expect("prios");
    assert_eq!(prios.de("nemotron"), None);
    assert_eq!(prios.de("groot"), Some(Via::Build));
    assert_eq!(prios.de("qwen"), Some(Via::Download));
    assert_eq!(prios.global, None, "no hay prio global");
}

/// Con global y por modelo **manda el por modelo**; el global sólo aplica a
/// los que no tienen prio propio.
///
/// `ropero install models --prio=build nemotron groot --prio=download`
/// → nemotron va por build (el global) y groot por download (el suyo).
#[test]
fn el_prio_por_modelo_manda_sobre_el_global() {
    let prios = prios_de(&["--prio=build", "nemotron", "groot", "--prio=download"]).expect("prios");
    assert_eq!(prios.global, Some(Via::Build));
    assert_eq!(prios.de("nemotron"), Some(Via::Build));
    assert_eq!(prios.de("groot"), Some(Via::Download));
}

// ─────────────────────────────────────────────────────────────────────────────
// El valor de `--prio`
// ─────────────────────────────────────────────────────────────────────────────

/// Un valor que no es `download` ni `build` es **error de uso** y el mensaje
/// **incluye el valor recibido**.
#[test]
fn un_valor_de_prio_invalido_es_error_con_el_valor_recibido() {
    let error = prios_de(&["--prio=fast", "nemotron"]).expect_err("debe fallar");
    assert_eq!(
        error,
        Error::PrioInvalido {
            valor: "fast".to_string()
        }
    );
    assert!(
        error.to_string().contains("fast"),
        "el mensaje no dice el valor recibido: {error}"
    );
    assert_eq!(error.exit_code(), 2, "un valor inválido es error de uso");
}

/// Los dos valores válidos se parsean.
#[test]
fn los_dos_valores_validos_se_parsean() {
    assert_eq!(Via::parse("download"), Ok(Via::Download));
    assert_eq!(Via::parse("build"), Ok(Via::Build));
}

// ─────────────────────────────────────────────────────────────────────────────
// La decisión de vía
// ─────────────────────────────────────────────────────────────────────────────

/// Un modelo con las dos vías y **sin** `--prio` se **pregunta**.
#[test]
fn un_modelo_con_las_dos_vias_y_sin_prio_pregunta() {
    let spec = spec(DOS_VIAS);
    assert_eq!(elegir_via(&spec, None), Ok(Decision::Pregunta));
    assert_eq!(
        elegir_via(&spec, Some(Via::Build)),
        Ok(Decision::Via(Via::Build))
    );
    assert_eq!(
        elegir_via(&spec, Some(Via::Download)),
        Ok(Decision::Via(Via::Download))
    );
}

/// Un modelo con **un solo** camino usa ese camino, y el `--prio` **se
/// ignora** (no es un error y no aborta).
#[test]
fn un_modelo_con_un_solo_camino_ignora_el_prio() {
    let spec = spec(SOLO_DOWNLOAD);
    assert_eq!(vias_de(&spec).unica(), Some(Via::Download));
    assert_eq!(elegir_via(&spec, None), Ok(Decision::Via(Via::Download)));
    assert_eq!(
        elegir_via(&spec, Some(Via::Build)),
        Ok(Decision::Via(Via::Download)),
        "el --prio=build se ignora: el modelo no tiene esa vía"
    );
    let ignorado = prio_ignorado(&spec, Some(Via::Build)).expect("aviso de ignorado");
    assert_eq!(ignorado.via, Via::Build);
    assert_eq!(ignorado.motivo, Motivo::SinCamino);
}

/// Un modelo con un solo camino y el `--prio` de ese camino también se
/// ignora: ya es su comportamiento por defecto.
#[test]
fn un_prio_que_ya_es_el_defecto_se_ignora() {
    let spec = spec(SOLO_BUILD);
    assert_eq!(
        elegir_via(&spec, Some(Via::Build)),
        Ok(Decision::Via(Via::Build))
    );
    let ignorado = prio_ignorado(&spec, Some(Via::Build)).expect("aviso de ignorado");
    assert_eq!(ignorado.via, Via::Build);
    assert_eq!(ignorado.motivo, Motivo::YaEsElDefecto);
}

/// Un modelo con las dos vías **no** ignora el `--prio`: lo aplica.
#[test]
fn un_modelo_con_las_dos_vias_no_ignora_el_prio() {
    let spec = spec(DOS_VIAS);
    assert!(prio_ignorado(&spec, Some(Via::Build)).is_none());
}

/// Un modelo que **no declara** ninguna vía es un error que dice qué pasa.
#[test]
fn un_modelo_sin_vias_es_error() {
    let spec = spec(SIN_VIAS);
    let error = elegir_via(&spec, None).expect_err("debe fallar");
    assert_eq!(
        error,
        Error::SinVias {
            modelo: "vacio".to_string()
        }
    );
    let texto = error.to_string();
    assert!(texto.contains("vacio"), "{texto}");
    assert!(texto.contains("[download]"), "{texto}");
    assert!(texto.contains("[build]"), "{texto}");
}

// ─────────────────────────────────────────────────────────────────────────────
// La lista de ids y alias
// ─────────────────────────────────────────────────────────────────────────────

/// La lista acepta **espacios o comas**, y los huecos vacíos se ignoran.
#[test]
fn la_lista_acepta_espacios_y_comas() {
    let tokens: Vec<String> = ["nemotron,groot", " qwen , ", ""]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(parse_lista(&tokens), vec!["nemotron", "groot", "qwen"]);
}

// ─────────────────────────────────────────────────────────────────────────────
// La resolución de nombres
// ─────────────────────────────────────────────────────────────────────────────

/// Un modelo se resuelve por su **id**.
#[test]
fn un_modelo_se_resuelve_por_su_id() {
    let cat = build_catalog(vec![spec(DOS_VIAS)], vec![]).expect("catálogo");
    let encontrado = resuelve_modelo(&cat, "nemotron").expect("resuelto");
    assert_eq!(encontrado.model.name, "nemotron");
}

/// Un modelo se resuelve por su **alias** (`[model].alias`).
#[test]
fn un_modelo_se_resuelve_por_su_alias() {
    let toml = DOS_VIAS.replace(
        "name = \"nemotron\"",
        "name = \"nemotron\"\nalias = \"nemotron-mini\"",
    );
    let cat = build_catalog(vec![spec(&toml)], vec![]).expect("catálogo");
    let encontrado = resuelve_modelo(&cat, "nemotron-mini").expect("resuelto");
    // El alias devuelve el **modelo real**, no una fila distinta.
    assert_eq!(ropero_core::real_name(encontrado), "nemotron");
    // Y el alias responde igual que el nombre.
    let por_id = resuelve_modelo(&cat, "nemotron").expect("resuelto");
    assert_eq!(
        ropero_core::real_name(encontrado),
        ropero_core::real_name(por_id)
    );
}

/// Un modelo se resuelve por su **nombre real**, aunque se le llame por un
/// alias.
#[test]
fn un_modelo_se_resuelve_por_su_nombre_real() {
    let toml = DOS_VIAS.replace(
        "name = \"nemotron\"",
        "name = \"nemotron\"\nalias = \"nemotron-mini\"",
    );
    let cat = build_catalog(vec![spec(&toml)], vec![]).expect("catálogo");
    let encontrado = resuelve_modelo(&cat, "nemotron").expect("resuelto");
    assert_eq!(ropero_core::real_name(encontrado), "nemotron");
}

/// Un nombre que **no** está en el catálogo es un error de uso.
#[test]
fn un_modelo_desconocido_es_error_de_uso() {
    let cat = build_catalog(vec![spec(DOS_VIAS)], vec![]).expect("catálogo");
    let error = resuelve_modelo(&cat, "inventado").expect_err("debe fallar");
    assert_eq!(
        error,
        Error::ModeloDesconocido {
            nombre: "inventado".to_string()
        }
    );
    assert_eq!(error.exit_code(), 2);
}

/// Un modelo que **reclama** el nombre de otro por su `[aliases]`.
const RECLAMA: &str = r#"
[model]
name = "dos"
engine = "llama-server"

[files]
gguf = "dos.gguf"

[download]
repo = "ejemplo/dos"
file = "dos.gguf"

[aliases]
nemotron = "low"
"#;

/// Si **dos** modelos responden al mismo nombre, el error **nombra el
/// conflicto** (R-MOD-9).
#[test]
fn dos_modelos_con_el_mismo_alias_nombran_el_conflicto() {
    let cat = build_catalog(vec![spec(DOS_VIAS), spec(RECLAMA)], vec![]).expect("catálogo");
    let error = resuelve_modelo(&cat, "nemotron").expect_err("debe fallar");
    let texto = error.to_string();
    match &error {
        Error::AliasEnConflicto { alias, modelos } => {
            assert_eq!(alias, "nemotron");
            assert_eq!(modelos, &vec!["nemotron".to_string(), "dos".to_string()]);
        }
        otra => panic!("esperaba un conflicto de alias: {otra:?}"),
    }
    assert!(texto.contains("nemotron"), "{texto}");
    assert!(texto.contains("dos"), "{texto}");
}

// ─────────────────────────────────────────────────────────────────────────────
// El menú de modelos
// ─────────────────────────────────────────────────────────────────────────────

/// El manifiesto de un motor, tal y como lo escribe el catálogo.
fn motor(id: &str, color: &str) -> ropero_core::EngineManifest {
    let toml = format!(
        "id = \"{id}\"\nlabel = \"{id}\"\ncolor = \"{color}\"\n\
         description = \"d\"\nmaturity = \"proven\"\nformats = [\"gguf\"]\n\
         port_min = 8100\nport_max = 8199\n\n[detect]\nbinaries = [\"{id}\"]\npaths = []\n"
    );
    parse_engine_manifest(&toml, Path::new("engines/motor.toml")).expect("manifiesto")
}

/// El menú de modelos sólo ofrece los de los motores **instalados** (T-25).
#[test]
fn el_menu_de_modelos_solo_ofrece_los_de_los_motores_instalados() {
    let llama = motor("llama-server", "naranja");
    let vllm = motor("vllm", "morado");
    let cat = build_catalog(
        vec![
            spec(DOS_VIAS),
            spec_en(
                &SOLO_DOWNLOAD
                    .replace("nemotron", "groot")
                    .replace("llama-server", "vllm"),
                "groot.toml",
            ),
        ],
        vec![],
    )
    .expect("catálogo");
    let paleta = Palette::plain();
    let motores = [
        MotorDelMenu {
            manifest: &llama,
            instalado: true,
        },
        MotorDelMenu {
            manifest: &vllm,
            instalado: false,
        },
    ];
    let menu = menu_modelos(&cat, &motores, &paleta);
    assert_eq!(menu.nombres, vec!["nemotron".to_string()]);
    // Con el motor **no** instalado, su modelo no se ofrece.
    let motores = [MotorDelMenu {
        manifest: &vllm,
        instalado: false,
    }];
    let menu = menu_modelos(&cat, &motores, &paleta);
    assert!(menu.nombres.is_empty(), "{:?}", menu.nombres);
}

/// Un modelo de **descarga manual** no se ofrece en el menú (R-DATA-3).
///
/// Son checkpoints enormes que el instalador no baja por sorpresa; si el
/// usuario lo pide a mano, sí se baja.
#[test]
fn el_menu_de_modelos_no_ofrece_los_de_descarga_manual() {
    let llama = motor("llama-server", "naranja");
    let cat = build_catalog(vec![spec(DOS_VIAS), spec_en(MANUAL, "manual.toml")], vec![])
        .expect("catálogo");
    let motores = [MotorDelMenu {
        manifest: &llama,
        instalado: true,
    }];
    let menu = menu_modelos(&cat, &motores, &Palette::plain());
    assert_eq!(menu.nombres, vec!["nemotron".to_string()]);
}

/// El menú de motores numera sus opciones y lleva su color.
#[test]
fn el_menu_de_motores_numera_y_colorea() {
    let llama = motor("llama-server", "naranja");
    let motores = [MotorDelMenu {
        manifest: &llama,
        instalado: true,
    }];
    let menu = menu_engines(&motores, &Palette::plain());
    assert_eq!(menu.nombres, vec!["llama-server".to_string()]);
    assert!(menu.lineas[0].contains("  1"), "{:?}", menu.lineas);
    assert!(menu.lineas[0].contains("llama-server"), "{:?}", menu.lineas);
    assert!(menu.lineas[0].contains("naranja"), "{:?}", menu.lineas);
    assert!(menu.lineas[0].contains("ya instalado"), "{:?}", menu.lineas);
}

/// La respuesta del menú acepta **números** y **nombres**.
#[test]
fn la_respuesta_del_menu_acepta_numeros_y_nombres() {
    let opciones = vec!["llama-server".to_string(), "vllm".to_string()];
    assert_eq!(
        resuelve_seleccion("2", &opciones).expect("selección"),
        vec!["vllm".to_string()]
    );
    assert_eq!(
        resuelve_seleccion("1 2", &opciones).expect("selección"),
        vec!["llama-server".to_string(), "vllm".to_string()]
    );
    assert_eq!(
        resuelve_seleccion("vllm", &opciones).expect("selección"),
        vec!["vllm".to_string()]
    );
    assert!(resuelve_seleccion("9", &opciones).is_err());
}
