use crate::usage_error;
use ropero_core::{LaunchRequest, build_argv};
const USO: &str = "ropero args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]...";
pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_usage(args, "args", USO, "")?;
    for tok in &args[1..] {
        if !tok.starts_with("--") {
            let msg = format!(
                "se esperaba --<nivel>|--<flag>|--<opt>=<v> (recibido: '{}')",
                tok
            );
            return Err(usage_error("args", &msg));
        }
    }
    let s = crate::discovery::scan()?;
    let spec = crate::spec_for(&s.catalog, &args[0], "args")?;
    let parsed = ropero_core::parse_request(spec, &args[1..]);
    if let Some(prev) = &parsed.repeated_level {
        let now = parsed.level.as_deref().unwrap_or_default();
        eprintln!(
            "aviso: nivel repetido: '{}' → '{}' (gana el último)",
            prev, now
        );
    }
    let req = LaunchRequest {
        level: parsed.level,
        flags: parsed.flags,
        options: parsed.options,
    };
    let ctx = ropero_core::resolve_context(spec);
    match build_argv(spec, &req, &ctx) {
        Ok(argv) => {
            for a in argv {
                println!("{}", a);
            }
            Ok(0)
        }
        Err(e) => {
            eprintln!("error: {}", e);
            eprintln!("  (ropero args --help → uso de args)");
            Err(2)
        }
    }
}
