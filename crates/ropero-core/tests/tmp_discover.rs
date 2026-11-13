use std::fs;
use std::path::{Path, PathBuf};

use ropero_core::{ModelSpec, build_catalog, parse_model};

fn collect(dir: &Path, out: &mut Vec<ModelSpec>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    files.sort();
    for path in files {
        let text = fs::read_to_string(&path).expect("read toml");
        out.push(parse_model(&text, &path).expect("parse model"));
    }
}

#[test]
fn tmp() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ropero-code/models");
    let mut active = Vec::new();
    let mut fired = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&base)
        .expect("read models dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        collect(&dir, &mut active);
        collect(&dir.join("fired"), &mut fired);
    }
    let cat = build_catalog(active, fired).expect("build catalog");
    println!("MODELS={}", cat.models.len());
    println!("FIRED={}", cat.fired.len());
}
