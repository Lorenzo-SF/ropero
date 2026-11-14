//! `ropero install`: los motores y los modelos.
//!
//! La CLI sólo **pega** (R-ARQ-1): el contrato de `--prio`, los menús y
//! los mensajes son del dominio ([`ropero_core::install`]), y la E/S —el
//! terminal, el binario del motor y el enlace— es del runtime
//! ([`ropero_runtime::install`]).
//!
//! ## Las tres formas de invocarlo
//!
//! | Invocación | Qué hace |
//! |---|---|
//! | `ropero install` | engines y luego models, en ese orden |
//! | `ropero install engines [<lista>]` | instala motores |
//! | `ropero install models [<lista>]` | resuelve modelos y decide su vía |
//!
//! Con **lista** se instala **directo** (modo desatendido, R-CLI-16); sin
//! lista hay **menú**, y sin terminal el menú es un **error de uso**
//! (código 2).

use clap::ArgMatches;

use ropero_core::EngineRow;
use ropero_core::Palette;
use ropero_core::install::{self, Decision, Token, Via};

/// `ropero install [<nada>|engines|models]`.
pub fn run(sub: &ArgMatches) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let tokens = crate::parser::install_tokens(sub);
    match sub.get_one::<String>("que").map(String::as_str) {
        Some("engines") => engines(&s, &tokens),
        Some("models") => models(&s, &tokens),
        Some(otro) => uso(&format!(
            "'install' sólo acepta 'engines' o 'models' (recibido: '{otro}')"
        )),
        None => {
            // `ropero install` a secas: engines y luego models (doc CLI v2 §9).
            let codigo = engines(&s, &tokens);
            if codigo != 0 {
                return codigo;
            }
            models(&s, &tokens)
        }
    }
}

/// `ropero install engines [<lista>]`.
fn engines(s: &crate::discovery::Scan, tokens: &[Token]) -> i32 {
    let sistema = ropero_runtime::capture_system_info();
    let filas = ropero_core::engine_rows(&s.engines, &sistema, true);
    let lista = install::nombres_de(tokens);
    let eleccion = if lista.is_empty() {
        match elegir_motores(&filas) {
            Ok(elegidos) => elegidos,
            Err(codigo) => return codigo,
        }
    } else {
        lista
    };
    let elegidos = match install::motores_elegidos(&filas, &eleccion) {
        Ok(elegidos) => elegidos,
        Err(e) => return uso(&e.to_string()),
    };
    for aviso in &elegidos.avisos {
        ropero_core::err_line(&format!("aviso: {aviso}"));
    }
    for fila in elegidos.instalar {
        let codigo = enlaza_motor(&fila.manifest, &s.env);
        if codigo != 0 {
            return codigo;
        }
    }
    0
}

/// Instala un motor: sólo crea el **enlace** (R-INST-1).
fn enlaza_motor(manifest: &ropero_core::EngineManifest, env: &ropero_core::Environment) -> i32 {
    match ropero_runtime::install::instala_motor(manifest, env) {
        Ok(estado) => {
            let (mensaje, codigo) = install::mensaje_motor(manifest.id.as_str(), &estado);
            if codigo == 0 {
                ropero_core::out_line(&mensaje);
            } else {
                ropero_core::err_line(&mensaje);
            }
            codigo
        }
        Err(e) => {
            ropero_core::err_line(&format!("error: {e}"));
            1
        }
    }
}

/// El menú de motores: sin terminal es un **error de uso** (R-INST-26).
fn elegir_motores(filas: &[EngineRow]) -> Result<Vec<String>, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_tty_engines()));
    }
    let compatibles: Vec<install::MotorDelMenu<'_>> = filas
        .iter()
        .filter(|f| f.compatible)
        .map(|f| install::MotorDelMenu {
            manifest: &f.manifest,
            instalado: ropero_runtime::engine_installed(&f.manifest),
        })
        .collect();
    let menu = install::menu_engines(&compatibles, &Palette::color());
    if menu.lineas.is_empty() {
        ropero_core::err_line("aviso: no hay motores compatibles con esta máquina");
        return Err(0);
    }
    ropero_runtime::install::pregunta_menu(&menu.lineas, &menu.nombres).map_err(|e| uso(&e))
}

/// `ropero install models [<lista>]`.
fn models(s: &crate::discovery::Scan, tokens: &[Token]) -> i32 {
    let prios = match install::asocia_tokens(tokens) {
        Ok(prios) => prios,
        Err(e) => return uso(&e.to_string()),
    };
    let lista = install::nombres_de(tokens);
    // Con lista se instala **directo** (desatendido): el usuario ya ha
    // dicho qué quiere, así que no se pregunta (doc CLI v2 §9.2).
    let desatendido = !lista.is_empty();
    let eleccion = if lista.is_empty() {
        match elegir_modelos(s) {
            Ok(elegidos) => elegidos,
            Err(codigo) => return codigo,
        }
    } else {
        lista
    };
    let elegidos = match install::modelos_elegidos(&s.catalog, &eleccion, &prios) {
        Ok(elegidos) => elegidos,
        Err(e) => return uso(&e.to_string()),
    };
    let modo = if desatendido {
        ropero_download::Modo::Desatendido
    } else {
        ropero_download::Modo::Interactivo
    };
    let mut codigo = 0;
    for elegido in elegidos {
        // Un `--prio` ignorado **se avisa** y no aborta (doc B §4.10.7).
        if let Some(aviso) = &elegido.aviso {
            ropero_core::err_line(aviso);
        }
        let via = match install::elegir_via(elegido.spec, elegido.prio) {
            Ok(Decision::Via(via)) => via,
            Ok(Decision::Pregunta) => match preguntar_via(&elegido.nombre) {
                Ok(via) => via,
                Err(codigo) => return codigo,
            },
            Err(e) => {
                ropero_core::err_line(&format!("error: {e}"));
                return e.exit_code();
            }
        };
        match via {
            Via::Download => {
                if baja_modelo(&elegido.nombre, elegido.spec, &s.env, modo) != 0 {
                    codigo = 1;
                }
            }
            Via::Build => {
                ropero_core::out_line(&install::via_build_pendiente(&elegido.nombre));
                ropero_core::err_line(&install::pista_build_pendiente());
                codigo = 1;
            }
        }
    }
    codigo
}

/// Baja un modelo de verdad: la vía `download` llama a `hf`.
///
/// El plan lo decide `ropero-download` (qué se baja, a dónde y con qué
/// argumentos) y la ejecución es suya (R-MOD-16). Aquí sólo se informa del
/// resultado, una línea por modelo.
fn baja_modelo(
    nombre: &str,
    spec: &ropero_core::ModelSpec,
    env: &ropero_core::Environment,
    modo: ropero_download::Modo,
) -> i32 {
    if spec.download.as_ref().is_some_and(|d| d.manual) {
        ropero_core::err_line(&format!(
            "aviso: '{nombre}' es de descarga manual (grande): se baja por petición explícita"
        ));
    }
    let plan = match ropero_download::plan_de_descarga(spec, env) {
        Ok(plan) => plan,
        Err(e) => return informa_error(nombre, &e),
    };
    match ropero_download::descarga(&plan, env, modo) {
        Ok(resultado) => {
            ropero_core::out_line(&ropero_download::linea(nombre, Via::Download, resultado));
            0
        }
        Err(e) => informa_error(nombre, &e),
    }
}

/// Informa de un error de descarga: la línea del modelo y su pista.
fn informa_error(nombre: &str, e: &ropero_download::Error) -> i32 {
    ropero_core::out_line(&ropero_download::linea_error(nombre, Via::Download, e));
    if let Some(pista) = e.pista() {
        ropero_core::err_line(&pista);
    }
    1
}

/// El menú de modelos: sólo los de los motores **instalados** (T-25).
fn elegir_modelos(s: &crate::discovery::Scan) -> Result<Vec<String>, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_tty_models()));
    }
    let motores: Vec<install::MotorDelMenu<'_>> = s
        .engines
        .iter()
        .map(|manifest| install::MotorDelMenu {
            manifest,
            instalado: ropero_runtime::engine_installed(manifest),
        })
        .collect();
    let menu = install::menu_modelos(&s.catalog, &motores, &Palette::color());
    if menu.lineas.is_empty() {
        ropero_core::err_line(
            "aviso: no hay modelos de motores instalados (ropero install engines)",
        );
        return Err(0);
    }
    ropero_runtime::install::pregunta_menu(&menu.lineas, &menu.nombres).map_err(|e| uso(&e))
}

/// La pregunta de qué vía se usa: sin terminal es un **error de uso**.
fn preguntar_via(modelo: &str) -> Result<Via, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_prio(modelo)));
    }
    ropero_runtime::install::pregunta_via(&install::pregunta_de_via(modelo)).map_err(|e| uso(&e))
}

/// Un **error de uso**: el mensaje dice qué hacer y el código es 2.
fn uso(mensaje: &str) -> i32 {
    ropero_core::err_line(&ropero_core::surface::usage_message("install", mensaje));
    2
}
