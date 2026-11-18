//! Plan de eliminación de un modelo.
//!
//! ## Qué calcula
//!
//! Una eliminación sólo puede borrar artefactos que se pueden **derivar** del
//! TOML y de las rutas del modelo: ficheros declarados/resueltos y operaciones
//! delegadas (FLM). Nunca directorios compartidos ni datos no declarados.
//!
//! La decisión es **pura** (sin tocar disco). La ejecución y la confirmación
//! interactiva viven en la CLI/runtime.

use std::path::PathBuf;

use crate::{Catalog, Environment, ModelFormat, ModelSpec, es_alias, real_name, resolve_paths};

/// Lo que hay que hacer para eliminar un modelo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemovalPlan {
    /// Ficheros concretos atribuibles al modelo que se pueden borrar.
    pub files: Vec<PathBuf>,
    /// Directorios de artefacto declarados como checkpoint (p. ej.
    /// safetensors), que se borrarán solo tras confirmación.
    pub directories: Vec<DirectoryArtifact>,
    /// Directorio del modelo bajo la raíz GGUF efectiva del engine del que se pueden retirar entradas
    /// vacías (nunca se borra recursivamente).
    pub gguf_model_dir: Option<PathBuf>,
    /// Operación FLM delegada, si aplica.
    pub flm: Option<FlmRemoval>,
    /// Motivos por los que la eliminación automática no es segura.
    pub blocked: Vec<String>,
    /// Notas informativas para mostrar al usuario.
    pub notes: Vec<String>,
}

/// Directorio concreto declarado en `[model].path`, acotado a su raíz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryArtifact {
    /// Raíz permitida del artefacto.
    pub root: PathBuf,
    /// Ruta exacta del checkpoint declarado.
    pub path: PathBuf,
}

/// Eliminación delegada a FLM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlmRemoval {
    /// Tag exacto que identifica el modelo principal.
    pub primary_tag: String,
    /// Todos los tags instalados según el TOML, incluidos auxiliares como
    /// `--decision`; se eliminan individualmente con `flm remove <tag>`.
    pub all_tags: Vec<String>,
}

impl RemovalPlan {
    /// Si no hay nada que borrar ni delegar.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
            && self.directories.is_empty()
            && self.flm.is_none()
            && self.gguf_model_dir.is_none()
    }

    /// Si la eliminación automática está bloqueada.
    pub fn is_blocked(&self) -> bool {
        !self.blocked.is_empty()
    }
}

/// Calcula el plan de eliminación de un modelo.
///
/// Sólo incluye artefactos derivables del TOML/model paths. No consulta el
/// disco: la existencia real y el tipo (fichero/directorio) los comprueba la
/// capa de ejecución.
pub fn removal_plan(spec: &ModelSpec, env: &Environment) -> RemovalPlan {
    let mut plan = RemovalPlan::default();

    if crate::flm::is_flm_model(spec) {
        plan_flm(spec, &mut plan);
        return plan;
    }

    if spec.model.format == ModelFormat::Ninfer && spec.model.engine == crate::EngineId::Ignis {
        plan.blocked.push(
            "Ignis no ofrece un comando local para eliminar un modelo y sus companions; no se borran artefactos automáticamente".to_string(),
        );
        return plan;
    }

    let paths = resolve_paths(spec, env);
    let model_name = real_name(spec);
    let gguf_root = env.engine_artifact_path(spec.model.engine, "GGUF_DIR", "models/gguf");
    let gguf_dir = gguf_root.join(&model_name);

    // Ficheros de [files] resueltos (GGUF/MTP/mmproj/chat_template declarados o
    // inferidos por resolve_paths como rutas concretas dentro del modelo).
    push_file(&mut plan, &gguf_dir, "gguf", paths.gguf);
    push_file(&mut plan, &gguf_dir, "mtp", paths.mtp);
    push_file(&mut plan, &gguf_dir, "mmproj", paths.mmproj);
    push_file(&mut plan, &gguf_dir, "chat_template", paths.chat_template);

    // Auxiliares de descarga con uso de runtime: mismos nombres/rutas que el
    // plan de descarga (derivables del TOML).
    if let Some(download) = spec.download.as_ref() {
        for (kind, aux) in &download.aux {
            if aux.file.is_empty() {
                continue;
            }
            if uso_de_aux_run(kind, &aux.use_) {
                let name = if aux.name.is_empty() {
                    &aux.file
                } else {
                    &aux.name
                };
                if !name.is_empty() {
                    match safe_join(&gguf_dir, name) {
                        Some(p) => plan.files.push(p),
                        None => plan.blocked.push(format!(
                            "el auxiliar '{}' declara una ruta no segura y no se puede eliminar automáticamente",
                            kind
                        )),
                    }
                }
            }
        }
    }

    plan.files = dedupe_paths(plan.files);
    if !plan.files.is_empty() {
        plan.gguf_model_dir = Some(gguf_dir);
    }

    // Safetensors: [model].path declara el checkpoint como una unidad. Se
    // elimina solo esa ruta, nunca el directorio raíz compartido.
    if spec.model.format == ModelFormat::Safetensors && !spec.model.path.is_empty() {
        let root =
            env.engine_artifact_path(spec.model.engine, "SAFETENSORS_DIR", "models/safetensors");
        match safe_join(&root, &spec.model.path) {
            Some(path) => plan.directories.push(DirectoryArtifact { root, path }),
            None => plan.blocked.push(format!(
                "el checkpoint safetensors '{}' sale de SAFETENSORS_DIR y no se puede eliminar automáticamente",
                spec.model.path
            )),
        }
    }

    if spec.model.format == ModelFormat::Basalt && !spec.model.path.is_empty() {
        let root = env.engine_artifact_path(spec.model.engine, "ARTIFACTS_DIR", "models/basalt");
        match safe_join(&root, &spec.model.path) {
            Some(path) => plan.files.push(path),
            None => plan.blocked.push(format!(
                "el pack Basalt '{}' sale de ARTIFACTS_DIR y no se puede eliminar automáticamente",
                spec.model.path
            )),
        }
    }

    plan
}

/// Calcula el plan y conserva los artefactos cuyo mismo destino pertenece a
/// otro TOML del catálogo. Los aliases materializados no cuentan como otro
/// propietario; la comparación se hace por rutas/tag instalados, no por repo
/// de origen.
pub fn removal_plan_in_catalog(
    spec: &ModelSpec,
    catalog: &Catalog,
    env: &Environment,
) -> RemovalPlan {
    let mut plan = removal_plan(spec, env);
    let nombre = real_name(spec);
    let mut seen_names = std::collections::BTreeSet::new();
    let otros: Vec<&ModelSpec> = catalog
        .models
        .iter()
        .chain(catalog.fired.iter())
        .filter(|other| !es_alias(other))
        .filter(|other| other.model.name != nombre)
        .filter(|other| seen_names.insert(real_name(other)))
        .collect();

    for other in otros {
        let referencia = real_name(other);
        let other_plan = removal_plan(other, env);

        for shared in plan
            .files
            .iter()
            .filter(|path| other_plan.files.contains(path))
            .cloned()
            .collect::<Vec<_>>()
        {
            plan.files.retain(|path| path != &shared);
            push_shared_note(&mut plan, &shared, &referencia);
        }
        for shared in plan
            .directories
            .iter()
            .filter(|artifact| {
                other_plan.directories.iter().any(|other| {
                    artifact.path.starts_with(&other.path) || other.path.starts_with(&artifact.path)
                })
            })
            .map(|artifact| artifact.path.clone())
            .collect::<Vec<_>>()
        {
            plan.directories.retain(|artifact| artifact.path != shared);
            push_shared_note(&mut plan, &shared, &referencia);
        }
        if let (Some(flm), Some(other_flm)) = (&plan.flm, &other_plan.flm) {
            let compartidos: Vec<String> = flm
                .all_tags
                .iter()
                .filter(|tag| other_flm.all_tags.contains(tag))
                .cloned()
                .collect();
            if !compartidos.is_empty() {
                if let Some(flm) = &mut plan.flm {
                    flm.all_tags
                        .retain(|candidate| !compartidos.contains(candidate));
                }
                for tag in &compartidos {
                    push_shared_note(&mut plan, PathBuf::from(tag).as_path(), &referencia);
                }
                if plan.flm.as_ref().is_some_and(|flm| flm.all_tags.is_empty()) {
                    plan.flm = None;
                }
            }
        }
    }

    if plan.files.is_empty() {
        plan.gguf_model_dir = None;
    }
    plan
}

fn push_shared_note(plan: &mut RemovalPlan, path: &std::path::Path, other: &str) {
    let note = format!(
        "se conserva '{}' porque su ruta se solapa con la de otro artefacto del modelo registrado '{}'",
        path.display(),
        other
    );
    if !plan.notes.contains(&note) {
        plan.notes.push(note);
    }
}

fn plan_flm(spec: &ModelSpec, plan: &mut RemovalPlan) {
    let all_tags = crate::flm::tags(spec);
    let Some(primary) = crate::flm::primary_tag(spec) else {
        plan.blocked.push(
            "el modelo FLM no declara tag ([model].path o --decision); no hay nada que eliminar"
                .to_string(),
        );
        return;
    };
    plan.flm = Some(FlmRemoval {
        primary_tag: primary,
        all_tags,
    });
}

fn push_file(plan: &mut RemovalPlan, root: &std::path::Path, kind: &str, path: Option<String>) {
    if let Some(p) = path
        && !p.is_empty()
    {
        let path = PathBuf::from(p);
        let safe = path.strip_prefix(root).is_ok_and(|relative| {
            relative.components().all(|component| {
                matches!(
                    component,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        });
        if safe {
            plan.files.push(path);
        } else {
            plan.blocked.push(format!(
                "el fichero declarado '{kind}' sale del directorio del modelo y no se puede eliminar automáticamente"
            ));
        }
    }
}

fn dedupe_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths.dedup();
    paths
}

/// Si un auxiliar es de uso runtime (`run`), según tipo y declaración.
fn uso_de_aux_run(kind: &str, declarado: &str) -> bool {
    match declarado {
        "run" => true,
        "build" => false,
        _ => !matches!(kind, "imatrix" | "tokenizer"),
    }
}

/// Une `name` a `base` sólo si la ruta queda léxicamente dentro de `base`.
///
/// Rechaza rutas absolutas y componentes `..` para no salir del directorio del
/// modelo ni tocar rutas compartidas/no declaradas.
fn safe_join(base: &std::path::Path, name: &str) -> Option<PathBuf> {
    use std::path::{Component, Path};
    let path = Path::new(name);
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(p) => normalized.push(p),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let joined = base.join(&normalized);
    if joined.starts_with(base) {
        Some(joined)
    } else {
        None
    }
}
