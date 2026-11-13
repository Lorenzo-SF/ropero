use std::fs;
use std::path::PathBuf;

fn collect_rs(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs(&path));
            } else if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn ropero_core_has_no_forbidden_io_or_network() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in collect_rs(&src) {
        let content = fs::read_to_string(&file).expect("read source file");
        for token in ["std::process", "std::fs", "tokio", "reqwest"] {
            assert!(
                !content.contains(token),
                "forbidden token `{token}` found in {}",
                file.display()
            );
        }
    }
}

#[test]
fn ropero_core_does_not_depend_on_workspace_crates() {
    let cargo_toml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).expect("read Cargo.toml");
    let forbidden = [
        "ropero-runtime",
        "ropero-download",
        "ropero-forge",
        "ropero-server",
        "ropero-cli",
        "tokio",
        "reqwest",
    ];
    for token in forbidden {
        assert!(
            !content.contains(token),
            "ropero-core Cargo.toml must not mention `{token}`"
        );
    }
}

#[test]
fn ropero_cli_under_400_lines() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let cli_src = repo_root.join("crates").join("ropero-cli").join("src");
    let mut lines = 0usize;
    for file in collect_rs(&cli_src) {
        let content = fs::read_to_string(&file).expect("read cli source");
        lines += content.lines().count();
    }
    assert!(
        lines < 400,
        "ropero-cli has {lines} lines, expected fewer than 400"
    );
}
