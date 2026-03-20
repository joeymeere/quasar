use {crate::config::QuasarConfig, std::path::PathBuf};

/// Find the compiled .so in target/deploy/ (and optionally target/profile/).
pub fn find_so(config: &QuasarConfig, include_profile: bool) -> Option<PathBuf> {
    let module = config.module_name();
    let name = &config.project.name;

    let mut candidates = vec![
        format!("target/deploy/{name}.so"),
        format!("target/deploy/{module}.so"),
        format!("target/deploy/lib{module}.so"),
    ];

    if include_profile {
        candidates.push(format!("target/profile/{module}.so"));
    }

    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// Convert a snake_case string to PascalCase.
pub fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
