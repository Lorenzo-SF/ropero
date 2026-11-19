//! Integraciones con agentes de código. Cada engine se representa como un
//! provider y cada modelo queda agrupado bajo el provider correspondiente.

use std::path::{Path, PathBuf};

use jsonc_parser::{
    ParseOptions,
    cst::{CstInputValue, CstObject, CstRootNode},
};
use ropero_core::{EngineManifest, ModelSpec, config::Config};
use serde_json::{Value, json};
use toml_edit::{DocumentMut, Item, Table, value};

const OPENCODE_PACKAGE: &str = "@opencode/ai/providers/openai-compatible";

/// Agentes enumerados por `ropero connect --list` y estado de integración.
pub const AGENTS: &[(&str, &str)] = &[
    (
        "claude",
        "Anthropic Messages API: variables de entorno para engines compatibles",
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
        "trae",
        "manual: OpenAI Chat Completions custom model in Settings > Models",
    ),
    (
        "trae-cn",
        "manual: OpenAI Chat Completions custom model in Settings > Models",
    ),
    (
        "claw",
        "alias histórico de OpenClaw (Clawdbot/Moltbot); mismo adaptador",
    ),
    (
        "droid",
        "compatible: customModels en ~/.factory/settings.json",
    ),
    (
        "hermes",
        "compatible: CLI de configuración y credenciales protegidas en .env",
    ),
    (
        "pi",
        "compatible: ~/.pi/agent/models.json con providers custom",
    ),
    (
        "mcode",
        "CLI oficial; la baja retira providers completos, requiere seleccionar todos sus modelos",
    ),
    (
        "openclaw",
        "compatible: models.json por agente con providers custom",
    ),
    (
        "picoclaw",
        "compatible: config.json v2 y credenciales protegidas en .security.yml",
    ),
];

/// Engines whose documented HTTP surface includes OpenAI's Responses API,
/// which Codex CLI requires for custom providers.
pub fn codex_responses_supported(engine: &str) -> bool {
    matches!(
        engine,
        "ignis" | "llama-server" | "turbo-llama" | "strata" | "vllm" | "tensorrt-llm" | "mlx-serve"
    )
}

/// Genera la configuración de entorno que Claude Code necesita para hablar
/// con una API Anthropic Messages servida localmente.
/// Claude Code sólo admite un `ANTHROPIC_BASE_URL` por proceso, así que los
/// modelos solicitados deben compartir engine y puerto. La URL es la raíz
/// del servidor: Claude añade `/v1/messages` por su cuenta.
pub fn claude_env_instructions(
    config: &Config,
    models: &[ModelSpec],
    remove: bool,
) -> Result<Vec<String>, String> {
    let Some(first) = models.first() else {
        return Err("indica al menos un modelo para Claude Code".into());
    };
    let engine = first.model.engine.as_str();
    if !matches!(
        engine,
        "llama-server"
            | "turbo-llama"
            | "vllm"
            | "strata"
            | "basalt"
            | "tensorrt-llm"
            | "mlx-serve"
    ) {
        return Err(format!(
            "el engine '{engine}' no está documentado como compatible con Anthropic Messages"
        ));
    }
    if models
        .iter()
        .any(|model| model.model.engine.as_str() != engine)
    {
        return Err(
            "Claude Code usa un solo ANTHROPIC_BASE_URL; selecciona modelos de un mismo engine"
                .into(),
        );
    }
    let ports = models
        .iter()
        .filter_map(|model| model.model.port.map(|port| port.0))
        .collect::<std::collections::BTreeSet<_>>();
    if ports.len() > 1 {
        return Err(
            "Claude Code usa un solo endpoint; los modelos tienen puertos distintos".into(),
        );
    }
    let port = ports
        .first()
        .copied()
        .ok_or_else(|| "no se pudo resolver el puerto del engine".to_string())?;
    let aliases = models
        .iter()
        .map(|model| model.model.alias.as_deref().unwrap_or(&model.model.name))
        .collect::<Vec<_>>();
    if remove {
        return Ok(vec![
            "Claude Code no guarda providers: elimina o desactiva estas variables en tu shell/perfil:".into(),
            "ANTHROPIC_BASE_URL".into(),
            "ANTHROPIC_AUTH_TOKEN".into(),
            "API_TIMEOUT_MS".into(),
            "DISABLE_TELEMETRY".into(),
            "ANTHROPIC_MODEL".into(),
            "ANTHROPIC_DEFAULT_HAIKU_MODEL".into(),
            "ANTHROPIC_DEFAULT_OPUS_MODEL".into(),
            "ANTHROPIC_DEFAULT_SONNET_MODEL".into(),
            format!("modelos seleccionados: {}", aliases.join(", ")),
        ]);
    }
    let primary = aliases[0];
    let has_api_key = config
        .engines
        .get(&first.model.engine)
        .and_then(|engine| engine.api_key.as_deref())
        .is_some();
    let auth = if has_api_key {
        "<clave-api-configurada-en-ropero>"
    } else {
        "dummy"
    };
    let mut lines = vec![
        format!("Claude Code — engine {engine}, API Anthropic Messages"),
        format!("ANTHROPIC_BASE_URL=http://127.0.0.1:{port}"),
        format!("ANTHROPIC_AUTH_TOKEN={auth}"),
        "API_TIMEOUT_MS=600000".into(),
        "DISABLE_TELEMETRY=1".into(),
        format!("ANTHROPIC_MODEL={primary}"),
        format!("ANTHROPIC_DEFAULT_HAIKU_MODEL={primary}"),
        format!("ANTHROPIC_DEFAULT_OPUS_MODEL={primary}"),
        format!("ANTHROPIC_DEFAULT_SONNET_MODEL={primary}"),
        format!("modelos seleccionados: {}", aliases.join(", ")),
        "Ejecuta `claude` con estas variables exportadas. La URL no lleva `/v1`: Claude solicita `/v1/messages`.".into(),
    ];
    if has_api_key {
        lines.push("Sustituye `<clave-api-configurada-en-ropero>` por la clave del engine antes de exportar; Ropero no imprime credenciales en la terminal.".into());
    }
    Ok(lines)
}

/// Values for registering Ropero models through TRAE's documented UI. This
/// intentionally never returns API-key material, even if one is configured.
pub fn trae_instructions(
    agent: &str,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    remove: bool,
) -> Vec<String> {
    let edition = if agent == "trae-cn" {
        "China edition"
    } else {
        "international edition"
    };
    let mut lines = vec![format!(
        "TRAE manual setup ({edition}): Settings > Models > Add model > Custom model; choose OpenAI Chat Completions."
    )];
    for model in models {
        let alias = model.model.alias.as_deref().unwrap_or(&model.model.name);
        let port = model
            .model
            .port
            .map(|port| port.0)
            .or_else(|| {
                engines
                    .iter()
                    .find(|engine| engine.id == model.model.engine)
                    .map(|engine| engine.port_range.min)
            })
            .unwrap_or_default();
        if remove {
            lines.push(format!("  Remove custom model `{alias}` from TRAE."));
        } else {
            let key = config
                .engines
                .get(&model.model.engine)
                .and_then(|engine| engine.api_key.as_deref())
                .map(|_| "use the API key configured for this Ropero engine")
                .unwrap_or("leave the API key empty if the engine has no key configured");
            lines.push(format!(
                "  {alias}: request URL http://127.0.0.1:{port}/v1; model ID {alias}; {key}."
            ));
        }
    }
    lines.push(
        "Ropero does not edit TRAE's private UI-managed settings or print API key values."
            .to_string(),
    );
    lines
}

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
        let served = served_model_id(model);
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let engine = manifest.id.as_str();
        let alias = format!("openai/ropero-{engine}/{served}");
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
    let original = if path.exists() {
        Some(
            std::fs::read_to_string(&path)
                .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?,
        )
    } else {
        None
    };
    let mut root: Value = if let Some(data) = &original {
        jsonc_parser::parse_to_serde_value::<Value>(data, &ParseOptions::default()).map_err(
            |e| {
                format!(
                    "{} no es JSONC válido ({e}); no se ha alterado",
                    path.display()
                )
            },
        )?
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
                .filter(|(id, p)| {
                    id.starts_with("ropero-")
                        && p.get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|n| n.starts_with("This is ROPERO! ("))
                })
                .map(|(id, _)| id.clone())
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
        write_opencode_jsonc(&path, original.as_deref(), &root)?;
        return Ok(path);
    }
    let providers = root_obj
        .entry("providers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "`providers` en OpenCode no es un objeto; no se ha alterado".to_string())?;

    for model in models {
        let served = served_model_id(model);
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
                    models_obj.remove(served);
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
            "name": served,
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
        model_map.insert(served.to_string(), model_config);
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
    write_opencode_jsonc(&path, original.as_deref(), &root)?;
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

/// Registra modelos OpenAI-compatible en el formato versionado de PicoClaw.
/// Las claves se guardan en `.security.yml`, que PicoClaw indexa mediante
/// `<model_name>:0`; nunca se escriben en `config.json`.
pub fn picoclaw(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    let dir = home.join(".picoclaw");
    let path = dir.join("config.json");
    let security_path = dir.join(".security.yml");
    let marker = home.join(".config/ropero/connect/picoclaw.json");
    let original_marker = match std::fs::read(&marker) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "no se pudo leer el registro de PicoClaw {}: {error}",
                marker.display()
            ));
        }
    };
    let mut managed = if let Some(bytes) = &original_marker {
        serde_json::from_slice::<Vec<String>>(bytes)
            .map_err(|e| format!("registro de PicoClaw inválido ({e}); no se ha alterado"))?
    } else {
        Vec::new()
    };
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} no es JSON válido ({e}); no se ha alterado",
                path.display()
            )
        })?
    } else {
        json!({"version": 2, "model_list": []})
    };
    let object = root
        .as_object_mut()
        .ok_or_else(|| "la raíz de config.json de PicoClaw debe ser un objeto".to_string())?;
    if object.get("version").and_then(Value::as_u64) != Some(2) {
        return Err(format!(
            "{} debe usar el esquema de PicoClaw v2; no se ha alterado",
            path.display()
        ));
    }
    let entries = object
        .entry("model_list")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| "`model_list` de PicoClaw debe ser una lista".to_string())?;
    let original_security = match std::fs::read(&security_path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "no se pudo leer {}: {error}",
                security_path.display()
            ));
        }
    };
    let mut security: serde_yaml::Value = if let Some(bytes) = &original_security {
        serde_yaml::from_slice(bytes).map_err(|e| {
            format!(
                "{} no es YAML válido ({e}); no se ha alterado",
                security_path.display()
            )
        })?
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };
    let security_root = security.as_mapping_mut().ok_or_else(|| {
        format!(
            "{} debe contener un mapa YAML; no se ha alterado",
            security_path.display()
        )
    })?;
    let model_list_key = serde_yaml::Value::String("model_list".into());
    let model_secrets = security_root
        .entry(model_list_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        .as_mapping_mut()
        .ok_or_else(|| {
            format!(
                "`model_list` de {} debe ser un mapa YAML; no se ha alterado",
                security_path.display()
            )
        })?;

    for model in models {
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let served = served_model_id(model);
        let model_name = format!("ropero-{}-{served}", manifest.id.as_str());
        let security_key = format!("{model_name}:0");
        if uninstall {
            if !managed.iter().any(|owned| owned == &model_name) {
                continue;
            }
            entries.retain(|entry| {
                entry.get("model_name").and_then(Value::as_str) != Some(model_name.as_str())
            });
            model_secrets.remove(serde_yaml::Value::String(security_key));
            managed.retain(|owned| owned != &model_name);
            continue;
        }
        if entries.iter().any(|entry| {
            entry.get("model_name").and_then(Value::as_str) == Some(model_name.as_str())
                && !managed.iter().any(|owned| owned == &model_name)
        }) {
            return Err(format!(
                "PicoClaw ya tiene `{model_name}` sin registro de Ropero; no se ha alterado"
            ));
        }
        let port = model
            .model
            .port
            .map(|p| p.0)
            .unwrap_or(manifest.port_range.min);
        let entry = json!({
            "model_name": model_name.clone(),
            "model": format!("openai/{served}"),
            "api_base": format!("http://127.0.0.1:{port}/v1"),
        });
        if let Some(existing) = entries.iter_mut().find(|entry| {
            entry.get("model_name").and_then(Value::as_str) == Some(model_name.as_str())
        }) {
            *existing = entry;
        } else {
            entries.push(entry);
        }
        if !managed.iter().any(|owned| owned == &model_name) {
            managed.push(model_name);
        }
        if let Some(key) = config
            .engines
            .get(&manifest.id)
            .and_then(|engine| engine.api_key.as_deref())
        {
            let key_yaml = serde_yaml::Value::String(security_key);
            model_secrets.insert(
                key_yaml,
                serde_yaml::to_value(json!({"api_keys": [key]}))
                    .map_err(|e| format!("no se pudo serializar credencial de PicoClaw: {e}"))?,
            );
        } else {
            model_secrets.remove(serde_yaml::Value::String(security_key));
        }
    }
    if model_secrets.is_empty() {
        security_root.remove(serde_yaml::Value::String("model_list".into()));
    }
    let next_security = if security_root.is_empty() {
        None
    } else {
        Some(
            serde_yaml::to_string(&security)
                .map_err(|e| format!("no se pudo serializar .security.yml: {e}"))?
                .into_bytes(),
        )
    };
    managed.sort();
    managed.dedup();
    let next_marker = if managed.is_empty() {
        None
    } else {
        Some(
            serde_json::to_vec_pretty(&managed)
                .map_err(|e| format!("no se pudo serializar el registro de PicoClaw: {e}"))?,
        )
    };
    let marker_result = match &next_marker {
        Some(bytes) => write_private_atomic(&marker, bytes),
        None => remove_if_exists(&marker),
    };
    if let Err(error) = marker_result {
        return Err(format!(
            "no se pudo actualizar el registro de PicoClaw: {error}"
        ));
    }
    let security_result = match &next_security {
        Some(bytes) => write_private_atomic(&security_path, bytes),
        None => remove_if_exists(&security_path),
    };
    if let Err(error) = security_result {
        let rollback = restore_optional_private(&marker, &original_marker);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!(
                "{error}; además falló la restauración del registro: {rollback}"
            )),
        };
    }
    if let Err(error) = write_json_atomic(&path, &root) {
        let security_rollback = restore_optional_private(&security_path, &original_security);
        let marker_rollback = restore_optional_private(&marker, &original_marker);
        return match (security_rollback, marker_rollback) {
            (Ok(()), Ok(())) => Err(error),
            (security, marker) => Err(format!(
                "{error}; falló la restauración: .security.yml={security:?}, registro={marker:?}"
            )),
        };
    }
    Ok(path)
}

/// Registra providers utilizables desde Codex CLI.
///
/// Codex tiene un único `model_provider` activo por configuración. Un provider
/// definido en `model_providers` no queda seleccionado por registrarlo
/// solamente; por eso Ropero crea un archivo de perfil por engine, sin cambiar
/// el perfil global. Codex los carga desde `$CODEX_HOME/<perfil>.config.toml`.
pub fn codex(
    home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    codex_in_dir(
        home,
        &home.join(".codex"),
        config,
        engines,
        models,
        uninstall,
    )
}

/// Variante para respetar `CODEX_HOME`; `home` sigue siendo el HOME del
/// usuario donde Ropero guarda su registro de propiedad.
pub fn codex_in_dir(
    home: &Path,
    codex_home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
) -> Result<PathBuf, String> {
    codex_in_dir_with_api_keys(
        home,
        codex_home,
        config,
        engines,
        models,
        uninstall,
        &std::collections::BTreeMap::new(),
    )
}

/// Igual que `codex_in_dir`, permitiendo proporcionar la clave efectiva del
/// engine cuando procede de entorno o del estado privado de Ropero.
pub fn codex_in_dir_with_api_keys(
    home: &Path,
    codex_home: &Path,
    config: &Config,
    engines: &[EngineManifest],
    models: &[ModelSpec],
    uninstall: bool,
    api_keys: &std::collections::BTreeMap<ropero_core::EngineId, Option<String>>,
) -> Result<PathBuf, String> {
    if models.is_empty() {
        return Err("indica al menos un modelo para Codex".into());
    }
    let path = codex_home.join("config.toml");
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
    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    } else if !doc.get("model_providers").is_some_and(Item::is_table) {
        return Err("`model_providers` en Codex no es una tabla; no se ha alterado".into());
    }
    for model in models {
        let served = served_model_id(model);
        let manifest = engines
            .iter()
            .find(|e| e.id == model.model.engine)
            .ok_or_else(|| format!("no hay manifiesto para {}", model.model.engine.as_str()))?;
        let id = format!("ropero-{}", manifest.id.as_str());
        let profile_id = id.clone();
        let profile_path = codex_home.join(format!("{profile_id}.config.toml"));
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
            if doc["model_providers"]
                .get(&id)
                .and_then(|p| p.get("name"))
                .and_then(Item::as_str)
                != Some("ROPERO")
            {
                return Err(format!(
                    "el provider de Codex `{id}` ya no conserva la marca Ropero; no se ha retirado"
                ));
            }
            installed.retain(|name| name != served);
            if installed.is_empty() {
                doc.get_mut("model_providers")
                    .and_then(Item::as_table_mut)
                    .ok_or_else(|| {
                        "`model_providers` en Codex no es una tabla; no se ha alterado".to_string()
                    })?
                    .remove(&id);
                std::fs::remove_file(&profile_path).map_err(|e| {
                    format!(
                        "no se pudo retirar el perfil Codex {}: {e}",
                        profile_path.display()
                    )
                })?;
                let _ = std::fs::remove_file(&marker);
            } else {
                let mut profile = read_codex_profile(&profile_path)?;
                profile["model_provider"] = value(id.as_str());
                profile["model"] = value(installed[0].as_str());
                write_codex_profile(&profile_path, &profile)?;
                write_json_atomic(&marker, &json!(installed))?;
            }
            continue;
        }
        let provider_exists = doc
            .get("model_providers")
            .and_then(Item::as_table)
            .is_some_and(|providers| providers.contains_key(&id));
        if provider_exists && !marker_exists {
            return Err(format!(
                "Codex ya tiene `{id}` sin marca de Ropero; no se ha alterado"
            ));
        }
        if profile_path.exists() && !marker_exists {
            return Err(format!(
                "Codex ya tiene el perfil `{profile_id}` sin marca de Ropero; no se ha alterado"
            ));
        }
        if provider_exists
            && doc
                .get("model_providers")
                .and_then(Item::as_table)
                .and_then(|providers| providers.get(&id))
                .is_none_or(|provider| !provider.is_table())
        {
            return Err(format!(
                "Codex ya tiene un provider incompatible `{id}`; no se ha alterado"
            ));
        }
        if !provider_exists {
            let providers = doc
                .get_mut("model_providers")
                .and_then(Item::as_table_mut)
                .ok_or_else(|| "`model_providers` en Codex no es una tabla".to_string())?;
            let table = providers.entry(&id).or_insert(Item::Table(Table::new()));
            table["name"] = value("ROPERO");
        }
        let table = doc
            .get_mut("model_providers")
            .and_then(Item::as_table_mut)
            .ok_or_else(|| "`model_providers` en Codex no es una tabla".to_string())?
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
        match api_keys.get(&manifest.id) {
            Some(Some(key)) => table["experimental_bearer_token"] = value(key.as_str()),
            Some(None) => {
                table.remove("experimental_bearer_token");
            }
            None => {
                if let Some(key) = config
                    .engines
                    .get(&manifest.id)
                    .and_then(|entry| entry.api_key.as_deref())
                {
                    table["experimental_bearer_token"] = value(key);
                }
            }
        }
        installed.push(served.to_string());
        installed.sort();
        installed.dedup();
        let mut profile = read_codex_profile(&profile_path)?;
        profile["model_provider"] = value(id.as_str());
        profile["model"] = value(installed[0].as_str());
        write_codex_profile(&profile_path, &profile)?;
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

fn read_codex_profile(path: &Path) -> Result<DocumentMut, String> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    std::fs::read_to_string(path)
        .map_err(|e| format!("no se pudo leer el perfil Codex {}: {e}", path.display()))?
        .parse::<DocumentMut>()
        .map_err(|e| format!("perfil Codex {} no es TOML válido ({e})", path.display()))
}

fn write_codex_profile(path: &Path, doc: &DocumentMut) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "ruta de perfil Codex sin directorio".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    write_temp_private(&tmp, doc.to_string().as_bytes())
        .map_err(|e| format!("no se pudo preparar el perfil Codex: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo reemplazar {}: {e}", path.display())
    })
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
            && !filter.iter().any(|entry| entry.as_str() == Some(id))
        {
            filter.push(json!(id));
            added_to_filter.push(id.to_owned());
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

/// Registra un provider Custom Endpoint en VS Code. Las credenciales se
/// gestionan en VS Code: nunca se copian a chatLanguageModels.json.
pub fn vscode(
    home: &Path,
    _config: &Config,
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
        // Earlier versions copied the engine key into this JSON file. Remove
        // that legacy value; VS Code manages API credentials through its UI.
        provider.remove("apiKey");
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
            .map(|m| served_model_id(m).to_string())
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
                    "MCode sólo permite retirar un provider completo. Quedarían los modelos Ropero {}; no se ha cambiado nada. Ejecuta --remove con todos los modelos registrados de ese engine: {}",
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
            command.arg("--model").arg(served_model_id(entry));
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
    hermes_with_binary(
        Path::new("hermes"),
        home,
        hermes_home,
        config,
        engines,
        models,
        uninstall,
    )
}

fn hermes_with_binary(
    binary: &Path,
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
            let existing = hermes_config(
                binary,
                hermes_home,
                &["get", &format!("providers.{provider}")],
            )?;
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
                hermes_config(
                    binary,
                    hermes_home,
                    &["unset", &format!("providers.{provider}")],
                )?;
                dotenv_set(hermes_home, &env_key, None, true)?;
                std::fs::remove_file(&marker)
                    .map_err(|e| format!("se retiró el provider, pero no el manifiesto ({e})"))?;
            } else {
                let model_map = installed
                    .iter()
                    .map(|name| (name.clone(), json!({})))
                    .collect::<serde_json::Map<String, Value>>();
                let models = serde_json::to_string(&model_map).map_err(|e| e.to_string())?;
                hermes_config(
                    binary,
                    hermes_home,
                    &["set", &format!("providers.{provider}.models"), &models],
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
        let model_map = installed
            .iter()
            .map(|name| (name.clone(), json!({})))
            .collect::<serde_json::Map<String, Value>>();
        let models_json = serde_json::to_string(&model_map).map_err(|e| e.to_string())?;
        let changes = (|| {
            hermes_config(binary, hermes_home, &["set", &api_path, &url])?;
            hermes_config(
                binary,
                hermes_home,
                &["set", &format!("providers.{provider}.key_env"), &env_key],
            )?;
            hermes_config(
                binary,
                hermes_home,
                &[
                    "set",
                    &format!("providers.{provider}.discover_models"),
                    "false",
                ],
            )?;
            hermes_config(
                binary,
                hermes_home,
                &["set", &format!("providers.{provider}.models"), &models_json],
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

fn hermes_config(
    binary: &Path,
    home: &Path,
    args: &[&str],
) -> Result<std::process::Output, String> {
    let output = std::process::Command::new(binary)
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

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("no se pudo retirar {}: {error}", path.display())),
    }
}

fn restore_optional_private(path: &Path, previous: &Option<Vec<u8>>) -> Result<(), String> {
    if let Some(bytes) = previous {
        write_private_atomic(path, bytes)
    } else {
        remove_if_exists(path)
    }
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
        let served = served_model_id(model);
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
                    list.retain(|entry| entry.get("id").and_then(Value::as_str) != Some(served));
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
        let api_key = config
            .engines
            .get(&manifest.id)
            .and_then(|e| e.api_key.clone())
            .or_else(|| {
                provider
                    .get("apiKey")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "not-needed".into());
        provider.insert("apiKey".into(), json!(api_key));
        let list = provider
            .entry("models")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("`models` de `{id}` no es una lista"))?;
        if !list
            .iter()
            .any(|entry| entry.get("id").and_then(Value::as_str) == Some(served))
        {
            let context = u64::from(model.model.context.max(1));
            list.push(json!({"id": served, "name": served, "reasoning": true, "input": ["text"], "contextWindow": context, "contextTokens": context * 80 / 100, "maxTokens": context * 25 / 100, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}}));
        }
    }
    write_json_atomic(&path, &root)?;
    Ok(path)
}

fn served_model_id(model: &ModelSpec) -> &str {
    model.model.alias.as_deref().unwrap_or(&model.model.name)
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
    if let Some(budget) = value_for("--model-thinking-budget")
        && budget != "off"
        && let Ok(tokens) = budget.parse::<u64>()
    {
        body.insert("thinking_budget".into(), json!(tokens));
    }
    let mut variant = json!({ "id": level, "body": body });
    if let Some(effort) = effort {
        variant["settings"]["reasoningEffort"] = json!(effort);
    }
    variant
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

/// Reescribe el objeto administrado dentro del CST existente para conservar
/// comentarios, espacios y comas finales del JSONC de OpenCode.
fn write_opencode_jsonc(
    path: &Path,
    original: Option<&str>,
    desired: &Value,
) -> Result<(), String> {
    let root = match original {
        Some(text) => CstRootNode::parse(text, &ParseOptions::default()).map_err(|e| {
            format!(
                "{} no es JSONC válido ({e}); no se ha alterado",
                path.display()
            )
        })?,
        None => CstRootNode::parse("{}", &ParseOptions::default())
            .expect("un objeto JSON vacío siempre es válido"),
    };
    let destination = root
        .value()
        .and_then(|node| node.as_object())
        .ok_or_else(|| {
            format!(
                "la raíz de {} debe ser un objeto; no se ha alterado",
                path.display()
            )
        })?;
    let desired = desired
        .as_object()
        .ok_or_else(|| "la raíz de la configuración de OpenCode debe ser un objeto".to_string())?;
    sync_cst_object(&destination, desired);
    write_private_atomic(path, root.to_string().as_bytes())
}

fn sync_cst_object(destination: &CstObject, desired: &serde_json::Map<String, Value>) {
    let names: Vec<String> = destination
        .properties()
        .iter()
        .filter_map(|prop| prop.decoded_name())
        .collect();
    for name in names {
        if !desired.contains_key(&name)
            && let Some(prop) = destination.get(&name)
        {
            prop.remove();
        }
    }

    for (name, desired_value) in desired {
        match (destination.get(name), desired_value.as_object()) {
            (Some(prop), Some(desired_object)) => {
                if let Some(existing) = prop.object_value() {
                    sync_cst_object(&existing, desired_object);
                } else {
                    prop.set_value(cst_value(desired_value));
                }
            }
            (Some(prop), _) => prop.set_value(cst_value(desired_value)),
            (None, _) => {
                destination.append(name, cst_value(desired_value));
            }
        }
    }
}

fn cst_value(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => CstInputValue::Bool(*value),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(value.clone()),
        Value::Array(values) => CstInputValue::Array(values.iter().map(cst_value).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), cst_value(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        claude_env_instructions, codex, codex_in_dir, codex_responses_supported,
        hermes_with_binary, openclaw, opencode, opencode_reasoning_variant, pi, picoclaw, vscode,
    };
    use jsonc_parser::ParseOptions;
    use ropero_core::config::Config;
    use ropero_core::{parse_engine_manifest, parse_model};
    use serde_json::Value;
    use std::path::Path;
    use toml_edit::DocumentMut;

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

    #[test]
    fn codex_rechaza_una_lista_vacia_sin_crear_configuracion() {
        let home = std::env::temp_dir().join(format!("ropero-codex-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let result = codex(&home, &Config::default(), &[], &[], false);
        assert!(result.is_err());
        assert!(!home.join(".codex/config.toml").exists());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn codex_usa_code_home_y_restringe_engines_a_responses_api() {
        let home =
            std::env::temp_dir().join(format!("ropero-codex-custom-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let codex_home = home.join("codex-custom");
        let mut model = parse_model(
            include_str!("../../../models/turbo-llama/ornith.toml"),
            Path::new("models/turbo-llama/ornith.toml"),
        )
        .unwrap();
        model.model.alias = Some("ornith-publico".into());
        let engine = parse_engine_manifest(
            include_str!("../../../engines/turbo-llama.toml"),
            Path::new("engines/turbo-llama.toml"),
        )
        .unwrap();
        let result = codex_in_dir(
            &home,
            &codex_home,
            &Config::default(),
            &[engine],
            &[model],
            false,
        );
        assert!(result.is_ok(), "{result:?}");
        assert!(codex_home.join("config.toml").exists());
        assert!(codex_home.join("ropero-turbo-llama.config.toml").exists());
        assert!(codex_responses_supported("turbo-llama"));
        assert!(!codex_responses_supported("fastflowllm"));
        assert!(!codex_responses_supported("airllm"));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn codex_registra_modelo_en_perfil_del_engine_y_lo_retira_sin_tocar_config_ajena() {
        let home = std::env::temp_dir().join(format!("ropero-codex-model-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(
            home.join(".codex/config.toml"),
            "model = \"user-model\"\nmodel_provider = \"user-provider\"\n\n[mcp_servers.keep-me]\ncommand = \"keep\"\n",
        )
        .unwrap();
        let mut model = parse_model(
            include_str!("../../../models/turbo-llama/ornith.toml"),
            Path::new("models/turbo-llama/ornith.toml"),
        )
        .unwrap();
        model.model.alias = Some("ornith-publico".into());
        model.model.port = Some(ropero_core::Port(9994));
        let engine = parse_engine_manifest(
            include_str!("../../../engines/turbo-llama.toml"),
            Path::new("engines/turbo-llama.toml"),
        )
        .unwrap();
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("codex-test-key".into());

        let path = codex(
            &home,
            &configured,
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let doc = saved.parse::<DocumentMut>().unwrap();
        let provider = &doc["model_providers"]["ropero-turbo-llama"];
        let profile_path = home.join(".codex/ropero-turbo-llama.config.toml");
        let profile = std::fs::read_to_string(&profile_path)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(
            provider["base_url"].as_str(),
            Some("http://127.0.0.1:9994/v1")
        );
        assert_eq!(provider["wire_api"].as_str(), Some("responses"));
        assert_eq!(
            provider["experimental_bearer_token"].as_str(),
            Some("codex-test-key")
        );
        assert_eq!(profile["model"].as_str(), Some("ornith-publico"));
        assert_eq!(
            profile["model_provider"].as_str(),
            Some("ropero-turbo-llama")
        );
        assert_eq!(doc["model"].as_str(), Some("user-model"));
        assert_eq!(doc["model_provider"].as_str(), Some("user-provider"));
        assert_eq!(
            doc["mcp_servers"]["keep-me"]["command"].as_str(),
            Some("keep")
        );

        codex(
            &home,
            &Config::default(),
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        let after_reconnect = std::fs::read_to_string(&path)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(
            after_reconnect["model_providers"]["ropero-turbo-llama"]["experimental_bearer_token"]
                .as_str(),
            Some("codex-test-key")
        );

        codex(
            &home,
            &Config::default(),
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            true,
        )
        .unwrap();
        let after = std::fs::read_to_string(path).unwrap();
        let doc = after.parse::<DocumentMut>().unwrap();
        assert!(doc["model_providers"].get("ropero-turbo-llama").is_none());
        assert_eq!(doc["model"].as_str(), Some("user-model"));
        assert_eq!(doc["model_provider"].as_str(), Some("user-provider"));
        assert!(!profile_path.exists());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn claude_muestra_url_messages_raiz_alias_y_variables_requeridas() {
        let mut model = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        model.model.alias = Some("groot-publico".into());
        model.model.port = Some(ropero_core::Port(9984));
        let lines = claude_env_instructions(&Config::default(), &[model.clone()], false).unwrap();
        let text = lines.join("\n");
        for expected in [
            "ANTHROPIC_BASE_URL=http://127.0.0.1:9984",
            "ANTHROPIC_AUTH_TOKEN=dummy",
            "API_TIMEOUT_MS=600000",
            "DISABLE_TELEMETRY=1",
            "ANTHROPIC_MODEL=groot-publico",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL=groot-publico",
            "ANTHROPIC_DEFAULT_OPUS_MODEL=groot-publico",
            "ANTHROPIC_DEFAULT_SONNET_MODEL=groot-publico",
        ] {
            assert!(text.contains(expected), "falta {expected}: {text}");
        }
        assert!(
            !text.contains(":9984/v1"),
            "Claude añade /v1/messages: {text}"
        );
        let mut configured = Config::default();
        configured
            .engines
            .entry(model.model.engine)
            .or_default()
            .api_key = Some("must-not-print-this".into());
        let keyed = claude_env_instructions(&configured, &[model], false)
            .unwrap()
            .join("\n");
        assert!(keyed.contains("<clave-api-configurada-en-ropero>"));
        assert!(keyed.contains("Sustituye `<clave-api-configurada-en-ropero>`"));
        assert!(!keyed.contains("must-not-print-this"));
    }

    #[test]
    fn claude_rechaza_multiples_engines_y_engines_sin_messages() {
        let mut llama = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        llama.model.port = Some(ropero_core::Port(9980));
        let ignis = parse_model(
            include_str!("../../../models/ignis/qwen3.8-27b-16g.toml"),
            Path::new("models/ignis/qwen3.8-27b-16g.toml"),
        )
        .unwrap();
        assert!(
            claude_env_instructions(&Config::default(), &[llama.clone(), ignis.clone()], false)
                .unwrap_err()
                .contains("un solo ANTHROPIC_BASE_URL")
        );
        assert!(
            claude_env_instructions(&Config::default(), &[ignis], false)
                .unwrap_err()
                .contains("no está documentado")
        );
    }

    #[test]
    fn picoclaw_v2_guarda_clave_separada_preserva_config_y_desinstala_solo_su_modelo() {
        let home = std::env::temp_dir().join(format!("ropero-picoclaw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let dir = home.join(".picoclaw");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            r#"{"version":2,"model_list":[{"model_name":"user-model","model":"openai/user","api_base":"http://example/v1"}],"agents":{"defaults":{"model":{"primary":"user-model"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join(".security.yml"),
            "model_list:\n  user-model:0:\n    api_keys:\n      - keep-me\n",
        )
        .unwrap();
        let mut model = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        model.model.alias = Some("groot-local".into());
        model.model.port = Some(ropero_core::Port(9984));
        let engine = parse_engine_manifest(
            include_str!("../../../engines/llama-server.toml"),
            Path::new("engines/llama-server.toml"),
        )
        .unwrap();
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("picoclaw-secret".into());

        let path = picoclaw(
            &home,
            &configured,
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        assert!(home.join(".config/ropero/connect/picoclaw.json").exists());
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], 2);
        assert_eq!(
            saved["agents"]["defaults"]["model"]["primary"],
            "user-model"
        );
        assert_eq!(saved["model_list"][0]["model_name"], "user-model");
        assert_eq!(
            saved["model_list"][1]["model_name"],
            "ropero-llama-server-groot-local"
        );
        assert_eq!(saved["model_list"][1]["model"], "openai/groot-local");
        assert_eq!(
            saved["model_list"][1]["api_base"],
            "http://127.0.0.1:9984/v1"
        );
        assert!(saved["model_list"][1].get("api_key").is_none());
        let secrets: serde_yaml::Value =
            serde_yaml::from_slice(&std::fs::read(dir.join(".security.yml")).unwrap()).unwrap();
        assert_eq!(
            secrets["model_list"]["user-model:0"]["api_keys"][0].as_str(),
            Some("keep-me")
        );
        assert_eq!(
            secrets["model_list"]["ropero-llama-server-groot-local:0"]["api_keys"][0].as_str(),
            Some("picoclaw-secret")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.join(".security.yml"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        picoclaw(
            &home,
            &Config::default(),
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            true,
        )
        .unwrap();
        let after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["model_list"].as_array().unwrap().len(), 1);
        assert_eq!(
            after["agents"]["defaults"]["model"]["primary"],
            "user-model"
        );
        let secrets: serde_yaml::Value =
            serde_yaml::from_slice(&std::fs::read(dir.join(".security.yml")).unwrap()).unwrap();
        assert_eq!(
            secrets["model_list"]["user-model:0"]["api_keys"][0].as_str(),
            Some("keep-me")
        );
        assert!(
            secrets["model_list"]
                .get("ropero-llama-server-groot-local:0")
                .is_none()
        );
        assert!(!home.join(".config/ropero/connect/picoclaw.json").exists());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn openclaw_y_pi_escriben_sus_archivos_oficiales_y_preservan_campos_ajenos() {
        let home = std::env::temp_dir().join(format!("ropero-agent-json-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let model = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        let engine = parse_engine_manifest(
            include_str!("../../../engines/llama-server.toml"),
            Path::new("engines/llama-server.toml"),
        )
        .unwrap();
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("agent-json-secret".into());
        type Connector = fn(
            &Path,
            &Config,
            &[ropero_core::EngineManifest],
            &[ropero_core::ModelSpec],
            bool,
        ) -> Result<std::path::PathBuf, String>;
        let cases: [(&str, Connector); 2] = [("openclaw", openclaw), ("pi", pi)];
        for (name, connector) in cases {
            let isolated = home.join(name);
            std::fs::create_dir_all(&isolated).unwrap();
            let path = if name == "openclaw" {
                isolated.join(".openclaw/agents/main/agent/models.json")
            } else {
                isolated.join(".pi/agent/models.json")
            };
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, r#"{"keep":"untouched","providers":{}}"#).unwrap();
            let actual = connector(
                &isolated,
                &configured,
                std::slice::from_ref(&engine),
                std::slice::from_ref(&model),
                false,
            )
            .unwrap();
            assert_eq!(actual, path);
            let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(saved["keep"], "untouched");
            assert_eq!(
                saved["providers"]["ropero-llama-server"]["baseUrl"],
                format!("http://127.0.0.1:{}/v1", engine.port_range.min)
            );
            assert_eq!(
                saved["providers"]["ropero-llama-server"]["apiKey"],
                "agent-json-secret"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            connector(
                &isolated,
                &Config::default(),
                std::slice::from_ref(&engine),
                std::slice::from_ref(&model),
                true,
            )
            .unwrap();
            let after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(after["keep"], "untouched");
            assert!(after["providers"].get("ropero-llama-server").is_none());
        }
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn vscode_custom_endpoint_no_persiste_claves_del_engine() {
        let home = std::env::temp_dir().join(format!("ropero-vscode-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let model = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        let engine = parse_engine_manifest(
            include_str!("../../../engines/llama-server.toml"),
            Path::new("engines/llama-server.toml"),
        )
        .unwrap();
        let config_path = home.join(".config/Code/User/chatLanguageModels.json");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &config_path,
            r#"[{"name":"ROPERO (llama-server)","vendor":"customendpoint","apiKey":"legacy-secret","models":[]}]"#,
        )
        .unwrap();
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("vscode-secret".into());
        let path = vscode(
            &home,
            &configured,
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let provider = &saved[0];
        assert_eq!(provider["vendor"], "customendpoint");
        assert_eq!(
            provider["models"][0]["url"],
            format!(
                "http://127.0.0.1:{}/v1/chat/completions",
                engine.port_range.min
            )
        );
        assert!(provider.get("apiKey").is_none());
        assert!(!saved.to_string().contains("vscode-secret"));
        assert!(!saved.to_string().contains("legacy-secret"));
        let _ = std::fs::remove_dir_all(home);
    }

    #[cfg(unix)]
    #[test]
    fn hermes_usa_cli_registra_clave_en_env_y_retirada_sigue_registro_de_propiedad() {
        use std::os::unix::fs::PermissionsExt;
        let home = std::env::temp_dir().join(format!("ropero-hermes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let hermes_home = home.join("hermes");
        let binary = home.join("fake-hermes");
        let log = home.join("calls.log");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            &binary,
            format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n", log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir_all(&hermes_home).unwrap();
        std::fs::write(hermes_home.join(".env"), "KEEP_ME=untouched\n").unwrap();
        let mut model = parse_model(
            include_str!("../../../models/llama-server/groot-o.toml"),
            Path::new("models/llama-server/groot-o.toml"),
        )
        .unwrap();
        model.model.port = Some(ropero_core::Port(9984));
        let engine = parse_engine_manifest(
            include_str!("../../../engines/llama-server.toml"),
            Path::new("engines/llama-server.toml"),
        )
        .unwrap();
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("hermes-secret".into());
        hermes_with_binary(
            &binary,
            &home,
            &hermes_home,
            &configured,
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        let env = std::fs::read_to_string(hermes_home.join(".env")).unwrap();
        assert!(env.contains("KEEP_ME=untouched"));
        assert!(env.contains("ROPERO_HERMES_LLAMA_SERVER_API_KEY=\"hermes-secret\""));
        assert_eq!(
            std::fs::metadata(hermes_home.join(".env"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let calls = std::fs::read_to_string(&log).unwrap();
        for expected in [
            "config set providers.ropero-llama-server.api http://127.0.0.1:9984/v1",
            "config set providers.ropero-llama-server.key_env ROPERO_HERMES_LLAMA_SERVER_API_KEY",
            "config set providers.ropero-llama-server.discover_models false",
            "config set providers.ropero-llama-server.models {\"groot-o\":{}}",
        ] {
            assert!(calls.contains(expected), "falta {expected} en {calls}");
        }
        let marker = home.join(".config/ropero/connect/hermes/llama-server.json");
        assert!(marker.exists());
        hermes_with_binary(
            &binary,
            &home,
            &hermes_home,
            &Config::default(),
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            true,
        )
        .unwrap();
        assert!(!marker.exists());
        let env = std::fs::read_to_string(hermes_home.join(".env")).unwrap();
        assert!(env.contains("KEEP_ME=untouched"));
        assert!(!env.contains("hermes-secret"));
        assert!(
            std::fs::read_to_string(log)
                .unwrap()
                .contains("config unset providers.ropero-llama-server")
        );
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn opencode_registra_el_alias_del_engine_y_no_el_nombre_interno() {
        let home =
            std::env::temp_dir().join(format!("ropero-opencode-alias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let mut model = parse_model(
            include_str!("../../../models/ignis/qwen3.8-27b-16g.toml"),
            Path::new("models/ignis/qwen3.8-27b-16g.toml"),
        )
        .unwrap();
        model.model.alias = Some("qwen3.8-27b".into());
        let engine = parse_engine_manifest(
            include_str!("../../../engines/ignis.toml"),
            Path::new("engines/ignis.toml"),
        )
        .unwrap();
        let state_dir = home.join("state");
        let mut configured = Config::default();
        configured.engines.entry(engine.id).or_default().api_key = Some("opencode-test-key".into());
        let result = opencode(
            &home,
            &state_dir,
            &configured,
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        );
        assert!(result.is_ok(), "{result:?}");
        let config_path = home.join(".config/opencode/opencode.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
        assert!(value["providers"]["ropero-ignis"]["models"]["qwen3.8-27b"].is_object());
        assert!(value["providers"]["ropero-ignis"]["models"]["qwen3.8-27b-16g"].is_null());
        assert_eq!(
            value["providers"]["ropero-ignis"]["settings"]["apiKey"],
            "opencode-test-key"
        );

        opencode(
            &home,
            &state_dir,
            &Config::default(),
            std::slice::from_ref(&engine),
            std::slice::from_ref(&model),
            false,
        )
        .unwrap();
        let reconnected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(
            reconnected["providers"]["ropero-ignis"]["settings"]["apiKey"],
            "opencode-test-key"
        );
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn opencode_consolida_provider_legacy_y_al_desinstalar_deja_el_config_limpio() {
        let home =
            std::env::temp_dir().join(format!("ropero-opencode-clean-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let dir = home.join(".config/opencode");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("opencode.jsonc");
        std::fs::write(
            &config_path,
            r#"{
  // This user's comment must survive connect.
  "provider": {"ropero-ignis": {"name": "This is ROPERO! (ignis)", "models": {"qwen3.8-27b": {"name": "qwen3.8-27b"}}}},
  "providers": {
    "custom": {"name": "User provider"}, // Keep custom provider.
  },
}"#,
        )
        .unwrap();
        let mut model = parse_model(
            include_str!("../../../models/ignis/qwen3.8-27b-16g.toml"),
            Path::new("models/ignis/qwen3.8-27b-16g.toml"),
        )
        .unwrap();
        model.model.alias = Some("qwen3.8-27b".into());
        let engine = parse_engine_manifest(
            include_str!("../../../engines/ignis.toml"),
            Path::new("engines/ignis.toml"),
        )
        .unwrap();
        let state_dir = home.join("state");
        let models = [model];
        assert!(
            opencode(
                &home,
                &state_dir,
                &Config::default(),
                std::slice::from_ref(&engine),
                &models,
                false
            )
            .is_ok()
        );
        let installed: Value = jsonc_parser::parse_to_serde_value(
            &std::fs::read_to_string(&config_path).unwrap(),
            &ParseOptions::default(),
        )
        .unwrap();
        let installed_text = std::fs::read_to_string(&config_path).unwrap();
        assert!(installed_text.contains("This user's comment must survive connect."));
        assert!(installed_text.contains("Keep custom provider."));
        assert!(installed.get("provider").is_none(), "{installed}");
        assert_eq!(installed["providers"]["custom"]["name"], "User provider");
        assert_eq!(
            installed["providers"]["ropero-ignis"]["models"]["qwen3.8-27b"]["name"],
            "qwen3.8-27b"
        );

        assert!(
            opencode(
                &home,
                &state_dir,
                &Config::default(),
                &[engine],
                &models,
                true
            )
            .is_ok()
        );
        let removed: Value = jsonc_parser::parse_to_serde_value(
            &std::fs::read_to_string(&config_path).unwrap(),
            &ParseOptions::default(),
        )
        .unwrap();
        assert!(removed.get("provider").is_none(), "{removed}");
        assert!(
            removed["providers"].get("ropero-ignis").is_none(),
            "{removed}"
        );
        assert_eq!(removed["providers"]["custom"]["name"], "User provider");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn opencode_jsonc_invalido_no_se_modifica() {
        let home =
            std::env::temp_dir().join(format!("ropero-opencode-invalid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let dir = home.join(".config/opencode");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("opencode.jsonc");
        let original = "{ // unfinished\n  \"providers\": {, }";
        std::fs::write(&path, original).unwrap();
        let model = parse_model(
            include_str!("../../../models/ignis/qwen3.8-27b-16g.toml"),
            Path::new("models/ignis/qwen3.8-27b-16g.toml"),
        )
        .unwrap();
        let engine = parse_engine_manifest(
            include_str!("../../../engines/ignis.toml"),
            Path::new("engines/ignis.toml"),
        )
        .unwrap();
        let result = opencode(
            &home,
            &home.join("state"),
            &Config::default(),
            &[engine],
            &[model],
            false,
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_dir_all(home);
    }
}
