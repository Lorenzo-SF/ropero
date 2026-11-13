use crate::usage_error;
pub fn run(args: &[String]) -> Result<i32, i32> {
    let mut all = false;
    for a in args {
        match a.as_str() {
            "--help" | "-h" => {
                println!("USO: ropero list [--all]");
                return Err(0);
            }
            "--all" => all = true,
            _ => return Err(usage_error("list", &format!("list no acepta '{}'", a))),
        }
    }
    let s = crate::discovery::scan()?;
    let rows = ropero_core::list_rows(&s.catalog, all);
    for row in &rows {
        println!("{}", ropero_core::list_row_text(&row.spec, row.fired));
    }
    if rows.is_empty() {
        eprintln!("aviso: el catálogo no tiene modelos que listar");
    }
    Ok(0)
}
