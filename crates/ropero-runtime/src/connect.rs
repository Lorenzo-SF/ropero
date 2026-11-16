//! Integraciones con agentes de código. Cada engine se representa como un
//! provider y cada modelo queda agrupado bajo el provider correspondiente.

use std::path::{Path, PathBuf};

use ropero_core::{EngineManifest, ModelSpec, config::Config};
use serde_json::{Value, json};
use toml_edit::{DocumentMut, Item, Table, value};

const OPENCODE_PACKAGE: &str = "@opencode/ai/providers/openai-compatible";

/// Agentes enumerados por `ropero connect --list` y estado de integración.
pub const AGENTS: &[(&str, &str)] = &[
    ("gemini", "sin proveedor OpenAI-compatible nativo"),
    ("cursor", "requiere configuración interactiva del usuario"),
    (
        "claude",
        "bloqueado: Claude Code usa Anthropic Messages y Ropero sirve OpenAI-compatible",
    ),
    (
        "codebuddy",
        "compatible: ~/.codebuddy/models.json, modelos custom OpenAI",
    ),
    (
        "codex",
        "provider custom Responses API; clave bearer en config local",
    ),
    (
        "opencode",
        "compatible: archivo JSON y modelos por provider",
    ),
    (
        "kilo",
        "compatible: providers OpenAI-compatible en kilo.json",
    ),
    (
        "aider",
        "compatible: model settings + metadata en ~/.aider.model.*",
    ),
    (
        "copilot",
        "BYOK Custom Endpoint de VS Code; requiere habilitar BYOK en Agent Host",
    ),
    (
        "vscode",
        "compatible: chatLanguageModels.json, provider Custom Endpoint",
    ),
    (
        "claw",
        "alias histórico de OpenClaw (Clawdbot/Moltbot); mismo adaptador",
    ),
    (
        "droid",
        "compatible: customModels en ~/.factory/settings.json",
    ),
    ("trae", "configuración interactiva dependiente de versión"),
    (
        "trae-cn",
        "configuración interactiva dependiente de versión",
    ),
    ("antigravity", "sin integración automática verificada"),
    (
        "hermes",
        "compatible: CLI de configuración y credenciales protegidas en .env",
    ),
    ("kiro", "sin integración automática verificada"),
    (
        "pi",
        "compatible: ~/.pi/agent/models.json con providers custom",
    ),
    (
        "devin",
        "servicio remoto; no se modifica configuración local",
    ),
    (
        "mcode",
        "CLI oficial; la baja retira providers completos, requiere seleccionar todos sus modelos",
    ),
    (
        "openclaw",
        "compatible: models.json por agente con providers custom",
    ),
];

/// Añade modelos OpenAI-compatible a los registros oficiales de Aider. Cada
/// entrada conserva su endpoint y clave, lo que permite agruparlos por engine
/// sin cambiar el modelo por defecto del usuario.
pub fn aider(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let settings_path = home.join(".aider.model.settings.yml");
    let metadata_path = home.join(".aider.model.metadata.json");
    let mut settings: serde_yaml::Value = if settings_path.exists() {
        let text = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("no se pudo leer {}: {e}", settings_path.display()))?;
        serde_yaml::from_str(&text).map_err(|e| {
            format!(
                "{} no es YAML válido; no se ha alterado: {e}",
                settings_path.display()
            )
        })?
    } else {
        serde_yaml::Value::Sequence(Vec::new())
    };
    let list = settings.as_sequence_mut().ok_or_else(|| {
        format!(
            "{} debe contener una lista YAML; no se ha alterado",
            settings_path.display()
        )
    })?;
    let mut metadata: Value = if metadata_path.exists() {
        let text = std::fs::read_to_string(&metadata_path)
            .map_err(|e| format!("no se pudo leer {}: {e}", metadata_path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} no es JSON válido; no se ha alterado: {e}",
                metadata_path.display()
            )
        })?
    } else {
        json!({})
    };
    let metadata = metadata.as_object_mut().ok_or_else(|| {
        format!(
            "{} debe ser un objeto JSON; no se ha alterado",
            metadata_path.display()
        )
    })?;

    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let engine = manifest.id.as_str();
        let alias = format!("openai/ropero-{engine}/{}", model.model.name);
        list.retain(|entry| {
            entry.get("name").and_then(serde_yaml::Value::as_str) != Some(alias.as_str())
        });
        if uninstall {
            metadata.remove(&alias);
            continue;
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let api_key = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.as_deref())
            .unwrap_or("not-needed");
        let extra_params = serde_yaml::to_value(json!({
            "api_base": format!("http://127.0.0.1:{port}/v1"),
            "api_key": api_key
        }))
        .map_err(|e| format!("no se pudo serializar settings de Aider: {e}"))?;
        let mut entry = serde_yaml::Mapping::new();
        entry.insert(
            serde_yaml::Value::String("name".into()),
            serde_yaml::Value::String(alias.clone()),
        );
        entry.insert(
            serde_yaml::Value::String("extra_params".into()),
            extra_params,
        );
        list.push(serde_yaml::Value::Mapping(entry));
        let context = u64::from(model.model.context.max(1));
        metadata.insert(
            alias,
            json!({
                "max_tokens": (context * 25 / 100).max(1),
                "max_input_tokens": context * 80 / 100,
                "max_output_tokens": (context * 25 / 100).max(1),
                "litellm_provider": "openai",
                "mode": "chat"
            }),
        );
    }
    let old_settings = std::fs::read(&settings_path).ok();
    let yaml = serde_yaml::to_string(&settings)
        .map_err(|e| format!("no se pudo serializar settings de Aider: {e}"))?;
    write_private_atomic(&settings_path, yaml.as_bytes())?;
    if let Err(error) = write_json_atomic(&metadata_path, &Value::Object(metadata.clone())) {
        if let Some(bytes) = old_settings {
            let _ = write_private_atomic(&settings_path, &bytes);
        } else {
            let _ = std::fs::remove_file(&settings_path);
        }
        return Err(error);
    }
    Ok(settings_path)
}

/// Resuelve el directorio de datos que heredará el proceso `mcode`.
pub fn mcode_data_dir(home: &Path) -> PathBuf {
    let override_dir = ["MINIMAX_DATA_DIR", "MAVIS_DATA_DIR"]
        .into_iter()
        .find_map(|key| std::env::var_os(key).filter(|value| !value.is_empty()));
    if let Some(value) = override_dir {
        let path = PathBuf::from(value);
        return if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| home.to_path_buf())
                .join(path)
        };
    }
    if let Some(profile) = std::env::var_os("MINIMAX_PROFILE").filter(|v| !v.is_empty()) {
        return home.join(format!(".minimax-{}", profile.to_string_lossy()));
    }
    home.join(".minimax")
}

/// Configura o desconfigura modelos en OpenCode de forma atómica.
pub fn opencode(
    home: &Path,
    state_dir: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let dir = home.join(".config/opencode");
    let json_path = dir.join("opencode.json");
    let jsonc_path = dir.join("opencode.jsonc");
    let path = if json_path.exists() {
        json_path
    } else if jsonc_path.exists() {
        jsonc_path
    } else {
        json_path
    };
    let mut root: Value = if path.exists() {
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&data).map_err(|e| {
            format!(
                "{} no es JSON estricto modificable con seguridad ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({})
    };
    let root_obj = root.as_object_mut().ok_or_else(|| {
        "la raíz de la configuración de OpenCode debe ser un objeto JSON".to_string()
    })?;
    // Migra entradas de Ropero desde la clave singular no reconocida por
    // OpenCode. Sus providers personalizados sólo se leen desde `providers`.
    if let Some(mut old_value) = root_obj.remove("provider") {
        if let Some(old) = old_value.as_object_mut() {
            let managed: Vec<String> = old
                .iter()
                .filter_map(|(id, p)| {
                    (id.starts_with("ropero-")
                        && p.get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|n| n.starts_with("This is ROPERO! (")))
                    .then(|| id.clone())
                })
                .collect();
            if !managed.is_empty() {
                let plural = root_obj.entry("providers").or_insert_with(|| json!({}));
                let plural = plural.as_object_mut().ok_or_else(|| {
                    "`providers` en OpenCode no es un objeto; no se ha alterado".to_string()
                })?;
                for id in managed {
                    if let Some(old_provider) = old.remove(&id) {
                        if let Some(existing) = plural.get_mut(&id).and_then(Value::as_object_mut) {
                            let old_models = old_provider.get("models").and_then(Value::as_object);
                            if let (Some(target), Some(source)) = (
                                existing.get_mut("models").and_then(Value::as_object_mut),
                                old_models,
                            ) {
                                for (name, model) in source {
                                    target.entry(name.clone()).or_insert_with(|| model.clone());
                                }
                            }
                        } else {
                            plural.insert(id, old_provider);
                        }
                    }
                }
            }
            if !old.is_empty() {
                root_obj.insert("provider".into(), old_value);
            }
        } else {
            root_obj.insert("provider".into(), old_value);
        }
    }
    if uninstall && !root_obj.contains_key("providers") {
        write_json_atomic(&path, &root)?;
        return Ok(path);
    }
    let providers = root_obj
        .entry("providers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "`providers` en OpenCode no es un objeto; no se ha alterado".to_string())?;

    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| {
                format!(
                    "no hay manifiesto para el engine {}",
                    model.model.engine.as_str()
                )
            })?;
        let provider_id = format!("ropero-{}", manifest.id.as_str());
        if uninstall {
            if let Some(provider) = providers
                .get_mut(&provider_id)
                .and_then(Value::as_object_mut)
            {
                let managed_name = format!("This is ROPERO! ({})", manifest.id.as_str());
                if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
                    continue;
                }
                if let Some(models_obj) = provider.get_mut("models").and_then(Value::as_object_mut)
                {
                    models_obj.remove(&model.model.name);
                    if models_obj.is_empty()
                        && provider.get("name").and_then(Value::as_str)
                            == Some(managed_name.as_str())
                    {
                        providers.remove(&provider_id);
                    }
                }
            }
            continue;
        }

        let active_port = crate::state::managed_ports(state_dir)
            .into_iter()
            .find_map(|p| {
                let state = crate::state::read_state(state_dir, p)?;
                (state.model == model.model.name && state.engine == manifest.id.as_str())
                    .then_some(state.port.unwrap_or(p))
            });
        let port = active_port
            .or_else(|| model.model.port.map(|p| p.0))
            .unwrap_or(manifest.port_range.min);
        let provider = providers.entry(provider_id.clone()).or_insert_with(|| {
            let mut p = json!({
                "package": OPENCODE_PACKAGE,
                "name": format!("This is ROPERO! ({})", manifest.id.as_str()),
                "settings": { "baseURL": format!("http://127.0.0.1:{port}/v1") },
                "models": {}
            });
            if let Some(key) = config
                .engines
                .get(&manifest.id)
                .and_then(|e| e.api_key.as_ref())
            {
                p["settings"]["apiKey"] = Value::String(key.clone());
            }
            p
        });
        let managed_name = format!("This is ROPERO! ({})", manifest.id.as_str());
        if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
            return Err(format!(
                "OpenCode ya tiene `{provider_id}` sin marca de Ropero; no se ha alterado"
            ));
        }
        let provider = provider.as_object_mut().ok_or_else(|| {
            format!("provider `{provider_id}` no es un objeto; no se ha alterado")
        })?;
        let settings = provider.entry("settings").or_insert_with(|| json!({}));
        let settings = settings.as_object_mut().ok_or_else(|| {
            format!("`settings` de `{provider_id}` no es un objeto; no se ha alterado")
        })?;
        settings.insert(
            "baseURL".into(),
            json!(format!("http://127.0.0.1:{port}/v1")),
        );
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.as_ref())
        {
            settings.insert("apiKey".into(), Value::String(key.clone()));
        } else {
            settings.remove("apiKey");
        }
        let model_map = provider
            .entry("models")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                format!("`models` de `{provider_id}` no es un objeto; no se ha alterado")
            })?;
        let context = u64::from(model.model.context.max(1));
        let mut model_config = json!({
            "name": model.model.name.clone(),
            "tool_call": true,
            "reasoning": true,
            "limit": {"context": context, "input": context * 80 / 100, "output": context * 25 / 100},
            "compaction": {"auto": true, "keep": {"tokens": context * 80 / 100}, "buffer": "20%"}
        });
        if !model.levels.list.is_empty() {
            let efforts: Vec<&str> = model
                .levels
                .list
                .iter()
                .filter(|level| level.as_str() != "none")
                .map(String::as_str)
                .collect();
            model_config["capabilities"]["reasoning"] = json!({
                "supported": true,
                "toggle": true,
                "effort": efforts,
            });
            model_config["variants"] = json!(
                model
                    .levels
                    .list
                    .iter()
                    .map(|level| opencode_reasoning_variant(model, level))
                    .collect::<Vec<_>>()
            );
        }
        model_map.insert(model.model.name.clone(), model_config);
    }
    // Versiones anteriores pudieron dejar modelos sin etiqueta en el
    // provider singular o en providers Ropero ya existentes. Completa sólo
    // el campo ausente; no cambia nombres definidos por el usuario.
    if !uninstall {
        for (provider_id, provider) in providers.iter_mut() {
            if !provider_id.starts_with("ropero-")
                || !provider
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name.starts_with("This is ROPERO! ("))
            {
                continue;
            }
            if let Some(models) = provider.get_mut("models").and_then(Value::as_object_mut) {
                for (model_id, model) in models {
                    if let Some(model) = model.as_object_mut() {
                        model
                            .entry("name")
                            .or_insert_with(|| Value::String(model_id.clone()));
                    }
                }
            }
        }
    }
    if uninstall && providers.is_empty() {
        root_obj.remove("providers");
    }
    write_json_atomic(&path, &root)?;
    Ok(path)
}

/// Configura el catálogo dedicado de providers de OpenClaw para `main`.
/// JSON5 no se reescribe: sólo modificamos JSON estricto para preservar integridad.
pub fn openclaw(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    json_array_provider(
        home.join(".openclaw/agents/main/agent/models.json"),
        "ROPERO",
        home,
        config,
        engines,
        models,
        uninstall,
    )
}

/// Configura Pi Coding Agent usando el mismo formato oficial de providers
/// OpenAI-compatible, bajo su archivo de modelos propio.
pub fn pi(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    json_array_provider(
        home.join(".pi/agent/models.json"),
        "ROPERO",
        home,
        config,
        engines,
        models,
        uninstall,
    )
}

/// Registra providers de Codex CLI. Codex sólo acepta Responses API; Ignis la
/// implementa. Los modelos se solicitan por su ID al seleccionar este provider.
pub fn codex(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let path = home.join(".codex/config.toml");
    let mut doc = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?
            .parse::<DocumentMut>()
            .map_err(|e| {
                format!(
                    "{} no es TOML válido ({e}); no se ha alterado",
                    path.display()
                )
            })?
    } else {
        DocumentMut::new()
    };
    if doc["model_providers"].is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    } else if !doc["model_providers"].is_table() {
        return Err("`model_providers` en Codex no es una tabla; no se ha alterado".into());
    }
    let providers = doc["model_providers"]
        .as_table_mut()
        .expect("tabla verificada");
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let id = format!("ropero-{}", manifest.id.as_str());
        let marker = home.join(format!(
            ".config/ropero/connect/codex/{}.json",
            manifest.id.as_str()
        ));
        let marker_exists = marker.exists();
        let mut installed = if marker_exists {
            serde_json::from_slice::<Vec<String>>(
                &std::fs::read(&marker)
                    .map_err(|e| format!("no se pudo leer el registro de Codex: {e}"))?,
            )
            .map_err(|e| format!("registro de Codex inválido ({e}); no se ha alterado"))?
        } else {
            Vec::new()
        };
        if uninstall {
            if !marker_exists {
                return Err(format!(
                    "Ropero no tiene un registro propio del provider de Codex `{id}`; no se ha alterado"
                ));
            }
            if providers
                .get(&id)
                .and_then(|p| p.get("name"))
                .and_then(Item::as_str)
                != Some("ROPERO")
            {
                return Err(format!(
                    "el provider de Codex `{id}` ya no conserva la marca Ropero; no se ha retirado"
                ));
            }
            installed.retain(|name| name != &model.model.name);
            if installed.is_empty() {
                providers.remove(&id);
                let _ = std::fs::remove_file(&marker);
            } else {
                write_json_atomic(&marker, &json!(installed))?;
            }
            continue;
        }
        if providers.contains_key(&id) && !marker_exists {
            return Err(format!(
                "Codex ya tiene `{id}` sin marca de Ropero; no se ha alterado"
            ));
        }
        if providers.contains_key(&id) && !providers[&id].is_table() {
            return Err(format!(
                "Codex ya tiene un provider incompatible `{id}`; no se ha alterado"
            ));
        }
        if !providers.contains_key(&id) {
            let table = providers.entry(&id).or_insert(Item::Table(Table::new()));
            table["name"] = value("ROPERO");
        }
        let table = providers
            .get_mut(&id)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| format!("provider `{id}` no es una tabla"))?;
        let active = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        table["name"] = value("ROPERO");
        table["base_url"] = value(format!("http://127.0.0.1:{active}/v1"));
        table["wire_api"] = value("responses");
        table["requires_openai_auth"] = value(false);
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.as_ref())
        {
            table["experimental_bearer_token"] = value(key.as_str());
        } else {
            table.remove("experimental_bearer_token");
        }
        installed.push(model.model.name.clone());
        installed.sort();
        installed.dedup();
        write_json_atomic(&marker, &json!(installed))?;
    }
    let mut bytes = doc.to_string().into_bytes();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    write_temp_private(&tmp, &bytes)
        .map_err(|e| format!("no se pudo preparar configuración protegida: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo reemplazar {}: {e}", path.display())
    })?;
    bytes.clear();
    Ok(path)
}

/// Configura providers OpenAI-compatible en el archivo global de Kilo Code.
/// La sintaxis JSONC se acepta sólo cuando también es JSON estricto para no
/// reescribir comentarios ni extensiones desconocidas.
pub fn kilo(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<(), String> {
    let dir = home.join(".config/kilo");
    let json_path = dir.join("kilo.json");
    let jsonc_path = dir.join("kilo.jsonc");
    let path = if json_path.exists() {
        json_path
    } else if jsonc_path.exists() {
        jsonc_path
    } else {
        json_path
    };
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} contiene JSONC u otra sintaxis que no se puede reescribir sin pérdida ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({})
    };
    let root = root
        .as_object_mut()
        .ok_or_else(|| "la raíz de kilo.json debe ser un objeto".to_string())?;
    let providers = root
        .entry("provider")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "`provider` en Kilo no es un objeto; no se ha alterado".to_string())?;
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let id = format!("ropero-{}", manifest.id.as_str());
        let managed_name = format!("ROPERO ({})", manifest.id.as_str());
        if uninstall {
            if let Some(provider) = providers.get_mut(&id).and_then(Value::as_object_mut) {
                if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
                    continue;
                }
                if let Some(list) = provider.get_mut("models").and_then(Value::as_object_mut) {
                    list.remove(&model.model.name);
                    if list.is_empty() {
                        providers.remove(&id);
                    }
                }
            }
            continue;
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let provider = providers.entry(id.clone()).or_insert_with(|| {
            json!({
                "name": managed_name.clone(),
                "npm": "@ai-sdk/openai-compatible",
                "options": {
                    "baseURL": format!("http://127.0.0.1:{port}/v1"),
                    "apiKey": config.engines.get(&manifest.id)
                        .and_then(|e| e.api_key.as_deref()).unwrap_or("not-needed")
                },
                "models": {}
            })
        });
        if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
            return Err(format!(
                "Kilo ya tiene el provider `{id}` sin marca de Ropero; no se ha alterado"
            ));
        }
        let provider = provider
            .as_object_mut()
            .ok_or_else(|| format!("provider `{id}` en Kilo no es un objeto; no se ha alterado"))?;
        let options = provider
            .entry("options")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| format!("`options` de Kilo `{id}` no es un objeto"))?;
        options.insert(
            "baseURL".into(),
            json!(format!("http://127.0.0.1:{port}/v1")),
        );
        options.insert(
            "apiKey".into(),
            json!(
                config
                    .engines
                    .get(&manifest.id)
                    .and_then(|e| e.api_key.as_deref())
                    .unwrap_or("not-needed")
            ),
        );
        let model_map = provider
            .entry("models")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| format!("`models` de Kilo `{id}` no es un objeto"))?;
        let context = u64::from(model.model.context.max(1));
        model_map.insert(
            model.model.name.clone(),
            json!({
                "name": model.model.name,
                "limit": {"context": context, "output": (context * 25 / 100).max(1)}
            }),
        );
    }
    write_json_atomic(&path, &Value::Object(root.clone()))?;
    Ok(())
}

/// Registra custom models de Factory Droid en settings.json. Droid no tiene
/// una tabla de providers: cada modelo incluye su endpoint y formato.
pub fn droid(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let path = home.join(".factory/settings.json");
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} no es JSON estricto modificable con seguridad ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({})
    };
    let root = root
        .as_object_mut()
        .ok_or_else(|| "la raíz de settings.json de Droid debe ser un objeto".to_string())?;
    let custom_models = root
        .entry("customModels")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| "`customModels` de Droid no es una lista; no se ha alterado".to_string())?;
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let display_name = format!("ROPERO ({}) · {}", manifest.id.as_str(), model.model.name);
        if uninstall {
            custom_models.retain(|entry| {
                !(entry.get("model").and_then(Value::as_str) == Some(model.model.name.as_str())
                    && entry.get("displayName").and_then(Value::as_str)
                        == Some(display_name.as_str()))
            });
            continue;
        }
        if custom_models.iter().any(|entry| {
            entry.get("model").and_then(Value::as_str) == Some(model.model.name.as_str())
                && entry.get("displayName").and_then(Value::as_str) != Some(display_name.as_str())
        }) {
            return Err(format!(
                "Droid ya tiene un custom model con ID `{}`; no se ha alterado",
                model.model.name
            ));
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let context = u64::from(model.model.context.max(1));
        let mut entry = json!({
            "model": model.model.name,
            "displayName": display_name,
            "baseUrl": format!("http://127.0.0.1:{port}/v1"),
            "provider": "generic-chat-completion-api",
            "maxContextLimit": context,
            "maxOutputTokens": (context * 25 / 100).max(1)
        });
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|engine| engine.api_key.as_ref())
        {
            entry["apiKey"] = Value::String(key.clone());
        }
        if let Some(existing) = custom_models.iter_mut().find(|entry| {
            entry.get("model").and_then(Value::as_str) == Some(model.model.name.as_str())
                && entry.get("displayName").and_then(Value::as_str) == Some(display_name.as_str())
        }) {
            *existing = entry;
        } else {
            custom_models.push(entry);
        }
    }
    write_json_atomic(&path, &Value::Object(root.clone()))?;
    Ok(path)
}

/// Registra modelos personalizados en CodeBuddy. Su esquema permite un
/// endpoint por modelo y usa `availableModels` como filtro opcional.
pub fn codebuddy(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let path = home.join(".codebuddy/models.json");
    let marker = home.join(".config/ropero/connect/codebuddy.json");
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} no es JSON estricto modificable con seguridad ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({"models": []})
    };
    let root = root
        .as_object_mut()
        .ok_or_else(|| "la raíz de models.json de CodeBuddy debe ser un objeto".to_string())?;
    if !root.contains_key("models") {
        root.insert("models".to_string(), json!([]));
    }
    let mut added_to_filter = if marker.exists() {
        serde_json::from_slice::<Vec<String>>(
            &std::fs::read(&marker)
                .map_err(|e| format!("no se pudo leer el registro de CodeBuddy: {e}"))?,
        )
        .map_err(|e| format!("registro de CodeBuddy inválido ({e}); no se ha alterado"))?
    } else {
        Vec::new()
    };
    let mut ids = std::collections::BTreeSet::new();
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let id = model.model.name.as_str();
        if !ids.insert(id) {
            return Err(format!(
                "CodeBuddy no puede distinguir el ID `{id}` repetido entre engines; no se ha alterado"
            ));
        }
        let display_name = format!("ROPERO ({}) · {id}", manifest.id.as_str());
        if uninstall {
            let existed = {
                let entries = root
                    .get_mut("models")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| "`models` de CodeBuddy no es una lista".to_string())?;
                let existed = entries.iter().any(|entry| {
                    entry.get("id").and_then(Value::as_str) == Some(id)
                        && entry.get("name").and_then(Value::as_str) == Some(display_name.as_str())
                });
                entries.retain(|entry| {
                    !(entry.get("id").and_then(Value::as_str) == Some(id)
                        && entry.get("name").and_then(Value::as_str) == Some(display_name.as_str()))
                });
                existed
            };
            if existed && added_to_filter.iter().any(|managed| managed == id) {
                if let Some(filter) = root
                    .get_mut("availableModels")
                    .and_then(Value::as_array_mut)
                {
                    filter.retain(|entry| entry.as_str() != Some(id));
                }
                added_to_filter.retain(|managed| managed != id);
            }
            continue;
        }
        let entries = root
            .get_mut("models")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "`models` de CodeBuddy no es una lista".to_string())?;
        if entries.iter().any(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(id)
                && entry.get("name").and_then(Value::as_str) != Some(display_name.as_str())
        }) {
            return Err(format!(
                "CodeBuddy ya tiene un modelo con ID `{id}` que no pertenece a Ropero; no se ha alterado"
            ));
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let context = u64::from(model.model.context.max(1));
        let mut entry = json!({
            "id": id,
            "name": display_name,
            "vendor": "ROPERO",
            "url": format!("http://127.0.0.1:{port}/v1/chat/completions"),
            "maxInputTokens": context * 80 / 100,
            "maxOutputTokens": (context * 25 / 100).max(1),
            "supportsToolCall": true,
            "supportsReasoning": true,
            "supportsImages": false
        });
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|engine| engine.api_key.as_ref())
        {
            entry["apiKey"] = Value::String(key.clone());
        }
        if let Some(existing) = entries.iter_mut().find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(id)
                && entry.get("name").and_then(Value::as_str) == Some(display_name.as_str())
        }) {
            *existing = entry;
        } else {
            entries.push(entry);
        }
        if let Some(filter) = root
            .get_mut("availableModels")
            .and_then(Value::as_array_mut)
        {
            if !filter.iter().any(|entry| entry.as_str() == Some(id)) {
                filter.push(json!(id));
                added_to_filter.push(id.to_owned());
            }
        }
    }
    write_json_atomic(&path, &Value::Object(root.clone()))?;
    if added_to_filter.is_empty() {
        if marker.exists() {
            std::fs::remove_file(&marker)
                .map_err(|e| format!("se actualizó CodeBuddy, pero no el registro ({e})"))?;
        }
    } else {
        added_to_filter.sort();
        added_to_filter.dedup();
        write_json_atomic(&marker, &json!(added_to_filter))?;
    }
    Ok(path)
}

/// Registra un provider Custom Endpoint en VS Code. Copilot consume este
/// mismo catálogo BYOK; su Agent Host puede requerir habilitación explícita.
pub fn vscode(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let path = home.join(".config/Code/User/chatLanguageModels.json");
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} no es JSON estricto modificable con seguridad ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!([])
    };
    let providers = root
        .as_array_mut()
        .ok_or_else(|| "la raíz de chatLanguageModels.json debe ser una lista".to_string())?;
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let group_name = format!("ROPERO ({})", manifest.id.as_str());
        let index = match providers.iter().position(|provider| {
            provider.get("name").and_then(Value::as_str) == Some(group_name.as_str())
        }) {
            Some(index) => index,
            None if uninstall => continue,
            None => {
                providers.push(json!({
                    "name": group_name,
                    "vendor": "customendpoint",
                    "apiType": "chat-completions",
                    "models": []
                }));
                providers.len() - 1
            }
        };
        let provider = providers
            .get_mut(index)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "provider de VS Code inválido; no se ha alterado".to_string())?;
        if provider.get("vendor").and_then(Value::as_str) != Some("customendpoint") {
            return Err(format!(
                "VS Code ya tiene el grupo `{group_name}` con otro provider; no se ha alterado"
            ));
        }
        let model_list = provider
            .entry("models")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("`models` de `{group_name}` no es una lista"))?;
        if uninstall {
            let had_model = model_list.iter().any(|entry| {
                entry.get("id").and_then(Value::as_str) == Some(model.model.name.as_str())
                    && entry.get("name").and_then(Value::as_str) == Some(model.model.name.as_str())
            });
            model_list.retain(|entry| {
                !(entry.get("id").and_then(Value::as_str) == Some(model.model.name.as_str())
                    && entry.get("name").and_then(Value::as_str) == Some(model.model.name.as_str()))
            });
            if had_model && model_list.is_empty() {
                providers.remove(index);
            }
            continue;
        }
        if model_list.iter().any(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(model.model.name.as_str())
                && entry.get("name").and_then(Value::as_str) != Some(model.model.name.as_str())
        }) {
            return Err(format!(
                "VS Code ya tiene el ID `{}` en `{group_name}`; no se ha alterado",
                model.model.name
            ));
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let context = u64::from(model.model.context.max(1));
        let entry = json!({
            "id": model.model.name,
            "name": model.model.name,
            "url": format!("http://127.0.0.1:{port}/v1/chat/completions"),
            "toolCalling": true,
            "vision": false,
            "maxInputTokens": context * 80 / 100,
            "maxOutputTokens": (context * 25 / 100).max(1)
        });
        if let Some(existing) = model_list.iter_mut().find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(model.model.name.as_str())
                && entry.get("name").and_then(Value::as_str) == Some(model.model.name.as_str())
        }) {
            *existing = entry;
        } else {
            model_list.push(entry);
        }
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|engine| engine.api_key.as_ref())
        {
            provider.insert("apiKey".into(), json!(key));
        } else {
            provider.remove("apiKey");
        }
    }
    write_json_atomic(&path, &root)?;
    Ok(path)
}

/// Usa la CLI documentada de MCode para añadir y retirar providers. MCode los
/// elimina como unidad, así que una baja parcial debe rechazarse sin mutar.
pub fn mcode(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<(), String> {
    let mut groups: std::collections::BTreeMap<String, Vec<&ModelSpec>> =
        std::collections::BTreeMap::new();
    for model in models {
        groups
            .entry(model.model.engine.as_str().to_owned())
            .or_default()
            .push(model);
    }
    for (engine, entries) in groups {
        let manifest = engines
            .iter()
            .find(|e| e.id.as_str() == engine)
            .ok_or_else(|| format!("no hay manifiesto para el engine {engine}"))?;
        let provider_name = format!("ropero-{engine}");
        let marker = mcode_data_dir(home)
            .join("ropero-connect")
            .join(format!("{engine}.json"));
        let marker_exists = marker.exists();
        let mut installed = if marker_exists {
            serde_json::from_slice::<Vec<String>>(
                &std::fs::read(&marker)
                    .map_err(|e| format!("no se pudo leer el registro de MCode: {e}"))?,
            )
            .map_err(|e| format!("registro de MCode inválido ({e}); no se ha alterado"))?
        } else {
            Vec::new()
        };
        let requested = entries
            .iter()
            .map(|m| m.model.name.clone())
            .collect::<Vec<_>>();
        if uninstall {
            if !marker_exists {
                return Err(format!(
                    "Ropero no tiene un registro propio del provider de MCode `{provider_name}`; no se ha alterado"
                ));
            }
            installed.retain(|name| !requested.contains(name));
            if installed.is_empty() {
                let status = std::process::Command::new("mcode")
                    .args(["provider", "remove", &provider_name, "--yes"])
                    .status()
                    .map_err(|e| format!("no se pudo ejecutar `mcode provider remove`: {e}"))?;
                if !status.success() {
                    return Err(format!(
                        "`mcode provider remove {provider_name} --yes` terminó con {status}; se conserva el registro de Ropero"
                    ));
                }
                std::fs::remove_file(&marker)
                    .map_err(|e| format!("se retiró el provider, pero no el registro ({e})"))?;
            } else {
                return Err(format!(
                    "MCode sólo permite retirar un provider completo. Quedarían los modelos Ropero {}; no se ha cambiado nada. Ejecuta --uninstall con todos los modelos registrados de ese engine: {}",
                    installed.join(", "),
                    installed.join(",")
                ));
            }
            continue;
        }
        let contexts = entries
            .iter()
            .map(|m| m.model.context)
            .collect::<std::collections::BTreeSet<_>>();
        if contexts.len() != 1 {
            return Err(format!(
                "MCode sólo admite un context-limit por provider y los modelos del engine {engine} tienen contextos distintos; no se ha configurado"
            ));
        }
        if !marker_exists {
            let listing = std::process::Command::new("mcode")
                .args(["provider", "list", "--json"])
                .output()
                .map_err(|e| format!("no se pudo inspeccionar providers de MCode: {e}"))?;
            if !listing.status.success() {
                return Err(format!(
                    "`mcode provider list --json` terminó con {}; no se ha alterado el perfil",
                    listing.status
                ));
            }
            let providers: Value = serde_json::from_slice(&listing.stdout).map_err(|e| {
                format!("MCode devolvió un listado JSON no reconocido ({e}); no se ha alterado")
            })?;
            if contains_provider_id(&providers, &provider_name) {
                return Err(format!(
                    "MCode ya tiene el provider `{provider_name}` y Ropero no lo registró; no se ha alterado"
                ));
            }
        }
        let context = u64::from(*contexts.first().unwrap_or(&4096));
        let port = entries[0]
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let mut command = std::process::Command::new("mcode");
        command
            .args(["provider", "add", "--name", &provider_name, "--base-url"])
            .arg(format!("http://127.0.0.1:{port}/v1"))
            .args(["--api-format", "openai-completions"]);
        for entry in entries {
            command.arg("--model").arg(&entry.model.name);
        }
        command.args([
            "--context-limit",
            &context.to_string(),
            "--output-limit",
            &(context * 25 / 100).max(1).to_string(),
        ]);
        let key = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.as_deref())
            .unwrap_or("not-needed");
        command
            .env("ROPERO_MCODE_PROVIDER_KEY", key)
            .args(["--api-key-env", "ROPERO_MCODE_PROVIDER_KEY"]);
        let status = command
            .status()
            .map_err(|e| format!("no se pudo ejecutar mcode (`mcode provider add`): {e}"))?;
        if !status.success() {
            return Err(format!(
                "`mcode provider add` terminó con {status}; comprueba el perfil y el endpoint, no se continuó con los demás engines"
            ));
        }
        installed.extend(requested);
        installed.sort();
        installed.dedup();
        if let Err(error) = write_json_atomic(&marker, &json!(installed)) {
            if !marker_exists {
                let rollback = std::process::Command::new("mcode")
                    .args(["provider", "remove", &provider_name, "--yes"])
                    .status();
                return match rollback {
                    Ok(status) if status.success() => Err(format!(
                        "no se pudo guardar el registro de Ropero ({error}); se revirtió el provider recién creado"
                    )),
                    Ok(status) => Err(format!(
                        "no se pudo guardar el registro ({error}) ni revertir el provider (mcode terminó con {status})"
                    )),
                    Err(rollback_error) => Err(format!(
                        "no se pudo guardar el registro ({error}) ni ejecutar la reversión: {rollback_error}"
                    )),
                };
            }
            return Err(format!(
                "no se pudo actualizar el registro de Ropero ({error}); el provider existente sigue instalado"
            ));
        }
    }
    Ok(())
}

/// Hermes exposes a documented config CLI, including `unset`. Ropero keeps a
/// small ownership manifest so uninstall never removes a similarly named user
/// provider. Credentials go into Hermes' `.env`, not command-line arguments.
pub fn hermes(
    home: &Path,
    hermes_home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<(), String> {
    let mut groups: std::collections::BTreeMap<String, Vec<&ModelSpec>> =
        std::collections::BTreeMap::new();
    for model in models {
        groups
            .entry(model.model.engine.as_str().to_owned())
            .or_default()
            .push(model);
    }
    for (engine, entries) in groups {
        let manifest = engines
            .iter()
            .find(|e| e.id.as_str() == engine)
            .ok_or_else(|| format!("no hay manifiesto para el engine {engine}"))?;
        let provider = format!("ropero-{engine}");
        let marker = home.join(format!(".config/ropero/connect/hermes/{engine}.json"));
        let env_key = format!(
            "ROPERO_HERMES_{}_API_KEY",
            engine.to_ascii_uppercase().replace('-', "_")
        );
        let marker_exists = marker.exists();
        let mut installed = if marker_exists {
            serde_json::from_slice::<Vec<String>>(
                &std::fs::read(&marker)
                    .map_err(|e| format!("no se pudo leer el manifiesto de Hermes: {e}"))?,
            )
            .map_err(|e| {
                format!("manifiesto de Hermes inválido ({e}); no se ha tocado su config")
            })?
        } else {
            let existing = hermes_config(hermes_home, &["get", &format!("providers.{provider}")])?;
            if !String::from_utf8_lossy(&existing.stdout).trim().is_empty() {
                return Err(format!(
                    "Hermes ya tiene `providers.{provider}` y Ropero no lo registró; no se ha alterado"
                ));
            }
            Vec::new()
        };
        let requested = entries
            .iter()
            .map(|m| m.model.name.clone())
            .collect::<Vec<_>>();
        if uninstall {
            if installed.is_empty() {
                return Err(format!(
                    "Ropero no tiene un registro propio del provider de Hermes `{provider}`; no se ha alterado"
                ));
            }
            installed.retain(|name| !requested.contains(name));
            if installed.is_empty() {
                hermes_config(hermes_home, &["unset", &format!("providers.{provider}")])?;
                dotenv_set(hermes_home, &env_key, None, true)?;
                std::fs::remove_file(&marker)
                    .map_err(|e| format!("se retiró el provider, pero no el manifiesto ({e})"))?;
            } else {
                let list = serde_json::to_string(&installed).map_err(|e| e.to_string())?;
                hermes_config(
                    hermes_home,
                    &["set", &format!("providers.{provider}.models"), &list],
                )?;
                write_json_atomic(&marker, &json!(installed))?;
            }
            continue;
        }
        let port = entries[0]
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let url = format!("http://127.0.0.1:{port}/v1");
        let api_path = format!("providers.{provider}.api");
        let key = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.as_deref())
            .unwrap_or("not-needed");
        // Validate key ownership and persist it before changing config.yaml.
        // If any CLI update fails for a new provider, roll the partial provider back.
        let snapshot = hermes_snapshot(hermes_home, &marker)?;
        dotenv_set(hermes_home, &env_key, Some(key), marker_exists)?;
        installed.extend(requested);
        installed.sort();
        installed.dedup();
        let list = serde_json::to_string(&installed).map_err(|e| e.to_string())?;
        let changes = (|| {
            hermes_config(hermes_home, &["set", &api_path, &url])?;
            hermes_config(
                hermes_home,
                &["set", &format!("providers.{provider}.key_env"), &env_key],
            )?;
            hermes_config(
                hermes_home,
                &[
                    "set",
                    &format!("providers.{provider}.discover_models"),
                    "false",
                ],
            )?;
            hermes_config(
                hermes_home,
                &["set", &format!("providers.{provider}.models"), &list],
            )?;
            Ok::<(), String>(())
        })();
        if let Err(error) = changes {
            return match restore_hermes_snapshot(hermes_home, &marker, &snapshot) {
                Ok(()) => Err(error),
                Err(rollback) => Err(format!("{error}; además falló la reversión: {rollback}")),
            };
        }
        if let Err(error) = write_json_atomic(&marker, &json!(installed)) {
            return match restore_hermes_snapshot(hermes_home, &marker, &snapshot) {
                Ok(()) => Err(error),
                Err(rollback) => Err(format!("{error}; además falló la reversión: {rollback}")),
            };
        }
    }
    Ok(())
}

struct HermesSnapshot {
    config: Option<Vec<u8>>,
    dotenv: Option<Vec<u8>>,
    marker: Option<Vec<u8>>,
}

fn hermes_snapshot(home: &Path, marker: &Path) -> Result<HermesSnapshot, String> {
    fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("no se pudo leer {}: {error}", path.display())),
        }
    }
    Ok(HermesSnapshot {
        config: read_optional(&home.join("config.yaml"))?,
        dotenv: read_optional(&home.join(".env"))?,
        marker: read_optional(marker)?,
    })
}

fn restore_hermes_snapshot(
    home: &Path,
    marker: &Path,
    snapshot: &HermesSnapshot,
) -> Result<(), String> {
    fn restore(path: &Path, bytes: &Option<Vec<u8>>) -> Result<(), String> {
        if let Some(bytes) = bytes {
            write_private_atomic(path, bytes)
        } else {
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("no se pudo retirar {}: {error}", path.display())),
            }
        }
    }
    restore(&home.join("config.yaml"), &snapshot.config)?;
    restore(&home.join(".env"), &snapshot.dotenv)?;
    restore(marker, &snapshot.marker)
}

fn hermes_config(home: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let output = std::process::Command::new("hermes")
        .env("HERMES_HOME", home)
        .arg("config")
        .args(args)
        .output()
        .map_err(|e| format!("no se pudo ejecutar hermes config: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "hermes config {} terminó con {}; no se continuó",
            args[0], output.status
        ));
    }
    Ok(output)
}

fn dotenv_set(home: &Path, key: &str, value: Option<&str>, owned: bool) -> Result<(), String> {
    let path = home.join(".env");
    let original = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?
    } else {
        String::new()
    };
    let mut found = false;
    let mut lines = Vec::new();
    for line in original.lines() {
        if line
            .split_once('=')
            .is_some_and(|(name, _)| name.trim() == key)
        {
            found = true;
            if let Some(value) = value {
                lines.push(format!(
                    "{key}=\"{}\"",
                    value
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                ));
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    if found && value.is_some() && !owned {
        return Err(format!(
            "Hermes ya define `{key}` en {}; no se ha sobrescrito",
            path.display()
        ));
    }
    if !found && let Some(value) = value {
        lines.push(format!(
            "{key}=\"{}\"",
            value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        ));
    }
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    write_private_atomic(&path, content.as_bytes())
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "ruta sin directorio".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    write_temp_private(&tmp, bytes)
        .map_err(|e| format!("no se pudo escribir archivo temporal protegido: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo reemplazar {}: {e}", path.display())
    })
}

/// Crea el temporal ya con permisos privados, antes de escribir cualquier secreto.
fn write_temp_private(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut created = false;
    let result = (|| {
        let mut file = options.open(path)?;
        created = true;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if result.is_err() && created {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn contains_provider_id(value: &Value, id: &str) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            (key == id
                || (matches!(key.as_str(), "id" | "name" | "provider" | "providerId")
                    && value.as_str() == Some(id)))
                || contains_provider_id(value, id)
        }),
        Value::Array(values) => values.iter().any(|value| contains_provider_id(value, id)),
        _ => false,
    }
}

fn json_array_provider(
    path: PathBuf,
    brand: &str,
    _home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let mut root: Value = if path.exists() {
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&data).map_err(|e| {
            format!(
                "{} no es JSON estricto ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({"providers": {}})
    };
    let providers = root
        .as_object_mut()
        .ok_or_else(|| "la raíz de models.json debe ser un objeto".to_string())?
        .entry("providers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "`providers` no es un objeto; no se ha alterado".to_string())?;
    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let id = format!("ropero-{}", manifest.id.as_str());
        if uninstall {
            if let Some(provider) = providers.get_mut(&id).and_then(Value::as_object_mut) {
                let managed_name = format!("{brand} ({})", manifest.id.as_str());
                if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
                    continue;
                }
                if let Some(list) = provider.get_mut("models").and_then(Value::as_array_mut) {
                    list.retain(|entry| {
                        entry.get("id").and_then(Value::as_str) != Some(model.model.name.as_str())
                    });
                    if list.is_empty()
                        && provider.get("name").and_then(Value::as_str)
                            == Some(managed_name.as_str())
                    {
                        providers.remove(&id);
                    }
                }
            }
            continue;
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let provider = providers.entry(id.clone()).or_insert_with(|| {
            let mut p = json!({"name": format!("{brand} ({})", manifest.id.as_str()), "baseUrl": format!("http://127.0.0.1:{port}/v1"), "api": "openai-completions", "apiKey": "not-needed", "models": []});
            if let Some(key) = config.engines.get(&manifest.id).and_then(|e| e.api_key.as_ref()) { p["apiKey"] = Value::String(key.clone()); }
            p
        });
        let managed_name = format!("{brand} ({})", manifest.id.as_str());
        if provider.get("name").and_then(Value::as_str) != Some(managed_name.as_str()) {
            return Err(format!(
                "{} ya tiene `{id}` sin marca de Ropero; no se ha alterado",
                brand
            ));
        }
        let provider = provider
            .as_object_mut()
            .ok_or_else(|| format!("provider `{id}` no es un objeto"))?;
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        provider.insert(
            "baseUrl".into(),
            json!(format!("http://127.0.0.1:{port}/v1")),
        );
        provider.insert("api".into(), json!("openai-completions"));
        provider.insert(
            "apiKey".into(),
            json!(
                config
                    .engines
                    .get(&manifest.id)
                    .and_then(|e| e.api_key.as_deref())
                    .unwrap_or("not-needed")
            ),
        );
        let list = provider
            .entry("models")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("`models` de `{id}` no es una lista"))?;
        if !list
            .iter()
            .any(|entry| entry.get("id").and_then(Value::as_str) == Some(model.model.name.as_str()))
        {
            let context = u64::from(model.model.context.max(1));
            list.push(json!({"id": model.model.name, "name": model.model.name, "reasoning": true, "input": ["text"], "contextWindow": context, "contextTokens": context * 80 / 100, "maxTokens": context * 25 / 100, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}}));
        }
    }
    write_json_atomic(&path, &root)?;
    Ok(path)
}

/// Construye una variante OpenCode para un nivel declarado por el modelo.
/// Ignis acepta `enable_thinking`, `reasoning_effort` y `thinking_budget` en
/// cada petición; low/minimal conservan el esfuerzo pero desactivan el bloque
/// de razonamiento por defecto, tal como se declara en el TOML.
fn opencode_reasoning_variant(model: &ModelSpec, level: &str) -> Value {
    let cfg = model.args.level.get(level);
    let value_for = |flag: &str| {
        cfg.and_then(|cfg| {
            cfg.base
                .windows(2)
                .find(|pair| pair[0] == flag)
                .map(|pair| pair[1].as_str())
        })
    };
    let thinking_enabled = value_for("--model-enable-thinking");
    // Ignis requiere pensamiento activado para aceptar reasoning_effort. Los
    // niveles none/minimal/low deben omitir el campo por completo.
    let effort = (thinking_enabled != Some("false"))
        .then(|| value_for("--model-reasoning-effort").unwrap_or(level));
    let mut body = serde_json::Map::new();
    if let Some(enabled) = thinking_enabled {
        body.insert("enable_thinking".into(), json!(enabled == "true"));
    }
    if let Some(budget) = value_for("--model-thinking-budget") {
        if budget != "off" {
            if let Ok(tokens) = budget.parse::<u64>() {
                body.insert("thinking_budget".into(), json!(tokens));
            }
        }
    }
    let mut variant = json!({ "id": level, "body": body });
    if let Some(effort) = effort {
        variant["settings"]["reasoningEffort"] = json!(effort);
    }
    variant
}

#[cfg(test)]
mod tests {
    use super::opencode_reasoning_variant;
    use ropero_core::parse_model;
    use std::path::Path;

    #[test]
    fn ignis_opencode_variants_follow_the_declared_thinking_levels() {
        let model = parse_model(
            include_str!("../../../models/ignis/qwen3.8-27b-16g.toml"),
            Path::new("models/ignis/qwen3.8-27b-16g.toml"),
        )
        .unwrap();
        assert_eq!(model.model.context, 131072);
        let minimal = opencode_reasoning_variant(&model, "minimal");
        let low = opencode_reasoning_variant(&model, "low");
        let medium = opencode_reasoning_variant(&model, "medium");
        let max = opencode_reasoning_variant(&model, "max");

        assert_eq!(minimal["body"]["enable_thinking"], false);
        assert!(minimal.get("settings").is_none());
        assert_eq!(low["body"]["enable_thinking"], false);
        assert_eq!(low["body"]["thinking_budget"], 2048);
        assert!(low.get("settings").is_none());
        assert_eq!(medium["body"]["enable_thinking"], true);
        assert_eq!(medium["settings"]["reasoningEffort"], "medium");
        assert_eq!(max["body"]["enable_thinking"], true);
        assert!(max["body"].get("thinking_budget").is_none());
    }
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "ruta de configuración sin directorio".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_temp_private(&tmp, &bytes)
        .map_err(|e| format!("no se pudo preparar configuración protegida: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo reemplazar {}: {e}", path.display())
    })
}
