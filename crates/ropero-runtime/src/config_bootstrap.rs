//! Arranque y bootstrap de la configuración (E/S).
//!
//! El primer uso crea el directorio de configuración y `config.toml` por
//! defecto sin sobrescribir nada. La creación del archivo es idempotente y
//! usa apertura exclusiva (`create_new`) para evitar pisadas.

use std::fs;
use std::io::Write;

use ropero_core::Environment;
use ropero_core::config::{
    self, Config, EstadoConfig, archivo_config, directorio_config, ejemplo_config,
};

pub(crate) const TESTS: &[(&str, &str)] = &[
    ("indice.md", include_str!("../../../docs/tests/indice.md")),
    (
        "test-adherencia-persona-restricciones-duras.md",
        include_str!("../../../docs/tests/test-adherencia-persona-restricciones-duras.md"),
    ),
    (
        "test-autocritica.md",
        include_str!("../../../docs/tests/test-autocritica.md"),
    ),
    (
        "test-calibracion.md",
        include_str!("../../../docs/tests/test-calibracion.md"),
    ),
    (
        "test-comprension-ambiguedad.md",
        include_str!("../../../docs/tests/test-comprension-ambiguedad.md"),
    ),
    (
        "test-concision-reflejo.md",
        include_str!("../../../docs/tests/test-concision-reflejo.md"),
    ),
    (
        "test-creatividad-divergente-restricciones.md",
        include_str!("../../../docs/tests/test-creatividad-divergente-restricciones.md"),
    ),
    (
        "test-falsa-creencia-segunda-orden.md",
        include_str!("../../../docs/tests/test-falsa-creencia-segunda-orden.md"),
    ),
    (
        "test-inteligencia-fluida.md",
        include_str!("../../../docs/tests/test-inteligencia-fluida.md"),
    ),
    (
        "test-logico-deductivo.md",
        include_str!("../../../docs/tests/test-logico-deductivo.md"),
    ),
    (
        "test-memoria-trabajo.md",
        include_str!("../../../docs/tests/test-memoria-trabajo.md"),
    ),
    (
        "test-planificacion-dependencias-restricciones.md",
        include_str!("../../../docs/tests/test-planificacion-dependencias-restricciones.md"),
    ),
    (
        "test-razonamiento-bajo-informacion-incompleta.md",
        include_str!("../../../docs/tests/test-razonamiento-bajo-informacion-incompleta.md"),
    ),
    (
        "test-razonamiento-casual-contractual.md",
        include_str!("../../../docs/tests/test-razonamiento-casual-contractual.md"),
    ),
    (
        "test-razonamiento-matematico-competicion.md",
        include_str!("../../../docs/tests/test-razonamiento-matematico-competicion.md"),
    ),
    (
        "test-razonamiento-trampa.md",
        include_str!("../../../docs/tests/test-razonamiento-trampa.md"),
    ),
    (
        "test-resistencia-prompt-injection.md",
        include_str!("../../../docs/tests/test-resistencia-prompt-injection.md"),
    ),
    (
        "test-herramienta-calculadora.md",
        include_str!("../../../docs/tests/test-herramienta-calculadora.md"),
    ),
    (
        "test-herramienta-consulta.md",
        include_str!("../../../docs/tests/test-herramienta-consulta.md"),
    ),
    (
        "test-herramienta-dato-hostil.md",
        include_str!("../../../docs/tests/test-herramienta-dato-hostil.md"),
    ),
    (
        "test-herramienta-sin-resultados.md",
        include_str!("../../../docs/tests/test-herramienta-sin-resultados.md"),
    ),
];

/// Carga el estado y la configuración resuelta, creando el directorio y el
/// archivo por defecto si no existen.
///
/// El archivo por defecto se genera con **valores puros** (sin overrides de
/// variables de entorno de rutas), para que los overrides solo afecten a la
/// configuración efectiva en memoria. Un error de lectura o parseo del archivo
/// existente se reporta en `estado.error` y se devuelve como error: la
/// configuración ilegible nunca debe quedar oculta.
pub fn prepara_config(env: &Environment) -> Result<(EstadoConfig, Config, Environment), String> {
    let (estado, config_resuelta, env_out) = carga_config(env)?;
    if let Some(error) = &estado.error {
        return Err(format!(
            "configuración ilegible en {}: {}. {}",
            estado.archivo.display(),
            error,
            error.que_hacer()
        ));
    }
    Ok((estado, config_resuelta, env_out))
}

/// Carga y bootstrap de configuración tolerante a errores de parseo/lectura.
///
/// Siempre que el bootstrap de E/S sea posible, devuelve el estado (con
/// `error` si el archivo no vale) y una configuración efectiva de mejor
/// esfuerzo. Sirve a comandos como `config` y `doctor`, que deben mostrar el
/// problema sin ocultarlo.
pub fn carga_config(env: &Environment) -> Result<(EstadoConfig, Config, Environment), String> {
    let directorio = directorio_config(env);
    let archivo = archivo_config(env);
    fs::create_dir_all(&directorio)
        .map_err(|e| format!("no se pudo crear {}: {e}", directorio.display()))?;

    let mut estado = EstadoConfig {
        directorio: directorio.clone(),
        archivo: archivo.clone(),
        existe: archivo.is_file(),
        error: None,
    };

    if !estado.existe {
        // La plantilla persiste valores por defecto puros; los overrides de
        // entorno solo se aplican en memoria a la configuración efectiva.
        let defectos = config::config_defectos(&env.vars, &env.home, &env.cwd);
        let contenido = ejemplo_config(&defectos);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&archivo)
        {
            Ok(mut f) => {
                f.write_all(contenido.as_bytes())
                    .map_err(|e| format!("no se pudo escribir {}: {e}", archivo.display()))?;
                estado.existe = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                estado.existe = archivo.is_file();
            }
            Err(e) => return Err(format!("no se pudo crear {}: {e}", archivo.display())),
        }
    }

    let texto = if estado.existe {
        match fs::read_to_string(&archivo) {
            Ok(t) => Some(t),
            Err(e) => {
                estado.error = Some(config::ErrorConfig::de_archivo(&e.to_string()));
                None
            }
        }
    } else {
        None
    };

    let config_resuelta = match config::resuelve_config(env, texto.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            estado.error = Some(e.clone());
            // Configuración efectiva de mejor esfuerzo (env sobre defectos) para
            // poder informar del problema; no debe usarse para ocultarlo.
            config::config_inicial(&env.vars, &env.home, &env.cwd)
        }
    };

    // Estructura de usuario estable: el catálogo de ejemplos versionado en
    // el repo no se copia. El usuario parte de una plantilla individual.
    let user_models = directorio.join("models");
    fs::create_dir_all(directorio.join("logs")).map_err(|e| {
        format!(
            "no se pudo crear {}: {e}",
            directorio.join("logs").display()
        )
    })?;
    fs::create_dir_all(&user_models)
        .map_err(|e| format!("no se pudo crear {}: {e}", user_models.display()))?;
    let template = user_models.join("template.toml");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&template)
    {
        Ok(mut file) => file
            .write_all(ropero_core::config::PLANTILLA_MODELO.as_bytes())
            .map_err(|e| format!("no se pudo escribir {}: {e}", template.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(format!("no se pudo crear {}: {e}", template.display())),
    }

    // Asegura que los directorios de la configuración efectiva existan.
    let mut dirs = vec![
        config_resuelta.models_dir.valor.clone(),
        config::directorio_engines(&config_resuelta.models_dir.valor),
        config_resuelta.logs_dir.valor.clone(),
    ];
    // Un override global de catálogo mantiene el layout antiguo y desactiva
    // las rutas TOML por engine, también durante el bootstrap.
    let external_catalog_override =
        env.get("ROPERO_MODELS_DIR").is_some() || env.get("ROPERO_ROOT").is_some();
    if !external_catalog_override {
        let models_engine_dirs: Vec<_> = config_resuelta
            .engines
            .values()
            .filter_map(|engine| {
                engine
                    .models_toml_dir
                    .as_ref()
                    .map(|ruta| ruta.valor.clone())
            })
            .collect();
        dirs.extend(models_engine_dirs);
    }
    for dir in dirs {
        fs::create_dir_all(&dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    }

    // La batería es un recurso de la instalación de Ropero, no un requisito
    // de que el usuario ejecute desde el checkout. Copiamos sólo los ficheros
    // que faltan para preservar cualquier edición local.
    let catalog_override =
        env.get("ROPERO_MODELS_DIR").is_some() || env.get("ROPERO_ROOT").is_some();
    if !catalog_override || env.get("XDG_CONFIG_HOME").is_some() {
        prepara_docs_tests(&directorio)?;
    }

    let env_out = env.clone().with_config(config_resuelta.clone());
    Ok((estado, config_resuelta, env_out))
}

/// Instala una copia de consulta sólo donde falte. El runner lee `TESTS`
/// compilado con Ropero, para que una configuración antigua no oculte
/// pruebas nuevas y las modificaciones locales no alteren la batería interna.
fn prepara_docs_tests(config_dir: &std::path::Path) -> Result<(), String> {
    let dir = config_dir.join("docs/tests");
    fs::create_dir_all(&dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    for (name, contents) in TESTS {
        let path = dir.join(name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => file
                .write_all(contents.as_bytes())
                .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("no se pudo crear {}: {e}", path.display())),
        }
    }
    Ok(())
}

/// Resuelve la raíz del catálogo según overrides o configuración externa.
pub fn resolve_catalog_root(
    env: &Environment,
    config: &Config,
) -> Result<std::path::PathBuf, String> {
    let tiene_override = env.get("ROPERO_MODELS_DIR").is_some() || env.get("ROPERO_ROOT").is_some();
    if tiene_override {
        return crate::find_root(env, &crate::exe_ancestors());
    }
    Ok(config::raiz_del_catalogo(&config.models_dir.valor))
}

#[cfg(test)]
mod tests {
    use super::TESTS;

    #[test]
    fn los_prompts_compilados_contienen_todos_los_archivos_del_indice() {
        let indice = TESTS
            .iter()
            .find(|(nombre, _)| *nombre == "indice.md")
            .map(|(_, texto)| *texto)
            .expect("el índice está compilado");
        let bateria = ropero_bateria::indice::parse_indice(indice).expect("índice válido");
        for test in bateria.tests {
            assert!(
                TESTS.iter().any(|(nombre, _)| *nombre == test.fichero),
                "falta compilar el prompt {}",
                test.fichero
            );
        }
    }
}

pub fn guardar_idioma(path: &std::path::Path, lang: &str) -> Result<(), String> {
    let permissions = std::fs::metadata(path).ok().map(|meta| meta.permissions());
    let mut document = match std::fs::read_to_string(path) {
        Ok(text) => text.parse::<toml_edit::DocumentMut>().map_err(|error| {
            format!("el config.toml no es TOML válido ({error}); no se ha alterado")
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => toml_edit::DocumentMut::new(),
        Err(error) => return Err(format!("no se pudo leer {}: {error}", path.display())),
    };
    document
        .as_table_mut()
        .insert("lang", toml_edit::value(lang));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    let write = std::fs::write(&temp, document.to_string()).and_then(|()| {
        if let Some(permissions) = permissions {
            std::fs::set_permissions(&temp, permissions)?;
        }
        std::fs::rename(&temp, path)
    });
    if let Err(error) = write {
        let _ = std::fs::remove_file(&temp);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod language_tests {
    use super::guardar_idioma;

    #[test]
    fn cambiar_idioma_preserva_comentarios_y_toml_valido() {
        let path =
            std::env::temp_dir().join(format!("ropero-config-lang-{}.toml", std::process::id()));
        std::fs::write(&path, "# conservar\n[paths]\nmodels_dir = \"models\"\n").unwrap();
        guardar_idioma(&path, "ES").unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let parsed = saved.parse::<toml_edit::DocumentMut>().unwrap();
        assert_eq!(parsed["lang"].as_str(), Some("ES"));
        assert_eq!(parsed["paths"]["models_dir"].as_str(), Some("models"));
        assert!(saved.contains("# conservar"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn config_toml_invalido_no_se_sobrescribe_al_cambiar_idioma() {
        let path =
            std::env::temp_dir().join(format!("ropero-config-bad-{}.toml", std::process::id()));
        let original = "[broken\n";
        std::fs::write(&path, original).unwrap();
        assert!(guardar_idioma(&path, "EN").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_file(path);
    }
}
