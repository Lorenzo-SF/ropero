#![cfg(unix)]

use ropero_core::{EngineManifest, Environment, parse_engine_manifest, parse_model};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[test]
fn tool_usa_el_pull_del_engine_y_sustituye_placeholders_sin_shell() {
    let base = std::env::temp_dir().join(format!("ropero-tool-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let binary = base.join("fake-pull");
    let args_file = base.join("args.txt");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
            args_file.display()
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&binary, permissions).unwrap();
    let manifest: EngineManifest = parse_engine_manifest(
        "id='vllm'\nlabel='vLLM'\nport_min=1\nport_max=10\n[install]\npull=['fake-pull','pull']\n",
        Path::new("engines/vllm.toml"),
    )
    .unwrap();
    let model = parse_model(
        "[model]\nname='sample'\nengine='vllm'\nformat='gguf'\n[files]\ngguf='sample.gguf'\n[download]\nrepo='owner/repo'\nfile='sample.gguf'\n[tool]\nparams=['--revision','${model}']\n",
        Path::new("models/vllm/sample.toml"),
    ).unwrap();
    let mut vars = BTreeMap::new();
    vars.insert("PATH".into(), base.to_string_lossy().into_owned());
    let env = Environment::new(vars, &base, &base);
    let results = ropero_runtime::tool::pull_model(&manifest, &model, &env).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, 0);
    assert_eq!(
        std::fs::read_to_string(args_file).unwrap(),
        "pull\nowner/repo\n--revision\nsample\n"
    );
    let _ = std::fs::remove_dir_all(base);
}
