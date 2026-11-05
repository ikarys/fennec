//! `fennec update`: reports the latest release and installs it over this binary.

pub fn run(args: &[String]) -> Result<(), String> {
    let install = match args {
        [] => true,
        [flag] if flag == "--check" => false,
        _ => return Err("usage: fennec update [--check]".into()),
    };
    let check = crate::update::check()?;
    if !check.newer {
        println!(
            "fennec {} is up to date (latest {})",
            check.current, check.latest
        );
        return Ok(());
    }
    println!(
        "fennec {}, latest {}: an update is available",
        check.current, check.latest
    );
    if install {
        crate::update::install(&check.latest, false)?;
        println!("fennec {} installed", check.latest);
    }
    Ok(())
}
