//! Interactive and headless `ropero test` execution.

use clap::ArgMatches;
use ropero_bateria::seleccion::seleccion_de;
use ropero_runtime::inferencia::{ErrorInferencia, tira};
use std::path::PathBuf;

pub fn run(sub: &ArgMatches) -> i32 {
    let model = value(sub, "model");
    let all_models = sub.get_flag("all-models");
    let test = value(sub, "test");
    let group = value(sub, "test-group");
    let path = value(sub, "path");
    let has_args =
        model.is_some() || all_models || test.is_some() || group.is_some() || path.is_some();
    let (model, all_models, selection, path) = if !has_args {
        match interactive() {
            Ok(result) => (
                result.model,
                result.all_models,
                result.selection,
                result.path,
            ),
            Err(error) => return ErrorInferencia::uso(error).informa(),
        }
    } else {
        if model.is_some() == all_models {
            return ErrorInferencia::uso("indica exactamente uno de --model o --all-models")
                .informa();
        }
        if test.is_none() && group.is_none() {
            return ErrorInferencia::uso("indica exactamente una opción: --test o --test-group")
                .informa();
        }
        let selection = match seleccion_de(ropero_bateria::seleccion::Opciones {
            grupo: group,
            test,
            ..Default::default()
        }) {
            Ok(selection) => selection,
            Err(error) => return ErrorInferencia::uso(error.mensaje).informa(),
        };
        (
            model,
            all_models,
            selection,
            path.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .display()
                    .to_string()
            }),
        )
    };

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let path = match ropero_runtime::path::expand_result_path(&path, &home, |name| {
        std::env::var(name).ok()
    }) {
        Ok(path) => path,
        Err(error) => return ErrorInferencia::uso(error).informa(),
    };
    let Ok(scan) = crate::discovery::scan() else {
        return 1;
    };
    let models = if all_models {
        ropero_core::snapshot::available_models(&crate::discovery::snapshot(&scan))
            .into_iter()
            .map(|entry| entry.spec.model.name.clone())
            .collect::<Vec<_>>()
    } else {
        model.into_iter().collect()
    };
    if models.is_empty() {
        return ErrorInferencia::uso(if scan.config.lang == "EN" {
            "there are no available models to test"
        } else {
            "no hay modelos disponibles para probar"
        })
        .informa();
    }
    models.into_iter().fold(0, |exit, model| {
        exit.max(tira(
            &scan.catalog,
            &scan.engines,
            &scan.env,
            &scan.root,
            Some(model),
            &selection,
            &path,
            &scan.config.lang,
        ))
    })
}

fn interactive() -> Result<ropero_runtime::test_wizard::TestWizardResult, String> {
    let scan = crate::discovery::scan().map_err(|_| {
        if crate::parser::idioma() == "EN" {
            "could not read the model catalog"
        } else {
            "no se pudo leer el catálogo"
        }
        .to_string()
    })?;
    let snapshot = crate::discovery::snapshot(&scan);
    let available = ropero_core::snapshot::available_models(&snapshot);
    let models = available
        .into_iter()
        .map(|entry| {
            let engine_id = entry.spec.model.engine;
            let engine = scan
                .engines
                .iter()
                .find(|manifest| manifest.id == engine_id)
                .map(|manifest| (manifest.label.clone(), Some(manifest.color)));
            let (label, color) = engine.unwrap_or_else(|| (engine_id.as_str().to_string(), None));
            (entry.spec.model.name.clone(), label, color)
        })
        .collect::<Vec<_>>();
    ropero_runtime::test_wizard::choose(&models, &scan.config.lang)
}

fn value(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.get_one::<String>(name).cloned()
}
