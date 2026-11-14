const USO: &str = "ropero levels <modelo>";
const TOO_MANY: &str = "levels no acepta '{}'";
pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_usage(args, "levels", USO, TOO_MANY)?;
    let s = crate::discovery::scan()?;
    let spec = crate::spec_for(&s.catalog, &args[0], "levels")?;
    for line in ropero_core::level_lines(spec) {
        println!("{}", line);
    }
    Ok(0)
}
