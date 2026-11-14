pub fn run(args: &[String]) -> Result<i32, i32> {
    let all = crate::check_all_flag(args, "list", "ropero list [--all]")?;
    let s = crate::discovery::scan()?;
    let rows = ropero_core::list_rows(&s.catalog, all);
    for row in &rows {
        ropero_core::out_line(&ropero_core::list_row_text(&row.spec, row.fired));
    }
    if rows.is_empty() {
        ropero_core::err_line("aviso: el catálogo no tiene modelos que listar");
    }
    Ok(0)
}
