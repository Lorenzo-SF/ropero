//! Ejecución de la eliminación de artefactos de un modelo.

use std::path::{Path, PathBuf};

use ropero_core::{DirectoryArtifact, EngineManifest, ModelSpec, RemovalPlan};

/// Elimina los ficheros concretos del plan. Nunca borra directorios.
pub fn elimina_ficheros(files: &[PathBuf], root: &Path) -> i32 {
    let mut ok = true;
    let canonical_root = match std::fs::canonicalize(root) {
        Ok(root) => root,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            for path in files {
                ropero_core::out_line(&format!("omitido (no existe): {}", path.display()));
            }
            return 0;
        }
        Err(e) => {
            ropero_core::err_line(&format!(
                "error: no se pudo inspeccionar la raíz '{}': {e}",
                root.display()
            ));
            return 1;
        }
    };
    for path in files {
        let target = match ruta_fichero_segura(root, &canonical_root, path) {
            Ok(target) => target,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ropero_core::out_line(&format!("omitido (no existe): {}", path.display()));
                continue;
            }
            Err(e) => {
                ropero_core::err_line(&format!(
                    "error: no se eliminó '{}' por una ruta no segura: {e}",
                    path.display()
                ));
                ok = false;
                continue;
            }
        };
        match std::fs::symlink_metadata(&target) {
            Ok(meta) => {
                if meta.file_type().is_dir() {
                    ropero_core::err_line(&format!(
                        "aviso: '{}' es un directorio; no se elimina por seguridad",
                        target.display()
                    ));
                    continue;
                }
                if let Err(e) = std::fs::remove_file(&target) {
                    ropero_core::err_line(&format!(
                        "error: no se pudo eliminar '{}': {e}",
                        target.display()
                    ));
                    ok = false;
                } else {
                    ropero_core::out_line(&format!("eliminado: {}", target.display()));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ropero_core::out_line(&format!("omitido (no existe): {}", path.display()));
            }
            Err(e) => {
                ropero_core::err_line(&format!(
                    "error: no se pudo inspeccionar '{}': {e}",
                    path.display()
                ));
                ok = false;
            }
        }
    }
    if ok { 0 } else { 1 }
}

fn ruta_fichero_segura(
    declared_root: &Path,
    canonical_root: &Path,
    path: &Path,
) -> std::io::Result<PathBuf> {
    use std::path::Component;

    let relative = path.strip_prefix(declared_root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "el fichero sale del directorio del modelo",
        )
    })?;
    let parts: Vec<_> = relative.components().collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "la ruta no es relativa y segura",
        ));
    }
    let mut parent = canonical_root.to_path_buf();
    for part in &parts[..parts.len() - 1] {
        parent.push(part.as_os_str());
        let metadata = std::fs::symlink_metadata(&parent)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "la ruta contiene un enlace o un componente que no es directorio",
            ));
        }
    }
    Ok(parent.join(parts[parts.len() - 1].as_os_str()))
}

/// Elimina directorios que el TOML declara como unidades de checkpoint.
///
/// Se valida la ruta contra la raíz y se rechazan enlaces simbólicos en los
/// componentes intermedios para que una sustitución del árbol no redirija el
/// borrado fuera del directorio de artefactos.
pub fn elimina_directorios(artifacts: &[DirectoryArtifact]) -> i32 {
    let mut ok = true;
    for artifact in artifacts {
        match elimina_directorio_declarado(artifact) {
            Ok(()) => {
                ropero_core::out_line(&format!(
                    "checkpoint eliminado: {}",
                    artifact.path.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ropero_core::out_line(&format!("omitido (no existe): {}", artifact.path.display()));
            }
            Err(e) => {
                ropero_core::err_line(&format!(
                    "error: no se pudo eliminar el checkpoint '{}': {e}",
                    artifact.path.display()
                ));
                ok = false;
            }
        }
    }
    if ok { 0 } else { 1 }
}

fn elimina_directorio_declarado(artifact: &DirectoryArtifact) -> std::io::Result<()> {
    use std::path::Component;

    let relative = artifact.path.strip_prefix(&artifact.root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "el checkpoint sale de su raíz declarada",
        )
    })?;
    let parts: Vec<_> = relative.components().collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "la ruta del checkpoint no es relativa y segura",
        ));
    }
    let root = match std::fs::canonicalize(&artifact.root) {
        Ok(root) => root,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(e),
        Err(e) => return Err(e),
    };
    let mut parent = root;
    for part in &parts[..parts.len() - 1] {
        parent.push(part.as_os_str());
        let metadata = std::fs::symlink_metadata(&parent)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "la ruta del checkpoint contiene un enlace o un componente que no es directorio",
            ));
        }
    }
    let target = parent.join(parts[parts.len() - 1].as_os_str());
    let metadata = std::fs::symlink_metadata(&target)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        std::fs::remove_file(target)
    } else {
        std::fs::remove_dir_all(target)
    }
}

/// Retira el directorio concreto del modelo sólo si quedó vacío.
pub fn limpiar_dir_vacio(dir: &Path) {
    if let Ok(mut entries) = std::fs::read_dir(dir)
        && entries.next().is_none()
        && std::fs::remove_dir(dir).is_ok()
    {
        ropero_core::out_line(&format!("directorio vacío eliminado: {}", dir.display()));
    }
}

/// Ejecuta `flm remove` para cada tag instalado que declara el TOML.
pub fn elimina_flm(
    manifest: &EngineManifest,
    tags: &[String],
    env: &ropero_core::Environment,
) -> i32 {
    if !crate::flm::delegated(manifest) {
        ropero_core::err_line(&format!(
            "error: el motor '{}' no declara delegación de eliminación FLM",
            manifest.id.as_str()
        ));
        return 1;
    }
    let mut ok = true;
    for tag in tags {
        ropero_core::out_line(&format!("ejecutando: flm remove {tag}"));
        match crate::flm::remove(manifest, tag, env) {
            Ok(res) => {
                if !res.stdout.trim().is_empty() {
                    ropero_core::out(res.stdout.trim_end(), true);
                }
                if !res.stderr.trim().is_empty() {
                    ropero_core::err_line(res.stderr.trim_end());
                }
                if res.ok() {
                    ropero_core::out_line(&format!("tag FLM '{tag}' eliminado"));
                } else {
                    ropero_core::err_line(&format!(
                        "error: 'flm remove {tag}' terminó con código {}",
                        res.status
                    ));
                    ok = false;
                }
            }
            Err(e) => {
                ropero_core::err_line(&format!("error al eliminar el tag FLM '{tag}': {e}"));
                ok = false;
            }
        }
    }
    if ok { 0 } else { 1 }
}

/// Ejecuta el plan ya confirmado.
pub fn ejecutar(
    engines: &[EngineManifest],
    spec: &ModelSpec,
    plan: &RemovalPlan,
    env: &ropero_core::Environment,
) -> i32 {
    let mut ok = true;
    if let Some(flm) = &plan.flm {
        let Some(manifest) = engines.iter().find(|m| m.id == spec.model.engine) else {
            ropero_core::err_line(&format!(
                "error: no se encontró el manifiesto del motor '{}' para eliminar el tag FLM",
                spec.model.engine.as_str()
            ));
            return 1;
        };
        if elimina_flm(manifest, &flm.all_tags, env) != 0 {
            ok = false;
        }
    }
    if !plan.files.is_empty() {
        let Some(root) = plan.gguf_model_dir.as_deref() else {
            ropero_core::err_line("error: falta la raíz del directorio de artefactos del modelo");
            return 1;
        };
        if elimina_ficheros(&plan.files, root) != 0 {
            ok = false;
        }
    }
    if !plan.directories.is_empty() && elimina_directorios(&plan.directories) != 0 {
        ok = false;
    }
    if ok {
        if let Some(dir) = &plan.gguf_model_dir {
            limpiar_dir_vacio(dir);
        }
        ropero_core::out_line("eliminación completada");
        0
    } else {
        ropero_core::err_line("la eliminación terminó con errores");
        1
    }
}

#[cfg(test)]
mod tests {
    use super::elimina_directorios;
    use ropero_core::DirectoryArtifact;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ropero-remove-safetensors-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn elimina_solo_el_checkpoint_declarado_bajo_la_raiz() {
        let root = tmp("checkpoint");
        let target = root.join("familia/modelo");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("weights.safetensors"), b"weights").unwrap();
        let artifact = DirectoryArtifact {
            root: root.clone(),
            path: target.clone(),
        };

        assert_eq!(elimina_directorios(&[artifact]), 0);
        assert!(!target.exists());
        assert!(root.is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn no_sigue_enlaces_simbolicos_en_componentes_intermedios() {
        use std::os::unix::fs::symlink;

        let root = tmp("symlink");
        let fuera = root.with_extension("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&fuera).unwrap();
        std::fs::write(fuera.join("keep.safetensors"), b"keep").unwrap();
        symlink(&fuera, root.join("familia")).unwrap();
        let artifact = DirectoryArtifact {
            root: root.clone(),
            path: root.join("familia/modelo"),
        };

        assert_eq!(elimina_directorios(&[artifact]), 1);
        assert!(fuera.join("keep.safetensors").exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(fuera);
    }
}
