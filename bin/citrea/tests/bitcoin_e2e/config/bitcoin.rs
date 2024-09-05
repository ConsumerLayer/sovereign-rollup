use std::path::PathBuf;

use bitcoin::Network;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinConfig {
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub rpc_user: String,
    pub rpc_password: String,
    pub data_dir: PathBuf,
    pub extra_args: Vec<String>,
    pub network: Network,
    pub docker_image: Option<String>,
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            p2p_port: 0,
            rpc_port: 0,
            rpc_user: "user".to_string(),
            rpc_password: "password".to_string(),
            data_dir: TempDir::new()
                .expect("Failed to create temporary directory")
                .into_path(),
            extra_args: vec![],
            network: Network::Regtest,
            docker_image: Some("bitcoin/bitcoin:latest".to_string()),
        }
    }
}

impl BitcoinConfig {
    fn base_args(&self) -> Vec<String> {
        vec![
            "-regtest".to_string(),
            format!("-datadir={}", self.data_dir.display()),
            format!("-port={}", self.p2p_port),
            format!("-rpcport={}", self.rpc_port),
            format!("-rpcuser={}", self.rpc_user),
            format!("-rpcpassword={}", self.rpc_password),
            "-server".to_string(),
            "-daemon".to_string(),
            "-txindex".to_string(),
            "-addresstype=bech32m".to_string(),
        ]
    }

    pub fn args(&self) -> Vec<String> {
        [self.base_args(), self.extra_args.clone()].concat()
    }
}