use ropero_core::{Environment, build_catalog, parse_model, removal_plan, removal_plan_in_catalog};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn env() -> Environment {
    let vars = BTreeMap::from([
        ("GGUF_DIR".to_string(), "/gguf".to_string()),
        ("SAFETENSORS_DIR".to_string(), "/st".to_string()),
        ("LOG_DIR".to_string(), "/logs".to_string()),
    ]);
    Environment::new(vars, "/home/u".to_string(), PathBuf::from("/cwd"))
}

fn env_con_rutas_engine(config_toml: &str) -> Environment {
    let e = Environment::new(BTreeMap::new(), "/home/u", "/cwd");
    let config = ropero_core::config::parse_config(config_toml, &e).unwrap();
    e.with_config(config)
}

#[test]
fn elimina_usa_rutas_gguf_por_engine_y_canoniza_alias_del_modelo() {
    let spec = parse_model(
        r#"
[model]
name = "demo"
engine = "llama"
context = 4096

[files]
gguf = "weights.gguf"

[download]
repo = "owner/demo"
file = "weights.gguf"

[download.aux.chat_template]
file = "chat_template.jinja"
name = "chat_template.jinja"
use = "run"
"#,
        Path::new("demo.toml"),
    )
    .unwrap();
    let env = env_con_rutas_engine("[engines.llama-server]\ngguf_dir = \"/engines/gguf\"\n");

    let plan = removal_plan(&spec, &env);
    assert_eq!(
        plan.files,
        vec![
            PathBuf::from("/engines/gguf/demo/chat_template.jinja"),
            PathBuf::from("/engines/gguf/demo/weights.gguf"),
        ]
    );
    assert_eq!(
        plan.gguf_model_dir,
        Some(PathBuf::from("/engines/gguf/demo"))
    );
}

#[test]
fn safetensors_por_engine_retiene_ruta_padre_hijo_compartida() {
    let principal = parse_model(
        r#"
[model]
name = "checkpoint-a"
engine = "vllm"
context = 4096
format = "safetensors"
path = "family/checkpoint"
[files]
"#,
        Path::new("a.toml"),
    )
    .unwrap();
    let hijo = parse_model(
        r#"
[model]
name = "checkpoint-b"
engine = "vllm"
context = 4096
format = "safetensors"
path = "family/checkpoint/variant"
[files]
"#,
        Path::new("b.toml"),
    )
    .unwrap();
    let catalog = build_catalog(vec![principal.clone(), hijo], vec![]).unwrap();
    let env =
        env_con_rutas_engine("[engines.vllm]\nsafetensors_dir = \"/engines/vllm/checkpoints\"\n");

    let plan = removal_plan_in_catalog(&principal, &catalog, &env);
    assert!(plan.directories.is_empty());
    assert!(plan.notes.iter().any(|note| {
        note.contains("/engines/vllm/checkpoints/family/checkpoint")
            && note.contains("checkpoint-b")
    }));
}

#[test]
fn gguf_con_auxiliares_runtime_lista_ficheros_atribuibles() {
    let spec = parse_model(
        r#"
[model]
name = "ornith-o"
engine = "llama-server"
context = 65536

[files]
gguf = "Ornith.gguf"

[download]
repo = "r/o"
file = "Ornith.gguf"

[download.aux.chat_template]
repo = "r/o"
file = "chat_template.jinja"
name = "chat_template.jinja"
use = "run"

[download.aux.imatrix]
repo = "r/o"
file = "imatrix.dat"
name = "imatrix.dat"
use = "build"
"#,
        Path::new("ornith-o.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(!plan.is_blocked(), "{:?}", plan.blocked);
    assert!(plan.flm.is_none());
    assert_eq!(
        plan.files,
        vec![
            PathBuf::from("/gguf/ornith-o/Ornith.gguf"),
            PathBuf::from("/gguf/ornith-o/chat_template.jinja"),
        ]
    );
    assert_eq!(plan.gguf_model_dir, Some(PathBuf::from("/gguf/ornith-o")));
}

#[test]
fn flm_deriva_tag_de_decision_cuando_path_vacio() {
    let spec = parse_model(
        r#"
[model]
name = "d3mini"
engine = "fastflowllm"
context = 32768
format = "q4nx"
path = ""

[files]

[args]
base = ["--decision", "d3:mini"]
"#,
        Path::new("d3mini.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(!plan.is_blocked(), "{:?}", plan.blocked);
    let flm = plan.flm.expect("flm");
    assert_eq!(flm.primary_tag, "d3:mini");
    assert_eq!(flm.all_tags, vec!["d3:mini".to_string()]);
    assert!(plan.files.is_empty());
}

#[test]
fn flm_con_path_y_decision_confirma_tags_exactos() {
    let spec = parse_model(
        r#"
[model]
name = "lfm3"
engine = "fastflowllm"
context = 131072
format = "q4nx"
path = "lfm3:1.7b"

[files]

[args]
base = ["--decision", "d3:mini"]
"#,
        Path::new("lfm3.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(!plan.is_blocked(), "{:?}", plan.blocked);
    let flm = plan.flm.expect("flm");
    assert_eq!(flm.primary_tag, "lfm3:1.7b");
    assert_eq!(
        flm.all_tags,
        vec!["lfm3:1.7b".to_string(), "d3:mini".to_string()]
    );
}

#[test]
fn safetensors_planifica_el_directorio_exactamente_declarado() {
    let spec = parse_model(
        r#"
[model]
name = "ckpt"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/modelo"

[files]
"#,
        Path::new("ckpt.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(!plan.is_blocked(), "{:?}", plan.blocked);
    assert!(plan.files.is_empty());
    assert_eq!(
        plan.directories,
        vec![ropero_core::DirectoryArtifact {
            root: PathBuf::from("/st"),
            path: PathBuf::from("/st/familia/modelo"),
        }]
    );
    assert!(plan.flm.is_none());
}

#[test]
fn safetensors_rechaza_path_que_sale_de_su_raiz() {
    let spec = parse_model(
        r#"
[model]
name = "ckpt"
engine = "vllm"
context = 4096
format = "safetensors"
path = "../fuera"

[files]
"#,
        Path::new("ckpt.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(plan.is_blocked());
    assert!(plan.directories.is_empty());
}

#[test]
fn aux_con_ruta_no_segura_bloquea() {
    let spec = parse_model(
        r#"
[model]
name = "m"
engine = "llama-server"
context = 1

[files]
gguf = "m.gguf"

[download]
repo = "r/m"
file = "m.gguf"

[download.aux.mmproj]
file = "mm.gguf"
name = "../fuera.gguf"
use = "run"
"#,
        Path::new("m.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(plan.is_blocked());
    assert!(plan.blocked.iter().any(|m| m.contains("ruta no segura")));
}

#[test]
fn files_absoluto_fuera_del_modelo_bloquea_el_borrado() {
    let spec = parse_model(
        r#"
[model]
name = "m"
engine = "llama-server"
context = 1

[files]
gguf = "/tmp/otro-modelo.gguf"
"#,
        Path::new("m.toml"),
    )
    .expect("parse");

    let plan = removal_plan(&spec, &env());
    assert!(plan.is_blocked());
    assert!(plan.files.is_empty());
    assert!(plan.blocked.iter().any(|message| message.contains("gguf")));
}

#[test]
fn conserva_checkpoint_safetensors_compartido_y_nombra_el_otro_modelo() {
    let principal = parse_model(
        r#"
[model]
name = "ckpt-a"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/checkpoint"
[files]
"#,
        Path::new("ckpt-a.toml"),
    )
    .unwrap();
    let otro = parse_model(
        r#"
[model]
name = "ckpt-b"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/checkpoint"
[files]
"#,
        Path::new("ckpt-b.toml"),
    )
    .unwrap();
    let catalog = build_catalog(vec![principal.clone(), otro], vec![]).unwrap();

    let plan = removal_plan_in_catalog(&principal, &catalog, &env());
    assert!(plan.directories.is_empty());
    assert!(
        plan.notes
            .iter()
            .any(|note| { note.contains("/st/familia/checkpoint") && note.contains("ckpt-b") })
    );
    assert!(plan.is_empty(), "no debe borrar el checkpoint compartido");
}

#[test]
fn safetensors_retiene_checkpoint_padre_hijo_pero_no_hermanos() {
    let principal = parse_model(
        r#"
[model]
name = "padre"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/checkpoint"
[files]
"#,
        Path::new("padre.toml"),
    )
    .unwrap();
    let hijo = parse_model(
        r#"
[model]
name = "hijo"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/checkpoint/variante"
[files]
"#,
        Path::new("hijo.toml"),
    )
    .unwrap();
    let catalog = build_catalog(vec![principal.clone(), hijo], vec![]).unwrap();

    let plan = removal_plan_in_catalog(&principal, &catalog, &env());
    assert!(
        plan.directories.is_empty(),
        "un hijo está dentro del árbol que se borraría"
    );
    assert!(plan.notes.iter().any(|note| note.contains("hijo")));

    let hermano = parse_model(
        r#"
[model]
name = "hermano"
engine = "vllm"
context = 4096
format = "safetensors"
path = "familia/checkpoint-otra"
[files]
"#,
        Path::new("hermano.toml"),
    )
    .unwrap();
    let catalog = build_catalog(vec![principal.clone(), hermano], vec![]).unwrap();
    let plan = removal_plan_in_catalog(&principal, &catalog, &env());
    assert_eq!(
        plan.directories.len(),
        1,
        "los checkpoints hermanos son disjuntos"
    );
    assert_eq!(
        plan.directories[0].path,
        PathBuf::from("/st/familia/checkpoint")
    );
}

#[test]
fn conserva_tag_flm_compartido_y_no_cuenta_aliases_como_duenos_extra() {
    let principal = parse_model(
        r#"
[model]
name = "flm-a"
engine = "fastflowllm"
context = 4096
format = "q4nx"
path = "tag:shared"
[files]
"#,
        Path::new("flm-a.toml"),
    )
    .unwrap();
    let otro = parse_model(
        r#"
[model]
name = "flm-b"
alias = "flm-b-alt"
engine = "fastflowllm"
context = 4096
format = "q4nx"
path = "tag:shared"
[files]
"#,
        Path::new("flm-b.toml"),
    )
    .unwrap();
    let catalog = build_catalog(vec![principal.clone(), otro], vec![]).unwrap();

    let plan = removal_plan_in_catalog(&principal, &catalog, &env());
    assert!(plan.flm.is_none());
    assert!(plan.notes.iter().any(|note| note.contains("flm-b")));
    assert!(plan.is_empty(), "no debe borrar el tag compartido");
}
