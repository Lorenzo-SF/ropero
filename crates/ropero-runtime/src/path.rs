//! Expansión segura de rutas introducidas por el usuario.

use std::path::{Path, PathBuf};

/// Expande `~`, `$HOME`, `${HOME}` y variables de entorno sin ejecutar una
/// shell. Las variables desconocidas se reportan para evitar crear rutas
/// literales que no sean las que el usuario pretendía.
pub fn expand_result_path(
    input: &str,
    home: &Path,
    variable_value: impl Fn(&str) -> Option<String>,
) -> Result<PathBuf, String> {
    let mut expanded = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '$' {
            expanded.push(character);
            continue;
        }

        let variable = if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == '}' {
                    closed = true;
                    break;
                }
                name.push(next);
            }
            if !closed || name.is_empty() {
                return Err("variable incompleta en la ruta de resultados".into());
            }
            name
        } else {
            let mut name = String::new();
            while chars
                .peek()
                .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
            {
                name.push(chars.next().unwrap_or_default());
            }
            if name.is_empty() {
                expanded.push('$');
                continue;
            }
            name
        };

        if variable == "HOME" {
            expanded.push_str(&home.to_string_lossy());
        } else {
            let value = variable_value(&variable).ok_or_else(|| {
                format!("la variable ${variable} no está definida en la ruta de resultados")
            })?;
            expanded.push_str(&value);
        }
    }

    if expanded == "~" {
        return Ok(home.to_path_buf());
    }
    if let Some(rest) = expanded.strip_prefix("~/") {
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(expanded))
}

#[cfg(test)]
mod tests {
    use super::expand_result_path;
    use std::path::Path;

    #[test]
    fn expande_tilde_y_home_a_rutas_absolutas() {
        let home = Path::new("/home/alice");
        for input in [
            "~/Escritorio/test",
            "$HOME/Escritorio/test",
            "${HOME}/Escritorio/test",
        ] {
            assert_eq!(
                expand_result_path(input, home, |_| None).unwrap(),
                Path::new("/home/alice/Escritorio/test")
            );
        }
    }

    #[test]
    fn expande_variables_y_rechaza_las_que_no_existen() {
        let home = Path::new("/home/alice");
        assert_eq!(
            expand_result_path("$HOME/$DESKTOP/test", home, |name| {
                (name == "DESKTOP").then(|| "Escritorio".into())
            })
            .unwrap(),
            Path::new("/home/alice/Escritorio/test")
        );
        assert!(expand_result_path("$UNKNOWN/test", home, |_| None).is_err());
    }

    #[test]
    fn conserva_rutas_relativas_y_no_evalua_shell() {
        let home = Path::new("/home/alice");
        assert_eq!(
            expand_result_path("./resultados", home, |_| None).unwrap(),
            Path::new("./resultados")
        );
        assert_eq!(
            expand_result_path("$(touch nope)", home, |_| None).unwrap(),
            Path::new("$(touch nope)")
        );
    }
}
