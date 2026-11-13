use crate::usage_error;
use ropero_core::ModelSpec;
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
    let rows = ropero_core::catalog_rows(&s.catalog, all);
    for (spec, fired) in &rows {
        row(spec, *fired);
    }
    if rows.is_empty() {
        eprintln!("aviso: el catálogo no tiene modelos que listar");
    }
    Ok(0)
}
fn row(spec: &ModelSpec, fired: bool) {
    let mut extra = String::new();
    if let Some(a) = &spec.model.alias {
        extra.push_str(&format!("alias={} ", a));
    }
    if fired {
        extra.push_str("retirado ");
    }
    println!(
        "  {:<16} engine={:<13} format={:<12} ctx={:<7} {}{}",
        spec.model.name,
        spec.model.engine.as_str(),
        spec.model.format.as_str(),
        spec.model.context,
        extra,
        ropero_core::model_description(spec)
    );
}
