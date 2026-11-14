const USO: &str = "ropero path <modelo>";
const TOO_MANY: &str = "path no acepta más argumentos (recibido: '{}')";
pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_usage(args, "path", USO, TOO_MANY)?;
    let s = crate::discovery::scan()?;
    let spec = crate::spec_for(&s.catalog, &args[0], "path")?;
    let paths = ropero_core::resolve_paths(spec, &s.env);
    for line in ropero_core::path_lines(&paths) {
        ropero_core::out_line(&line);
    }
    Ok(0)
}
