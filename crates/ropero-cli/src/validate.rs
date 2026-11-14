use ropero_core::validate_catalog;

pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_no_args(args, "validate", "ropero validate")?;
    let scan = crate::discovery::scan()?;
    let errors = validate_catalog(&scan.catalog, &scan.texts);
    let mut exit = 0;
    for error in &errors {
        eprintln!("{}:{}", error.path(), error.line().unwrap_or(0));
        eprintln!("  {}", error.message());
        exit = exit.max(error.exit_code());
    }
    for problem in &scan.problems {
        eprintln!("{}", problem);
        exit = exit.max(1);
    }
    if exit == 0 {
        println!("{}", ropero_core::validate_ok_line(scan.files, &scan.base));
    } else {
        eprintln!("{}", ropero_core::validate_failed_line(&scan.base));
    }
    if exit == 0 { Ok(0) } else { Err(exit) }
}
