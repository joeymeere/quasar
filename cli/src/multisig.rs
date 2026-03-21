use {
    ed25519_dalek::SigningKey,
    solana_address::Address,
    solana_signature::Signature,
    solana_signer::{Signer, SignerError},
    std::{fs, path::Path},
};

// ---------------------------------------------------------------------------
// Solana CLI config
// ---------------------------------------------------------------------------

/// Read the Solana CLI config to get RPC URL and keypair path.
/// Falls back to defaults if config is missing.
pub fn solana_rpc_url(url_override: Option<&str>) -> String {
    if let Some(url) = url_override {
        return url.to_string();
    }
    read_config_field("json_rpc_url")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string())
}

pub fn solana_keypair_path(keypair_override: Option<&Path>) -> std::path::PathBuf {
    if let Some(p) = keypair_override {
        return p.to_path_buf();
    }
    read_config_field("keypair_path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".config/solana/id.json")
        })
}

fn read_config_field(field: &str) -> Option<String> {
    let config_path = dirs::home_dir()?.join(".config/solana/cli/config.yml");
    let contents = fs::read_to_string(config_path).ok()?;
    // Simple YAML parsing — find "field: value" line
    contents.lines().find_map(|line| {
        let line = line.trim();
        let prefix = format!("{field}:");
        if line.starts_with(&prefix) {
            Some(line[prefix.len()..].trim().trim_matches('\'').trim_matches('"').to_string())
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Keypair
// ---------------------------------------------------------------------------

/// Thin wrapper around ed25519-dalek SigningKey that implements solana Signer.
pub struct Keypair(pub SigningKey);

impl Keypair {
    /// Read a Solana keypair JSON file (array of 64 bytes).
    pub fn read_from_file(path: &Path) -> Result<Self, crate::error::CliError> {
        let contents = fs::read_to_string(path)?;
        let bytes: Vec<u8> = serde_json::from_str(&contents).map_err(anyhow::Error::from)?;
        if bytes.len() != 64 {
            return Err(anyhow::anyhow!(
                "keypair file must contain exactly 64 bytes, got {}",
                bytes.len()
            )
            .into());
        }
        let secret: [u8; 32] = bytes[..32].try_into().unwrap();
        Ok(Self(SigningKey::from_bytes(&secret)))
    }

    pub fn address(&self) -> Address {
        Address::from(self.0.verifying_key().to_bytes())
    }
}

impl Signer for Keypair {
    fn try_pubkey(&self) -> Result<Address, SignerError> {
        Ok(self.address())
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        use ed25519_dalek::Signer as _;
        Ok(Signature::from(self.0.sign(message).to_bytes()))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}
