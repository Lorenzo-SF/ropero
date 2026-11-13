const USO: &str = "ropero path <modelo>";
const TOO_MANY: &str = "path no acepta más argumentos (recibido: '{}')";
pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_usage(args, "path", USO, TOO_MANY)?;
    let s = crate::discovery::scan()?;
    let spec = crate::spec_for(&s.catalog, &args[0], "path")?;
    let paths = ropero_core::resolve_paths(spec);
    show("GGUF", &paths.gguf);
    show("MTP", &paths.mtp);
    show("MMPROJ", &paths.mmproj);
    show("CHAT_TEMPLATE", &paths.chat_template);
    show("MODEL_PATH", &paths.model_path);
    println!("LOG={}", paths.log);
    Ok(0)
}
fn show(label: &str, value: &Option<String>) {
    if let Some(v) = value {
        println!("{}={}", label, v);
    }
}
