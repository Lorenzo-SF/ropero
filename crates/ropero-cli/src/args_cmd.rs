use crate::usage_error;
use ropero_core::build_argv;
const USO: &str = "ropero args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]...";
pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_usage(args, "args", USO, "")?;
    if let Some(tok) = ropero_core::first_invalid_token(&args[1..]) {
        return Err(usage_error(
            "args",
            &ropero_core::invalid_token_message(tok),
        ));
    }
    let s = crate::discovery::scan()?;
    let spec = crate::spec_for(&s.catalog, &args[0], "args")?;
    let parsed = ropero_core::parse_request(spec, &args[1..]);
    if let Some(prev) = &parsed.repeated_level {
        let now = parsed.level.as_deref().unwrap_or_default();
        eprintln!("{}", ropero_core::repeated_level_notice(prev, now));
    }
    let req = parsed.to_launch_request();
    let ctx = ropero_core::resolve_context(spec, &s.env);
    match build_argv(spec, &req, &ctx) {
        Ok(argv) => {
            for a in argv {
                println!("{}", a);
            }
            Ok(0)
        }
        Err(e) => {
            eprintln!("{}", ropero_core::argv_error_message(&e.to_string()));
            Err(2)
        }
    }
}
