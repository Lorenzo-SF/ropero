pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_no_args(args, "validate", "ropero validate")?;
    let s = crate::discovery::scan()?;
    if s.problems.is_empty() {
        println!(
            "OK: {} modelo(s) validados en {}",
            s.files,
            s.base.display()
        );
        return Ok(0);
    }
    for p in &s.problems {
        eprintln!("{}", p);
    }
    eprintln!("{} problema(s) en {}", s.problems.len(), s.base.display());
    Err(1)
}
