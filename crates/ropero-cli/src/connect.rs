use clap::ArgMatches;

pub fn run(matches: &ArgMatches) -> i32 {
    if !matches.get_flag("list")
        && matches.get_one::<String>("model").is_none()
        && matches.get_one::<String>("agent").is_none()
    {
        ropero_core::out(
            "Conecta modelos del catálogo con agentes de código\n\nUSO: ropero connect --model MODELOS --agent AGENTE [--remove]\n     ropero connect --list\n\nMODELOS admite nombres separados por coma o punto y coma. Los providers se agrupan por engine de Ropero.\n",
            false,
        );
        return 0;
    }
    if matches.get_flag("list") {
        if matches.get_one::<String>("model").is_some()
            || matches.get_one::<String>("agent").is_some()
            || matches.get_flag("remove")
        {
            ropero_core::err_line("error: --list no se combina con --model, --agent ni --remove");
            return 2;
        }
        for (id, note) in ropero_runtime::connect::AGENTS {
            ropero_core::out_line(&format!("{id:<14} {note}"));
        }
        return 0;
    }
    let Some(agent) = matches.get_one::<String>("agent").map(String::as_str) else {
        ropero_core::err_line(
            "error: falta --agent; usa 'ropero connect --list' para ver opciones",
        );
        return 2;
    };
    let Some(requested) = matches.get_one::<String>("model") else {
        ropero_core::err_line("error: falta --model");
        return 2;
    };
    if !matches!(
        agent,
        "claude"
            | "opencode"
            | "openclaw"
            | "claw"
            | "pi"
            | "codex"
            | "hermes"
            | "mcode"
            | "aider"
            | "kilo"
            | "droid"
            | "codebuddy"
            | "copilot"
            | "vscode"
            | "trae"
            | "trae-cn"
    ) {
        ropero_core::err_line(&format!(
            "error: la integración automática con '{agent}' aún no está implementada; consulta 'ropero connect --list'"
        ));
        return 1;
    }
    let Ok(scan) = crate::discovery::scan() else {
        return 1;
    };
    let names = requested
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        if let Some(model) = scan
            .catalog
            .models
            .iter()
            .chain(scan.catalog.fired.iter())
            .find(|m| {
                m.model.name.eq_ignore_ascii_case(name)
                    || m.model
                        .alias
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(name))
            })
        {
            if !selected.iter().any(|m: &&ropero_core::ModelSpec| {
                m.model.name == model.model.name && m.model.engine == model.model.engine
            }) {
                selected.push(model);
            }
        } else {
            missing.push(name.to_string());
        }
    }
    if !missing.is_empty() {
        ropero_core::err_line(&format!(
            "error: no se encontró en el catálogo: {}",
            missing.join(", ")
        ));
        return 1;
    }
    if selected.is_empty() {
        ropero_core::err_line("error: indica al menos un modelo");
        return 2;
    }
    if agent == "codex" && !matches.get_flag("remove") {
        let incompatible = selected
            .iter()
            .filter(|model| {
                !ropero_runtime::connect::codex_responses_supported(model.model.engine.as_str())
            })
            .map(|model| format!("{} ({})", model.model.name, model.model.engine.as_str()))
            .collect::<Vec<_>>();
        if !incompatible.is_empty() {
            ropero_core::err_line(&format!(
                "error: Codex sólo admite providers con Responses API; no se pueden conectar: {}",
                incompatible.join(", ")
            ));
            return 1;
        }
    }
    let state_dir =
        ropero_runtime::state::state_dir(&scan.env.home, scan.env.get("ROPERO_STATE_DIR"));
    let mut owned = selected.into_iter().cloned().collect::<Vec<_>>();
    let mut ports_by_engine =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<u16>>::new();
    for model in &mut owned {
        if let Some(engine) = scan.engines.iter().find(|e| e.id == model.model.engine) {
            let active_port = ropero_runtime::state::managed_ports(&state_dir)
                .into_iter()
                .find_map(|port| {
                    let state = ropero_runtime::state::read_state(&state_dir, port)?;
                    (state.model == model.model.name && state.engine == engine.id.as_str())
                        .then_some(state.port.unwrap_or(port))
                });
            let configured_port = model.model.port.map(|p| p.0);
            let fallback_port = configured_port.unwrap_or(engine.port_range.min);
            if active_port.is_none() && !matches.get_flag("remove") {
                let occupied_by_other = ropero_runtime::state::managed_ports(&state_dir)
                    .into_iter()
                    .find_map(|port| {
                        let state = ropero_runtime::state::read_state(&state_dir, port)?;
                        (state.engine == engine.id.as_str()
                            && state.model != model.model.name
                            && state.port.unwrap_or(port) == fallback_port)
                            .then_some(state.model)
                    });
                if let Some(other_model) = occupied_by_other {
                    ropero_core::err_line(&format!(
                        "error: no se puede conectar '{}' al puerto {fallback_port}: ese endpoint del engine '{}' está sirviendo el modelo '{other_model}'",
                        model.model.name,
                        engine.id.as_str()
                    ));
                    return 1;
                }
            }
            let resolved_port = active_port.unwrap_or(fallback_port);
            model.model.port = Some(ropero_core::Port(resolved_port));
            ports_by_engine
                .entry(engine.id.as_str().to_owned())
                .or_default()
                .insert(resolved_port);
        }
    }
    if let Some((engine, ports)) = ports_by_engine.iter().find(|(_, ports)| ports.len() > 1) {
        ropero_core::err_line(&format!(
            "error: los modelos del engine '{engine}' tienen puertos distintos ({}); un provider sólo puede usar un endpoint",
            ports
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return 1;
    }
    // Los conectores reciben una copia en memoria con la clave efectiva del
    // engine (config, entorno o estado privado). No escribimos secretos en
    // config.toml; cada integración los guarda donde su agente los necesita.
    let mut connect_config = scan.config.clone();
    for model in &owned {
        if let Some(key_state) = ropero_runtime::api_key::connect_key(
            model.model.engine,
            &scan.config,
            &scan.env,
            &state_dir,
        ) {
            connect_config
                .engines
                .entry(model.model.engine)
                .or_default()
                .api_key = key_state;
        }
    }
    if agent == "claude" {
        match ropero_runtime::connect::claude_env_instructions(
            &connect_config,
            &owned,
            matches.get_flag("remove"),
        ) {
            Ok(lines) => {
                for line in lines {
                    ropero_core::out_line(&line);
                }
                return 0;
            }
            Err(error) => {
                ropero_core::err_line(&format!("error: {error}"));
                return 1;
            }
        }
    }
    if agent == "trae" || agent == "trae-cn" {
        for line in ropero_runtime::connect::trae_instructions(
            agent,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        ) {
            ropero_core::out_line(&line);
        }
        return 0;
    }
    let result = if agent == "aider" {
        ropero_runtime::connect::aider(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "opencode" {
        ropero_runtime::connect::opencode(
            &scan.env.home,
            &state_dir,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "openclaw" || agent == "claw" {
        ropero_runtime::connect::openclaw(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "pi" {
        ropero_runtime::connect::pi(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "codex" {
        let codex_home = scan
            .env
            .get("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| scan.env.home.join(".codex"));
        let api_keys = owned
            .iter()
            .filter_map(|model| {
                ropero_runtime::api_key::connect_key(
                    model.model.engine,
                    &connect_config,
                    &scan.env,
                    &state_dir,
                )
                .map(|key| (model.model.engine, key))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        ropero_runtime::connect::codex_in_dir_with_api_keys(
            &scan.env.home,
            &codex_home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
            &api_keys,
        )
    } else if agent == "hermes" {
        let hermes_home = scan
            .env
            .get("HERMES_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| scan.env.home.join(".hermes"));
        ropero_runtime::connect::hermes(
            &scan.env.home,
            &hermes_home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
        .map(|()| hermes_home.join("config.yaml"))
    } else if agent == "kilo" {
        ropero_runtime::connect::kilo(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
        .map(|()| scan.env.home.join(".config/kilo/kilo.json"))
    } else if agent == "droid" {
        ropero_runtime::connect::droid(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "codebuddy" {
        ropero_runtime::connect::codebuddy(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else if agent == "copilot" || agent == "vscode" {
        ropero_runtime::connect::vscode(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
    } else {
        ropero_runtime::connect::mcode(
            &scan.env.home,
            &connect_config,
            &scan.engines,
            &owned,
            matches.get_flag("remove"),
        )
        .map(|()| ropero_runtime::connect::mcode_data_dir(&scan.env.home).join("config.yaml"))
    };
    match result {
        Ok(path) => {
            let target = if agent == "copilot" {
                " (BYOK de VS Code; para Agent Host habilita chat.agentHost.byokModels.enabled)"
            } else {
                ""
            };
            ropero_core::out_line(&format!(
                "{} {}{} en {}",
                if matches.get_flag("remove") {
                    "Conexiones retiradas"
                } else {
                    "Modelos conectados"
                },
                requested,
                target,
                path.display()
            ));
            if agent == "codex" && !matches.get_flag("remove") {
                let engines = owned
                    .iter()
                    .map(|model| model.model.engine.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                for engine in engines {
                    let profile = format!("ropero-{engine}");
                    for model in owned
                        .iter()
                        .filter(|model| model.model.engine.as_str() == engine)
                    {
                        let alias = model.model.alias.as_deref().unwrap_or(&model.model.name);
                        ropero_core::out_line(&format!(
                            "Para usar {alias} en Codex: codex --profile {profile} -m {alias}"
                        ));
                    }
                }
            }
            0
        }
        Err(e) => {
            ropero_core::err_line(&format!("error: {e}"));
            1
        }
    }
}
