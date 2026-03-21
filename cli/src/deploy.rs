use {
    crate::{config::QuasarConfig, error::CliResult, style, utils},
    std::{
        path::PathBuf,
        process::{Command, Stdio},
    },
};

pub fn run(
    program_keypair: Option<PathBuf>,
    upgrade_authority: Option<PathBuf>,
    keypair: Option<PathBuf>,
    url: Option<String>,
    skip_build: bool,
    multisig: Option<String>,
) -> CliResult {
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // Build unless skipped
    if !skip_build {
        crate::build::run(false, false, None)?;
    }

    // Find the .so binary
    let so_path = utils::find_so(&config, false).unwrap_or_else(|| {
        eprintln!(
            "\n  {}",
            style::fail(&format!("no compiled binary found for \"{name}\""))
        );
        eprintln!();
        eprintln!("  Run {} first.", style::bold("quasar build"));
        eprintln!();
        std::process::exit(1);
    });

    if let Some(multisig_addr) = &multisig {
        // Parse multisig address
        let multisig_bytes: [u8; 32] = bs58::decode(multisig_addr)
            .into_vec()
            .map_err(|e| anyhow::anyhow!("invalid multisig address: {e}"))?
            .try_into()
            .map_err(|_| anyhow::anyhow!("multisig address must be 32 bytes"))?;
        let multisig_key = solana_address::Address::from(multisig_bytes);

        // Read program ID from the program keypair (public key = bytes 32..64)
        let prog_keypair_path = program_keypair.unwrap_or_else(|| {
            let default = PathBuf::from("target")
                .join("deploy")
                .join(format!("{}-keypair.json", name));
            if !default.exists() {
                let module = config.module_name();
                let alt = PathBuf::from("target")
                    .join("deploy")
                    .join(format!("{module}-keypair.json"));
                if alt.exists() {
                    return alt;
                }
            }
            default
        });
        let prog_bytes: Vec<u8> =
            serde_json::from_str(&std::fs::read_to_string(&prog_keypair_path)?)
                .map_err(anyhow::Error::from)?;
        if prog_bytes.len() != 64 {
            return Err(anyhow::anyhow!(
                "program keypair must contain exactly 64 bytes, got {}",
                prog_bytes.len()
            )
            .into());
        }
        let program_id =
            solana_address::Address::from(<[u8; 32]>::try_from(&prog_bytes[32..64]).unwrap());

        let payer_path = crate::multisig::solana_keypair_path(keypair.as_deref());
        let rpc_url = crate::multisig::solana_rpc_url(url.as_deref());

        return crate::multisig::propose_upgrade(
            &so_path,
            &program_id,
            &multisig_key,
            &payer_path,
            &rpc_url,
            0, // vault_index
        );
    }

    // Find the program keypair
    let keypair_path = program_keypair.unwrap_or_else(|| {
        let default = PathBuf::from("target")
            .join("deploy")
            .join(format!("{}-keypair.json", name));
        if !default.exists() {
            // Try module name (underscores)
            let module = config.module_name();
            let alt = PathBuf::from("target")
                .join("deploy")
                .join(format!("{module}-keypair.json"));
            if alt.exists() {
                return alt;
            }
        }
        default
    });

    if !keypair_path.exists() {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program keypair not found: {}",
                keypair_path.display()
            ))
        );
        eprintln!();
        eprintln!(
            "  Run {} to generate one, or pass {} explicitly.",
            style::bold("quasar keys new"),
            style::bold("--program-keypair")
        );
        eprintln!();
        std::process::exit(1);
    }

    let sp = style::spinner("Deploying...");

    let mut cmd = Command::new("solana");
    cmd.args([
        "program",
        "deploy",
        so_path.to_str().unwrap_or_default(),
        "--program-id",
        keypair_path.to_str().unwrap_or_default(),
    ]);

    if let Some(authority) = &upgrade_authority {
        cmd.args([
            "--upgrade-authority",
            authority.to_str().unwrap_or_default(),
        ]);
    }

    if let Some(payer) = &keypair {
        cmd.args(["--keypair", payer.to_str().unwrap_or_default()]);
    }

    if let Some(cluster) = &url {
        cmd.args(["--url", cluster]);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);

            // Extract program ID from solana CLI output
            let program_id = stdout
                .lines()
                .find(|l| l.contains("Program Id:"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim())
                .unwrap_or("(unknown)");

            println!(
                "\n  {}",
                style::success(&format!("Deployed to {}", style::bold(program_id)))
            );
            println!();
            Ok(())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let stdout = String::from_utf8_lossy(&o.stdout);
            if !stderr.is_empty() {
                eprintln!();
                for line in stderr.lines() {
                    eprintln!("  {line}");
                }
            }
            if !stdout.is_empty() {
                for line in stdout.lines() {
                    eprintln!("  {line}");
                }
            }
            eprintln!();
            eprintln!("  {}", style::fail("deploy failed"));
            std::process::exit(o.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "\n  {}",
                style::fail(&format!("failed to run solana program deploy: {e}"))
            );
            eprintln!();
            eprintln!(
                "  Make sure the {} CLI is installed and configured.",
                style::bold("solana")
            );
            eprintln!();
            std::process::exit(1);
        }
    }
}
