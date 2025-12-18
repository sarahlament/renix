use color_eyre::{eyre::Context, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::process::Command;

/// Discover NixOS configurations from a flake
/// Returns a set of configuration names found under nixosConfigurations
pub fn discover_configurations(flake_path: &str) -> Result<HashSet<String>> {
    // Run nix flake show --json
    let output = Command::new("nix")
        .args(["flake", "show", "--json", flake_path])
        .output()
        .wrap_err("Failed to execute nix flake show")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!("nix flake show failed: {}", stderr));
    }

    let stdout =
        String::from_utf8(output.stdout).wrap_err("nix flake show output was not valid UTF-8")?;

    let json: Value =
        serde_json::from_str(&stdout).wrap_err("Failed to parse nix flake show JSON output")?;

    // Navigate to nixosConfigurations
    let mut configs = HashSet::new();

    if let Some(nixos_configs) = json.get("nixosConfigurations") {
        if let Some(obj) = nixos_configs.as_object() {
            for key in obj.keys() {
                configs.insert(key.clone());
            }
        }
    }

    Ok(configs)
}

/// Get the current hostname
pub fn get_hostname() -> Result<String> {
    let output = Command::new("hostname")
        .output()
        .wrap_err("Failed to execute hostname command")?;

    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!("hostname command failed"));
    }

    let hostname = String::from_utf8(output.stdout)
        .wrap_err("hostname output was not valid UTF-8")?
        .trim()
        .to_string();

    Ok(hostname)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flake_json() {
        let json = r#"{
            "nixosConfigurations": {
                "athena": {},
                "remote-server": {}
            }
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let mut configs = HashSet::new();

        if let Some(nixos_configs) = value.get("nixosConfigurations") {
            if let Some(obj) = nixos_configs.as_object() {
                for key in obj.keys() {
                    configs.insert(key.clone());
                }
            }
        }

        assert!(configs.contains("athena"));
        assert!(configs.contains("remote-server"));
        assert_eq!(configs.len(), 2);
    }
}
