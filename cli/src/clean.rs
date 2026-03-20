use {
    crate::{error::CliResult, style},
    std::{fs, path::Path, process::Command},
};

pub fn run(all: bool) -> CliResult {
    let dirs = [
        "target/deploy",
        "target/profile",
        "target/idl",
        "target/client",
    ];

    let removed: Vec<&str> = dirs
        .iter()
        .filter(|d| Path::new(d).exists())
        .copied()
        .collect();

    if removed.is_empty() && !all {
        println!("  {}", style::dim("nothing to clean"));
        return Ok(());
    }

    for dir in &removed {
        fs::remove_dir_all(Path::new(dir))?;
    }

    if all {
        let output = Command::new("cargo")
            .arg("clean")
            .output()
            .map_err(anyhow::Error::from)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "  {}",
                style::fail(&format!("cargo clean failed: {}", stderr.trim()))
            );
            std::process::exit(1);
        }
    }

    println!("  {}", style::success("clean"));
    Ok(())
}
