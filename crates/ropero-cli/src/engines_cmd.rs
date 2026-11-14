/// `ropero engines`: la lista de motores con su color, su madurez y si
/// son compatibles con esta máquina.
///
/// El texto lo compone el dominio; aquí sólo se despacha y se imprime.
pub fn run(args: &[String]) -> Result<i32, i32> {
    let all = crate::check_all_flag(args, "engines", "ropero engines [--all]")?;
    let s = crate::discovery::scan()?;
    let sistema = ropero_runtime::capture_system_info();
    let lineas = ropero_core::engine_lines(&s.engines, &sistema, all);
    for linea in &lineas {
        ropero_core::out_line(linea);
    }
    if lineas.is_empty() {
        ropero_core::err_line("aviso: no hay motores que listar");
    }
    Ok(0)
}
