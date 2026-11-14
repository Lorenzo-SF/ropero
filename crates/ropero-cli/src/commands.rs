//! Los comandos que hacen su trabajo: `list`, `engines`, `levels`, `path`,
//! `status`, `doctor`, `template`, `validate` y `args`.
//!
//! Todos comparten la misma forma: escanear (o capturar), componer el
//! texto en el dominio e imprimirlo. Por eso viven juntos: separarlos en
//! nueve ficheros de veinte líneas no añade nada, y el CLI tiene un
//! **límite de líneas** que es un contrato, no una sugerencia.

use ropero_core::Catalog;

/// `ropero list [--all]`: los modelos **disponibles**, agrupados por motor.
///
/// "Disponible" son las dos cosas a la vez (doc CLI v2 §15): el motor está
/// instalado y el modelo está descargado.
pub fn list(all: bool) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let foto = crate::discovery::snapshot(&s);
    let sistema = ropero_runtime::capture_system_info();
    let razones = ropero_core::snapshot::razones_por_motor(&s.engines, &sistema);
    let paleta = ropero_core::Palette::color();
    let lineas = ropero_core::listing::list_lines(&foto, all, &paleta, &razones);
    if lineas.is_empty() {
        ropero_core::err_line(&ropero_core::listing::empty_notice());
        return 0;
    }
    for linea in &lineas {
        ropero_core::out_line(linea);
    }
    0
}

/// `ropero status`: el estado de la máquina, tipo `nvidia-smi`.
pub fn status() -> i32 {
    let maquina = ropero_runtime::capture_machine_state();
    let texto = ropero_core::status::status_text(&maquina);
    ropero_core::out(&texto, false);
    0
}

/// `ropero engines [--all]`: la lista de motores con su color, su madurez
/// y si son compatibles con esta máquina.
pub fn engines(all: bool) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let sistema = ropero_runtime::capture_system_info();
    let lineas = ropero_core::engine_lines(&s.engines, &sistema, all);
    if lineas.is_empty() {
        ropero_core::err_line("aviso: no hay motores que listar");
        return 0;
    }
    for linea in &lineas {
        ropero_core::out_line(linea);
    }
    0
}

/// `ropero levels <modelo>`.
pub fn levels(modelo: &str) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let Some(spec) = find(&s.catalog, modelo) else {
        return unknown_model("levels", modelo);
    };
    for linea in ropero_core::level_lines(spec) {
        ropero_core::out_line(&linea);
    }
    0
}

/// `ropero path <modelo>`.
pub fn path(modelo: &str) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let Some(spec) = find(&s.catalog, modelo) else {
        return unknown_model("path", modelo);
    };
    let paths = ropero_core::resolve_paths(spec, &s.env);
    for linea in ropero_core::path_lines(&paths) {
        ropero_core::out_line(&linea);
    }
    0
}

/// `ropero template`: vuelca la plantilla del catálogo.
pub fn template() -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let candidates = ropero_core::template_candidates(&s.root, &s.base);
    let Some(path) = candidates.into_iter().find(|p| ropero_runtime::existe(p)) else {
        ropero_core::err_line("error: no encuentro la plantilla");
        return 1;
    };
    match std::fs::read_to_string(&path) {
        Ok(c) => {
            ropero_core::out(&c, false);
            0
        }
        Err(e) => {
            ropero_core::err_line(&format!("error: {}", e));
            1
        }
    }
}

/// `ropero validate`: valida los TOML del catálogo y de los motores.
pub fn validate() -> i32 {
    use ropero_core::{validate_catalog, validate_engine_set};
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let errores = validate_catalog(&s.catalog, &s.texts);
    let errores_motor = validate_engine_set(&s.engines);
    let mut exit = 0;
    for error in &errores {
        ropero_core::err_line(&format!("{}:{}", error.path(), error.line().unwrap_or(0)));
        ropero_core::err_line(&format!("  {}", error.message()));
        exit = exit.max(error.exit_code());
    }
    for error in &errores_motor {
        ropero_core::err_line(&format!("engines: {error}"));
        exit = exit.max(1);
    }
    for problema in &s.problems {
        ropero_core::err_line(problema);
        exit = exit.max(1);
    }
    if exit == 0 {
        ropero_core::out_line(&ropero_core::validate_ok_line(s.files, &s.base));
    } else {
        ropero_core::err_line(&ropero_core::validate_failed_line(&s.base));
    }
    exit
}

/// `ropero args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]`.
pub fn args(modelo: &str, tokens: &[String]) -> i32 {
    if let Some(tok) = ropero_core::first_invalid_token(tokens) {
        let msg = ropero_core::surface::invalid_token_message(tok);
        ropero_core::err_line(&ropero_core::surface::usage_message("args", &msg));
        return 2;
    }
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let Some(spec) = find(&s.catalog, modelo) else {
        return unknown_model("args", modelo);
    };
    let parsed = ropero_core::parse_request(spec, tokens);
    if let Some(prev) = &parsed.repeated_level {
        let now = parsed.level.as_deref().unwrap_or_default();
        ropero_core::err_line(&ropero_core::surface::repeated_level_notice(prev, now));
    }
    let req = parsed.to_launch_request();
    let ctx = ropero_core::resolve_context(spec, &s.env);
    match ropero_core::build_argv(spec, &req, &ctx) {
        Ok(argv) => {
            for a in argv {
                ropero_core::out_line(&a);
            }
            0
        }
        Err(e) => {
            ropero_core::err_line(&ropero_core::surface::argv_error_message(&e.to_string()));
            2
        }
    }
}

/// `ropero <modelo>`: arranca el modelo en su motor.
///
/// La lógica **no** vive aquí: el runtime sabe lo que es un arranque
/// (R-ARQ-1). Aquí sólo se escanea el catálogo y se le pasa al runtime,
/// que escupe el código de salida.
pub fn arranca(peticion: &ropero_runtime::RunRequest) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    ropero_runtime::arranca_todo(peticion, &s.catalog, &s.engines, &s.env)
}

/// `ropero stop [<modelo>]`: para los motores lanzados por ropero.
///
/// Sin argumento para **todos**; con argumento, sólo el de ese modelo.
pub fn stop(modelo: &str) -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    ropero_runtime::para_todo(modelo, &s.env)
}

/// `ropero doctor`: las dependencias de ropero y las de cada motor.
///
/// El orden es el del doc CLI v2 §14: primero las del propio ropero, luego
/// las de cada motor (disponible o no). Sale con **0** si todo está bien, y
/// con **1** si falta algo.
pub fn doctor() -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let informe = ropero_core::doctor::DoctorReport {
        ropero: comprueba_ropero(),
        engines: s
            .engines
            .iter()
            .map(|m| ropero_core::doctor::EngineDeps {
                installed: ropero_runtime::engine_installed(m),
                manifest: m.clone(),
                checks: comprueba_deps(&m.deps),
            })
            .collect(),
    };
    for linea in informe.lines() {
        ropero_core::out_line(&linea);
    }
    let resumen = informe.summary();
    if informe.todo_ok() {
        ropero_core::out_line(&resumen);
        0
    } else {
        ropero_core::err_line(&resumen);
        1
    }
}

/// Las dependencias del propio ropero.
///
/// La primera es el propio binario: que esté en el `PATH` es lo que
/// permite invocarlo sin escribir la ruta entera.
fn comprueba_ropero() -> Vec<ropero_core::doctor::DepCheck> {
    ropero_core::doctor::DEPENDENCIAS_ROPERO
        .iter()
        .map(|(nombre, _)| ropero_core::doctor::DepCheck {
            dependency: ropero_core::doctor::dependencia_ropero(nombre),
            status: if esta_en_path(nombre) {
                ropero_core::doctor::DepStatus::Ok
            } else {
                ropero_core::doctor::DepStatus::Missing
            },
        })
        .collect()
}

/// Las dependencias de un motor.
fn comprueba_deps(deps: &[String]) -> Vec<ropero_core::doctor::DepCheck> {
    deps.iter()
        .map(|nombre| ropero_core::doctor::DepCheck {
            dependency: ropero_core::doctor::Dependency {
                name: nombre.clone(),
                remedy: "instálalo con tu gestor de paquetes".to_string(),
            },
            status: if ropero_runtime::which(nombre).is_some() {
                ropero_core::doctor::DepStatus::Ok
            } else {
                ropero_core::doctor::DepStatus::Missing
            },
        })
        .collect()
}

/// Si la dependencia está satisfied.
///
/// La primera es el propio binario `ropero`: se busca por su nombre de
/// ejecutable, no por la palabra "ropero".
fn esta_en_path(nombre: &str) -> bool {
    if nombre == "ropero" {
        return ropero_runtime::ropero_en_path();
    }
    ropero_runtime::which(nombre).is_some()
}

/// Busca un modelo en el catálogo.
fn find<'a>(cat: &'a Catalog, modelo: &str) -> Option<&'a ropero_core::ModelSpec> {
    ropero_core::find_spec(cat, modelo)
}

/// El mensaje de un modelo que no está en el catálogo (error de uso, 2).
fn unknown_model(cmd: &str, modelo: &str) -> i32 {
    let msg = ropero_core::surface::unknown_model_message(modelo);
    ropero_core::err_line(&ropero_core::surface::usage_message(cmd, &msg));
    2
}
