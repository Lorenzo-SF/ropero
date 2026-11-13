pub fn run(args: &[String]) -> Result<i32, i32> {
    crate::check_no_args(args, "template", "ropero template")?;
    let s = crate::discovery::scan()?;
    let candidates = ropero_core::template_candidates(&s.root, &s.base);
    let Some(path) = candidates.into_iter().find(|p| p.is_file()) else {
        eprintln!("error: no encuentro la plantilla");
        return Err(1);
    };
    match std::fs::read_to_string(&path) {
        Ok(c) => {
            print!("{}", c);
            Ok(0)
        }
        Err(e) => {
            eprintln!("error: {}", e);
            Err(1)
        }
    }
}
