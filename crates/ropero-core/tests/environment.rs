//! Tests del entorno capturado: `expand_path` y `env_path`.
//!
//! `ropero-core` es dominio puro: **no** lee el entorno real. Los tests
//! construyen el contexto a mano (variables, `$HOME` y directorio de
//! trabajo), así que no dependen de la máquina que los ejecuta.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ropero_core::Environment;

/// Un entorno de pruebas: `$HOME` en `/home/usuario` y cwd en `/trabajo`.
fn entorno() -> Environment {
    Environment::new(BTreeMap::new(), "/home/usuario", "/trabajo")
}

/// Un entorno con una variable de entorno puesta.
fn entorno_con(vars: &[(&str, &str)]) -> Environment {
    let mapa: BTreeMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    Environment::new(mapa, "/home/usuario", "/trabajo")
}

// ─────────────────────────────────────────────────────────────────────────────
// `expand_path`
// ─────────────────────────────────────────────────────────────────────────────

/// `~` solo es el `$HOME`.
#[test]
fn expand_path_con_tilde_solo_es_el_home() {
    assert_eq!(entorno().expand_path("~"), PathBuf::from("/home/usuario"));
}

/// `~/algo` cuelga del `$HOME`.
#[test]
fn expand_path_con_tilde_diagonal_cuelga_del_home() {
    assert_eq!(
        entorno().expand_path("~/modelos/gguf"),
        PathBuf::from("/home/usuario/modelos/gguf")
    );
}

/// Una ruta **relativa** cuelga del directorio de trabajo.
#[test]
fn expand_path_con_ruta_relativa_cuelga_del_cwd() {
    assert_eq!(
        entorno().expand_path("modelos/gguf"),
        PathBuf::from("/trabajo/modelos/gguf")
    );
}

/// Una ruta **absoluta** se queda como está: no se le cuelga nada.
#[test]
fn expand_path_con_ruta_absoluta_no_la_toca() {
    assert_eq!(
        entorno().expand_path("/opt/modelos/gguf"),
        PathBuf::from("/opt/modelos/gguf")
    );
}

/// Un `~` que **no** va solo ni seguido de `/` no es el `$HOME`: es un
/// nombre de fichero cualquiera.
#[test]
fn expand_path_no_expande_un_tilde_que_no_es_el_home() {
    assert_eq!(
        entorno().expand_path("~otro"),
        PathBuf::from("/trabajo/~otro")
    );
}

/// La ruta vacía cuelga del directorio de trabajo (es relativa).
#[test]
fn expand_path_con_ruta_vacia_cuelga_del_cwd() {
    assert_eq!(entorno().expand_path(""), PathBuf::from("/trabajo"));
}

/// Un `..` relativo se cuelga del cwd **sin** normalizar: la normalización
/// es del sistema de ficheros, no del dominio.
#[test]
fn expand_path_no_normaliza_los_puntos() {
    assert_eq!(
        entorno().expand_path("../otro"),
        PathBuf::from("/trabajo/../otro")
    );
}

/// La expansión depende del `$HOME` capturado, no del real.
#[test]
fn expand_path_usa_el_home_capturado() {
    let env = Environment::new(BTreeMap::new(), "/otro/home", "/otro/trabajo");
    assert_eq!(env.expand_path("~/x"), PathBuf::from("/otro/home/x"));
    assert_eq!(env.expand_path("x"), PathBuf::from("/otro/trabajo/x"));
}

// ─────────────────────────────────────────────────────────────────────────────
// `env_path`
// ─────────────────────────────────────────────────────────────────────────────

/// Si la variable está definida, su valor manda (expandido).
#[test]
fn env_path_con_variable_definida_usa_su_valor() {
    let env = entorno_con(&[("GGUF_DIR", "/mnt/gguf")]);
    assert_eq!(
        env.env_path("GGUF_DIR", "models/gguf"),
        PathBuf::from("/mnt/gguf")
    );
}

/// Si la variable está definida con una ruta relativa, se cuelga del cwd.
#[test]
fn env_path_con_variable_relativa_cuelga_del_cwd() {
    let env = entorno_con(&[("GGUF_DIR", "datos/gguf")]);
    assert_eq!(
        env.env_path("GGUF_DIR", "models/gguf"),
        PathBuf::from("/trabajo/datos/gguf")
    );
}

/// Si la variable no está, el valor por defecto cuelga del `$HOME`.
#[test]
fn env_path_sin_variable_usa_el_default_bajo_el_home() {
    let env = entorno();
    assert_eq!(
        env.env_path("GGUF_DIR", "models/gguf"),
        PathBuf::from("/home/usuario/models/gguf")
    );
}

/// El valor por defecto puede venir escrito con `~/`: se quita el `~/` (el
/// default es relativo al `$HOME`, no al cwd).
#[test]
fn env_path_sin_variable_quita_el_tilde_del_default() {
    let env = entorno();
    assert_eq!(
        env.env_path("GGUF_DIR", "~/models/gguf"),
        PathBuf::from("/home/usuario/models/gguf")
    );
}

/// El valor por defecto **con** `~` y **con** `/` delante: el `~/` se quita
/// entero y el resto cuelga del `$HOME`.
#[test]
fn env_path_sin_variable_quita_el_tilde_y_la_diagonal() {
    let env = entorno();
    assert_eq!(
        env.env_path("GGUF_DIR", "~/models/gguf"),
        PathBuf::from("/home/usuario/models/gguf")
    );
    // `LOG_DIR` es una ruta gobernada por la configuración: su defecto es
    // `<config>/logs`, no `~/.logs/ropero` ni `~/logs`.
    assert_eq!(
        env.env_path("LOG_DIR", "logs"),
        PathBuf::from("/home/usuario/.config/ropero/logs")
    );
}

/// Una variable **definida** gana sobre el valor por defecto, aunque esté
/// vacía: el valor vacío es el que dice el usuario.
#[test]
fn env_path_con_variable_vacia_gana_al_default() {
    let env = entorno_con(&[("GGUF_DIR", "")]);
    // La variable vacía es relativa: cuelga del cwd.
    assert_eq!(
        env.env_path("GGUF_DIR", "models/gguf"),
        PathBuf::from("/trabajo")
    );
}

/// `env_path` no lee el entorno real: sólo el mapa capturado.
#[test]
fn env_path_no_consulta_el_entorno_real() {
    // `PATH` existe en el entorno real de cualquier máquina; aquí no está en
    // el mapa, así que el valor por defecto manda.
    let env = entorno();
    assert_eq!(
        env.env_path("PATH", "bin"),
        PathBuf::from("/home/usuario/bin")
    );
    // Y una variable puesta en el mapa gana sobre la real.
    let env = entorno_con(&[("PATH", "/mi/bin")]);
    assert_eq!(env.env_path("PATH", "bin"), PathBuf::from("/mi/bin"));
}

// ─────────────────────────────────────────────────────────────────────────────
// `get`
// ─────────────────────────────────────────────────────────────────────────────

/// `get` lee del mapa capturado; una variable que no está devuelve `None`.
#[test]
fn get_devuelve_el_valor_del_mapa() {
    let env = entorno_con(&[("ROPERO_ROOT", "/catalogo")]);
    assert_eq!(env.get("ROPERO_ROOT"), Some("/catalogo"));
    assert_eq!(env.get("NO_EXISTE"), None);
}

// ─────────────────────────────────────────────────────────────────────────────
// `models_base`
// ─────────────────────────────────────────────────────────────────────────────

/// Con `models/` presente la base es `models/`; si no, la raíz.
#[test]
fn models_base_elige_models_cuando_existe() {
    assert_eq!(
        ropero_core::models_base(Path::new("/catalogo"), true),
        PathBuf::from("/catalogo/models")
    );
    assert_eq!(
        ropero_core::models_base(Path::new("/catalogo"), false),
        PathBuf::from("/catalogo")
    );
}
