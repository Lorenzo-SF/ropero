//! Interactive model and evaluation selection for `ropero test`.

use ropero_bateria::{
    indice::{Bateria, parse_indice},
    seleccion::Seleccion,
};
use ropero_core::{EngineColor, Palette, palette::HELP_HEADING};

const INDICE: &str = include_str!("../../../docs/tests/indice.md");

pub struct TestWizardResult {
    pub model: Option<String>,
    pub all_models: bool,
    pub selection: Seleccion,
    pub path: String,
}

pub fn choose(
    models: &[(String, String, Option<EngineColor>)],
    lang: &str,
) -> Result<TestWizardResult, String> {
    let english = lang == "EN";
    if !crate::install::hay_tty() {
        return Err(if english {
            "no terminal: use --model or --all-models and exactly one of --test or --test-group"
                .into()
        } else {
            "sin terminal: usa --model o --all-models y exactamente uno de --test o --test-group"
                .into()
        });
    }
    if models.is_empty() {
        return Err(if english {
            "there are no available models to test".into()
        } else {
            "no hay modelos disponibles para probar".into()
        });
    }

    let palette = Palette::color();
    let mut model_options = Vec::new();
    let mut engine_legend = std::collections::BTreeMap::new();
    for (_, engine_name, color) in models {
        if let Some(color) = color {
            engine_legend.insert(engine_name, *color);
        }
    }
    let legend = engine_legend
        .iter()
        .map(|(name, color)| palette.paint(*color, name))
        .collect::<Vec<_>>()
        .join(" - ");
    let mut lines = vec![
        if english { "Engines:" } else { "Motores:" }.to_string(),
        format!("  {legend}"),
        String::new(),
    ];
    lines.push(if english { "Models:" } else { "Modelos:" }.into());
    for (index, (name, _, color)) in models.iter().enumerate() {
        let name = color.map_or_else(|| name.clone(), |color| palette.paint(color, name));
        lines.push(format!("  {}. {name}", index + 1));
        model_options.push(models[index].0.clone());
    }
    let all_word = if english { "all" } else { "todos" };
    lines.push(format!(
        "  {}. {all_word} — {}",
        model_options.len() + 1,
        if english {
            "test all available models"
        } else {
            "probar todos los modelos disponibles"
        }
    ));
    model_options.push(all_word.into());
    ropero_core::out_line(&palette.paint_hex(
        HELP_HEADING,
        if english {
            "-------- Ropero - Tests --------"
        } else {
            "-------- Ropero - Pruebas --------"
        },
    ));
    let selected = choose_one(
        if english {
            "Choose a model to test:"
        } else {
            "Elige el modelo que quieres probar:"
        },
        &lines,
        &model_options,
        english,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| {
        if english {
            "no model was selected".to_string()
        } else {
            "no se eligió ningún modelo".to_string()
        }
    })?;
    let all_models = selected == all_word;
    let model = (!all_models).then_some(selected);

    let battery = parse_indice(INDICE).map_err(|error| error.to_string())?;
    let selection = choose_selection(&battery, english)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let path = crate::install::pregunta(&format!(
        "{} [{}] {}: ",
        if english {
            "Results directory"
        } else {
            "Directorio de resultados"
        },
        cwd.display(),
        if english {
            "(empty uses current directory)"
        } else {
            "(vacío usa el actual)"
        }
    ))
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| cwd.display().to_string());
    Ok(TestWizardResult {
        model,
        all_models,
        selection,
        path,
    })
}

fn choose_selection(battery: &Bateria, english: bool) -> Result<Seleccion, String> {
    let groups = battery
        .grupos
        .iter()
        .map(|group| group.letra.to_string())
        .collect::<Vec<_>>();
    let mut lines = vec![
        if english {
            "Test groups:"
        } else {
            "Grupos de tests:"
        }
        .to_string(),
    ];
    for (index, group) in battery.grupos.iter().enumerate() {
        lines.push(format!(
            "  {}. {} — {}",
            index + 1,
            group.letra,
            group.nombre
        ));
    }
    let all_word = if english { "all" } else { "todos" };
    lines.push(format!(
        "  {}. {all_word} — {}",
        groups.len() + 1,
        if english {
            "entire test suite"
        } else {
            "toda la batería"
        }
    ));
    let mut options = groups;
    options.push(all_word.into());
    let selected = choose_one(
        if english {
            "Choose a group or all tests:"
        } else {
            "Elige un grupo o todos los tests:"
        },
        &lines,
        &options,
        english,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| {
        if english {
            "no test group was selected".to_string()
        } else {
            "no se eligió ningún grupo".to_string()
        }
    })?;
    if selected == all_word {
        return Ok(Seleccion::Todos);
    }

    let group = selected.chars().next().ok_or("grupo vacío")?;
    let tests = battery.tests_del_grupo(group);
    let mut lines = vec![if english {
        format!("Tests in group {group}:")
    } else {
        format!("Tests del grupo {group}:")
    }];
    let mut options = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        lines.push(format!("  {}. {} — {}", index + 1, test.id, test.titulo));
        options.push(test.id.clone());
    }
    lines.push(format!(
        "  {}. {all_word} — {}",
        options.len() + 1,
        if english {
            "entire group"
        } else {
            "grupo completo"
        }
    ));
    options.push(all_word.into());
    let selected = choose_one(
        if english {
            "Choose one test or all tests in the group:"
        } else {
            "Elige un test o todos los tests del grupo:"
        },
        &lines,
        &options,
        english,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| {
        if english {
            "no test was selected".to_string()
        } else {
            "no se eligió ningún test".to_string()
        }
    })?;
    if selected == all_word {
        return Ok(Seleccion::Grupo(group));
    }
    Ok(Seleccion::Test(selected))
}

fn choose_one(
    prompt: &str,
    lines: &[String],
    options: &[String],
    english: bool,
) -> Result<Vec<String>, String> {
    for line in lines {
        ropero_core::out_line(line);
    }
    let answer = crate::install::pregunta(&format!("{prompt} ")).ok_or_else(|| {
        if english {
            "no answer was provided; cancelling".to_string()
        } else {
            "no hay respuesta: se cancela".to_string()
        }
    })?;
    ropero_core::install::resuelve_seleccion(&answer, options).map_err(|error| error.to_string())
}
