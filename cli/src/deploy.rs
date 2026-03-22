use {
    crate::{config::QuasarConfig, error::CliResult, style, utils},
    std::path::PathBuf,
};

/// Resolve the program keypair path, falling back to target/deploy/<name>-keypair.json.
fn resolve_program_keypair(config: &QuasarConfig, program_keypair: Option<PathBuf>) -> PathBuf {
    program_keypair.unwrap_or_else(|| {
        let name = &config.project.name;
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
    })
}

/// Parse and validate a base58 multisig address.
fn parse_multisig_address(addr: &str) -> Result<solana_address::Address, crate::error::CliError> {
    let bytes: [u8; 32] = bs58::decode(addr)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("invalid multisig address: {e}"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("multisig address must be 32 bytes"))?;
    Ok(solana_address::Address::from(bytes))
}

/// Build unless skipped, then locate the compiled .so binary.
fn build_and_find_so(
    config: &QuasarConfig,
    name: &str,
    skip_build: bool,
) -> Result<PathBuf, crate::error::CliError> {
    if !skip_build {
        crate::build::run(false, false, None)?;
    }
    utils::find_so(config, false).ok_or_else(|| {
        anyhow::anyhow!(
            "no compiled binary found for \"{name}\". Run `quasar build` first."
        )
        .into()
    })
}

pub struct DeployOpts {
    pub program_keypair: Option<PathBuf>,
    pub upgrade_authority: Option<PathBuf>,
    pub keypair: Option<PathBuf>,
    pub url: Option<String>,
    pub skip_build: bool,
    pub multisig: Option<String>,
    pub status: bool,
    pub upgrade: bool,
    pub priority_fee: Option<u64>,
}

pub fn run(opts: DeployOpts) -> CliResult {
    let DeployOpts {
        program_keypair,
        upgrade_authority,
        keypair,
        url,
        skip_build,
        multisig,
        status,
        upgrade,
        priority_fee,
    } = opts;
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // Resolve cluster URL once
    let rpc_url = crate::rpc::solana_rpc_url(url.as_deref());

    // Resolve priority fee: use override or auto-calculate
    let fee = match priority_fee {
        Some(f) => f,
        None => {
            let auto = crate::rpc::get_recent_prioritization_fees(&rpc_url).unwrap_or(0);
            if auto > 0 {
                println!(
                    "  {} Auto priority fee: {} micro-lamports",
                    style::dim("i"),
                    auto
                );
            }
            auto
        }
    };

    // --upgrade --multisig: Squads proposal flow
    if upgrade {
        if let Some(multisig_addr) = &multisig {
            let multisig_key = parse_multisig_address(multisig_addr)?;
            let payer_path = crate::rpc::solana_keypair_path(keypair.as_deref());

            if status {
                return crate::multisig::show_proposal_status(
                    &multisig_key,
                    &payer_path,
                    &rpc_url,
                    fee,
                );
            }

            let so_path = build_and_find_so(&config, name, skip_build)?;
            let prog_keypair_path = resolve_program_keypair(&config, program_keypair);
            let program_id =
                crate::rpc::read_program_id_from_keypair(&prog_keypair_path)?;

            return crate::multisig::propose_upgrade(
                &so_path,
                &program_id,
                &multisig_key,
                &payer_path,
                &rpc_url,
                0,
                fee,
            );
        }
    }

    // Everything below needs a build and a .so
    let so_path = build_and_find_so(&config, name, skip_build)?;
    let keypair_path = resolve_program_keypair(&config, program_keypair);

    if !keypair_path.exists() {
        return Err(anyhow::anyhow!(
            "program keypair not found: {}. Run `quasar keys new` to generate one, or pass `--program-keypair` explicitly.",
            keypair_path.display()
        )
        .into());
    }

    // Read program ID from the keypair for on-chain check
    let program_id = crate::rpc::read_program_id_from_keypair(&keypair_path)?;
    let exists = crate::rpc::program_exists_on_chain(&rpc_url, &program_id)?;

    // Forward check: deploy on existing program
    if !upgrade && exists {
        return Err(anyhow::anyhow!(
            "program already deployed at {}. Use `quasar deploy --upgrade` to upgrade an existing program.",
            bs58::encode(program_id).into_string()
        )
        .into());
    }

    // Reverse check: --upgrade on non-existent program
    if upgrade && !exists {
        return Err(anyhow::anyhow!(
            "program not found at {}. Drop `--upgrade` for a fresh deploy.",
            bs58::encode(program_id).into_string()
        )
        .into());
    }

    // Load the payer keypair
    let payer_path = crate::rpc::solana_keypair_path(keypair.as_deref());
    let payer = crate::rpc::Keypair::read_from_file(&payer_path)?;

    if upgrade {
        // Authority validation before buffer upload
        let authority_keypair = if let Some(ref auth_path) = upgrade_authority {
            crate::rpc::Keypair::read_from_file(auth_path)?
        } else {
            crate::rpc::Keypair::read_from_file(&payer_path)?
        };

        let sp = style::spinner("Verifying upgrade authority...");
        crate::bpf_loader::verify_upgrade_authority(
            &rpc_url,
            &program_id,
            &authority_keypair.address(),
        )?;
        sp.finish_and_clear();

        // Upgrade
        let sp = style::spinner("Uploading and upgrading...");
        crate::bpf_loader::upgrade_program(
            &so_path,
            &program_id,
            &authority_keypair,
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "\n  {}",
            style::success(&format!(
                "Upgraded {}",
                style::bold(&bs58::encode(program_id).into_string())
            ))
        );
    } else {
        // Fresh deploy
        let program_kp = crate::rpc::Keypair::read_from_file(&keypair_path)?;

        let sp = style::spinner("Deploying...");
        let addr = crate::bpf_loader::deploy_program(
            &so_path,
            &program_kp,
            &payer,
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "\n  {}",
            style::success(&format!(
                "Deployed to {}",
                style::bold(&bs58::encode(addr).into_string())
            ))
        );
    }

    // --multisig without --upgrade: transfer authority to vault after deploy
    if let Some(multisig_addr) = &multisig {
        let multisig_key = parse_multisig_address(multisig_addr)?;
        let (vault, _) = crate::multisig::vault_pda(&multisig_key, 0);

        let authority_keypair = if let Some(ref auth_path) = upgrade_authority {
            crate::rpc::Keypair::read_from_file(auth_path)?
        } else {
            crate::rpc::Keypair::read_from_file(&payer_path)?
        };

        let sp = style::spinner("Transferring upgrade authority to multisig vault...");
        crate::bpf_loader::set_authority(
            &crate::bpf_loader::programdata_pda(&program_id).0,
            &authority_keypair,
            Some(&vault),
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "  {}",
            style::success(&format!(
                "Upgrade authority transferred to vault {}",
                style::bold(&crate::multisig::short_addr(&vault))
            ))
        );
        println!();
        println!(
            "  Future upgrades: {}",
            style::dim(&format!(
                "quasar deploy --upgrade --multisig {multisig_addr}"
            ))
        );
    }

    println!();
    Ok(())
}
