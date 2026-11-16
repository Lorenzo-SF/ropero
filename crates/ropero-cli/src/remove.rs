//! `ropero <alias> remove`: elimina los artefactos derivables de un modelo.

use crate::discovery;
use ropero_core::{RemovalPlan, find_spec, removal_plan_in_catalog};

pub fn run(alias: &str, args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        ropero_core::out_line(
            "elimina los artefactos descargados de un modelo\n\nUSO:  ropero <alias> remove\n\nOPCIONES\n  -h, --help:  esta ayuda\n\n  Pide confirmación interactiva antes de borrar.\n  Solo elimina artefactos derivables del TOML del modelo.",
        );
        return 0;
    }
    if !args.is_empty() {
        ropero_core::err_line(&ropero_core::surface::usage_message(
            "<modelo> remove",
            &format!("opción desconocida: '{}'", args[0]),
        ));
        return 2;
    }
    let Ok(s) = discovery::scan() else {
        return 1;
    };
    let Some(spec) = find_spec(&s.catalog, alias) else {
        ropero_core::err_line(&ropero_core::surface::unknown_model_message(alias));
        return 1;
    };
    let plan = removal_plan_in_catalog(spec, &s.catalog, &s.env);
    mostrar_plan(alias, spec, &plan);
    if plan.is_blocked() {
        for m in &plan.blocked {
            ropero_core::err_line(&format!("error: {m}"));
        }
        ropero_core::err_line("no se ha eliminado nada");
        return 1;
    }
    if plan.is_empty() {
        if plan.notes.is_empty() {
            ropero_core::out_line("no hay artefactos derivables que eliminar");
        } else {
            ropero_core::out_line(
                "no quedan artefactos exclusivos que eliminar; los compartidos se conservan",
            );
        }
        return 0;
    }
    if !ropero_runtime::install::hay_tty() {
        ropero_core::err_line(
            "error: la eliminación requiere confirmación interactiva en un terminal (TTY); no se ha eliminado nada",
        );
        return 1;
    }
    match confirmar(alias, &plan) {
        Ok(true) => ropero_runtime::remove::ejecutar(&s.engines, spec, &plan),
        Ok(false) => {
            ropero_core::out_line("operación cancelada; no se ha eliminado nada");
            0
        }
        Err(msg) => {
            ropero_core::err_line(&format!("error: {msg}"));
            ropero_core::err_line("no se ha eliminado nada");
            1
        }
    }
}

fn mostrar_plan(alias: &str, spec: &ropero_core::ModelSpec, plan: &RemovalPlan) {
    let nombre = ropero_core::real_name(spec);
    ropero_core::out_line(&if nombre == alias {
        format!("modelo: {nombre}")
    } else {
        format!("modelo: {nombre} (alias: {alias})")
    });
    ropero_core::out_line(&format!("motor:  {}", spec.model.engine.as_str()));
    if let Some(flm) = &plan.flm {
        ropero_core::out_line("\ntags a eliminar mediante FLM:");
        for tag in &flm.all_tags {
            ropero_core::out_line(&format!("  - {tag}"));
        }
    }
    if !plan.files.is_empty() {
        ropero_core::out_line("\nficheros a eliminar (si existen):");
        for f in &plan.files {
            ropero_core::out_line(&format!("  - {}", f.display()));
        }
    }
    if !plan.directories.is_empty() {
        ropero_core::out_line("\ndirectorios de artefactos a eliminar (incluido su contenido):");
        for dir in &plan.directories {
            ropero_core::out_line(&format!("  - {}", dir.path.display()));
        }
    }
    for nota in &plan.notes {
        ropero_core::out_line(&format!("aviso: {nota}"));
    }
}

fn confirmar(alias: &str, plan: &RemovalPlan) -> Result<bool, String> {
    let resumen = plan
        .flm
        .as_ref()
        .map(|f| format!("{} tag(s) FLM", f.all_tags.len()))
        .unwrap_or_else(|| {
            format!(
                "{} fichero(s) y {} directorio(s) de artefactos",
                plan.files.len(),
                plan.directories.len()
            )
        });
    let respuesta = ropero_runtime::install::pregunta(&format!(
        "¿Eliminar {resumen} del modelo '{alias}'? Esta acción no se puede deshacer. [s/N]: "
    ))
    .ok_or_else(|| "no se pudo leer la confirmación; se cancela".to_string())?;
    let r = respuesta.trim().to_ascii_lowercase();
    Ok(r == "s" || r == "si" || r == "sí")
}
