use ropero_core::ModelFormat;

#[test]
fn variants_parse() {
    // serde rename check via toml? just variants exist
    let _ = ModelFormat::Gguf;
    let _ = ModelFormat::Safetensors;
    let _ = ModelFormat::Q4nx;
    let _ = ModelFormat::Strata;
}

#[test]
fn artifact_locations() {
    assert_eq!(
        ModelFormat::Gguf.artifact_path("m", Some("a.gguf"), ""),
        "$GGUF_DIR/m/a.gguf"
    );
    assert_eq!(
        ModelFormat::Safetensors.artifact_path("m", None, "org/model"),
        "$SAFETENSORS_DIR/org/model"
    );
    assert_eq!(
        ModelFormat::Q4nx.artifact_path("m", None, "some-tag"),
        "some-tag"
    );
    assert_eq!(
        ModelFormat::Strata.artifact_path("m", None, "fam/pkg"),
        "fam/pkg"
    );
}

#[test]
fn default_is_gguf() {
    assert_eq!(ModelFormat::default(), ModelFormat::Gguf);
}
