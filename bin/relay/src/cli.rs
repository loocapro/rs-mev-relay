use crate::StartUpErrors;
use clap::Parser;
use relay_primitives::{
    blst::{
        public_key::BlsPublicKey,
        secret_key::BlsSecretKey,
        signer::{ForkData, ForkDatas, ForkVersion},
    },
    constants::EPOCH_SLOTS,
    hex, NamedChain,
};
use std::net::SocketAddr;
use tree_hash::{Hash256, HASHSIZE};
use url::Url;

const DEFAULT_GRPC_PORT: &str = "50051";
const DEFAULT_HTTP_PORT: &str = "9063";
const DEFAULT_METRICS_PORT: &str = "9090";

const DEFAULT_CHAIN: &str = "mainnet";

// MAINNET FORK DATA
const DEFAULT_FORK_VERSION: &str = "0x00000000";
const DEFAULT_CURRENT_VERSION: &str = "0x20000093";
// Mainnet genesis validators root (chain constant, not account-linked)
const DEFAULT_VALIDATORS_ROOT: &str =
    "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95";

/// Placeholder; set --auth.url to your auth provider's endpoint for proposer API validation.
const DEFAULT_AUTH_URL: &str = "https://auth.example.com";
const DEFAULT_BEACON_URL: &str = "http://127.0.0.1:3500";

#[derive(Debug, Parser, Clone)]
#[command(author, about = "MEV Relay", long_about = None)]
pub(crate) struct CliArgs {
    /// Proposer auth API base URL (validates api_key for proposer endpoints).
    #[clap(long = "auth.url", default_value = DEFAULT_AUTH_URL)]
    pub(crate) mbs_auth_url: Url,
    /// The beacon node base url.
    #[clap(long = "beacon.url", default_value = DEFAULT_BEACON_URL)]
    pub(crate) beacon_url: Url,
    /// The gRPC server port.
    /// The gRPC server is used to expose a POST /eth/v1/builder/blocks
    /// and accept bids via gRPC.
    #[clap(long = "grpc.port", default_value = DEFAULT_GRPC_PORT)]
    pub(crate) grpc_port: u16,
    /// The http server port.
    /// It exposes:
    /// GET /relay/v1/builder/validators
    /// POST /eth/v1/builder/blocks
    /// GET /relay/v1/data/bidtraces/builder_blocks_received
    /// POST /eth/v1/builder/blinded_blocks
    /// POST /eth/v1/builder/header/{slot}/{parent_hash}/{pubkey}
    /// POST /eth/v1/builder/validators
    /// Following https://flashbots.github.io/relay-specs
    #[clap(long = "http.port", default_value = DEFAULT_HTTP_PORT)]
    pub(crate) http_port: u16,
    /// The http server port.
    #[clap(long = "metrics.port", default_value = DEFAULT_METRICS_PORT)]
    pub(crate) metrics_port: u16,
    /// Slots per epoch.
    #[clap(long = "epoch.slots", default_value_t = EPOCH_SLOTS)]
    pub(crate) slots_per_epoch: u64,
    /// Comma separated builder BLS pubkeys, enabled to submit bids.
    #[clap(long = "builders.enabled", value_delimiter = ',')]
    pub(crate) white_listed_builders: Vec<relay_primitives::events::BlsPublicKey>,
    /// The fork version is a a 4-byte array representing the active fork version.
    /// It used to compute the domain for signing and verifying signatures.
    /// So that every fork has a unique domain and uses a unique set of keys.
    /// In the case of mainnet the fork version is 0x00000000.
    #[clap(long = "fork-data.genesis-version",value_parser = parse_fork_version, default_value = DEFAULT_FORK_VERSION)]
    pub(crate) genesis_fork_version: ForkVersion,
    /// The current version is a a 4-byte array representing the current fork version.
    /// It used to compute the domain for signing and verifying signatures with the Proposer domain.
    #[clap(long = "fork-data.current-version",value_parser = parse_fork_version, default_value = DEFAULT_CURRENT_VERSION)]
    pub(crate) current_fork_version: ForkVersion,
    /// The genesis validators root used to verify signatures and sign.
    #[clap(long = "fork-data.genesis-validators-root", value_parser = parse_genesis_root, default_value = DEFAULT_VALIDATORS_ROOT)]
    pub(crate) genesis_validators_root: Hash256,
    /// The chain the relay is running on.
    #[clap(long = "chain", default_value = DEFAULT_CHAIN)]
    pub(crate) chain: NamedChain,
    /// BLS secret key to sign headers
    #[arg(
        long = "bls-secret-key",
        value_parser = parse_bls_secret_key,
    )]
    pub(crate) bls_secret_key: BlsSecretKey,
}

/// Helper to parse a [BlsSecretKey] from string
fn parse_bls_secret_key(s: &str) -> Result<BlsSecretKey, String> {
    s.parse::<BlsSecretKey>()
        .map_err(|e| format!("Invalid BLS secret key: {}", e))
}

impl CliArgs {
    /// Get the gRPC server address.
    pub(crate) fn grpc_addr(&self) -> SocketAddr {
        format!("0.0.0.0:{}", self.grpc_port).parse().unwrap()
    }

    /// Get the metrics server port.
    pub(crate) fn metrics_port(&self) -> u16 {
        self.metrics_port
    }

    /// Get the Http server address.
    pub(crate) fn http_addr(&self) -> SocketAddr {
        format!("0.0.0.0:{}", self.http_port).parse().unwrap()
    }

    pub(crate) fn bls_secret_key(&self) -> BlsSecretKey {
        self.bls_secret_key.clone()
    }

    /// Initializes the fork version from a named chain
    /// It will require the debug namespace of a CL to fetch the genesis validators root and the current fork version.
    pub(crate) fn genesis_fork_version(&self) -> Result<ForkVersion, StartUpErrors> {
        let chain = self.chain;
        ForkVersion::try_from_chain(chain).map_err(StartUpErrors::InvalidChain)
    }

    /// Setup the default fork data in case there is no debug namespace of the CL active
    pub(crate) fn fork_data(&self) -> ForkDatas {
        ForkDatas::new(self.fork_data_builder(), self.fork_data_proposer())
    }

    /// Get the fork data, used to sign and verify signatures.
    pub(crate) fn fork_data_builder(&self) -> ForkData {
        ForkData {
            current_version: self.genesis_fork_version.clone().into(),
            genesis_validators_root: Hash256::default(),
        }
    }
    /// Get the fork data, used to sign and verify signatures.
    pub(crate) fn fork_data_proposer(&self) -> ForkData {
        ForkData {
            current_version: self.current_fork_version.clone().into(),
            genesis_validators_root: self.genesis_validators_root,
        }
    }
    /// Get the enabled builders to submit bids.
    pub(crate) fn enabled_builders(&self) -> Vec<BlsPublicKey> {
        self.white_listed_builders
            .iter()
            .map(|b| (*b).into())
            .collect()
    }
}

fn parse_fork_version(s: &str) -> Result<ForkVersion, String> {
    s.parse::<ForkVersion>()
}

fn parse_genesis_root(s: &str) -> Result<Hash256, String> {
    let s = s.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|e| format!("Invalid genesis root: {}", e))?;

    if bytes.len() != HASHSIZE {
        return Err("Invalid genesis root length".to_string());
    }

    Ok(Hash256::from_slice(bytes.as_slice()))
}

#[cfg(test)]
mod tests {
    use crate::cli::{DEFAULT_BEACON_URL, DEFAULT_GRPC_PORT};

    use super::CliArgs;
    use clap::{Args, Parser};
    use relay_primitives::{constants::EPOCH_SLOTS, hex, test_utils};

    /// A helper type to parse Args more easily
    #[derive(Parser)]
    struct CommandParser<T: Args> {
        #[clap(flatten)]
        args: T,
    }
    #[test]
    fn parse_configs() {
        // Testing with no arguments to ensure defaults are correctly applied
        let args = CliArgs::parse_from([
            "test",
            "--bls-secret-key",
            test_utils::TEST_BLS_SECRET_KEY,
        ]);
        assert_eq!(
            args.beacon_url.as_str().trim_end_matches('/'),
            DEFAULT_BEACON_URL
        );
        assert_eq!(args.grpc_port, DEFAULT_GRPC_PORT.parse::<u16>().unwrap());
        assert_eq!(args.slots_per_epoch, EPOCH_SLOTS);
        assert_eq!(args.white_listed_builders.len(), 0);

        let builder1 = test_utils::test_builder_pubkey_str();
        let builder2 = test_utils::test_builder_pubkey_str();

        // Testing each argument individually with a custom value
        let custom_beacon_url = "http://localhost:1234";
        let custom_grpc_port = "50051";
        let custom_slots_per_epoch = "32";

        let custom_builders = format!("{builder1},{builder2}");

        let custom_fork_version = "01010101";
        let custom_genesis_root =
            "364b5565185058cbade2bd5d4633fb13794bcd7b474c76935c4a053571404352";

        let args = CliArgs::parse_from([
            "test",
            "--bls-secret-key",
            test_utils::TEST_BLS_SECRET_KEY,
            "--beacon.url",
            custom_beacon_url,
            "--grpc.port",
            custom_grpc_port,
            "--epoch.slots",
            custom_slots_per_epoch,
            "--builders.enabled",
            &custom_builders.to_string(),
            "--fork-data.current-version",
            custom_fork_version,
            "--fork-data.genesis-validators-root",
            custom_genesis_root,
            "--chain",
            "dev",
        ]);

        assert_eq!(
            args.beacon_url.as_str().trim_end_matches('/'),
            custom_beacon_url
        );
        assert_eq!(args.grpc_port, custom_grpc_port.parse::<u16>().unwrap());
        assert_eq!(
            args.slots_per_epoch,
            custom_slots_per_epoch.parse::<u64>().unwrap()
        );
        assert_eq!(args.white_listed_builders.len(), 2);
        assert_eq!(args.white_listed_builders[0].to_string(), builder1);
        assert_eq!(args.white_listed_builders[1].to_string(), builder2);
        let genesis_root = hex::encode(args.genesis_validators_root);
        assert_eq!(genesis_root, custom_genesis_root);
        let fork_version = hex::encode(args.current_fork_version);
        assert_eq!(fork_version, custom_fork_version);
        assert_eq!(args.chain, relay_primitives::NamedChain::Dev);
    }

    #[test]
    fn parse_bls_key() {
        let key = test_utils::test_builder_pubkey_str();
        let args = CliArgs::parse_from([
            "test",
            "--builders.enabled",
            key,
            "--bls-secret-key",
            test_utils::TEST_BLS_SECRET_KEY,
        ]);
        assert_eq!(args.white_listed_builders.len(), 1);
        assert_eq!(args.white_listed_builders[0].to_string(), key);
        let builders = args.enabled_builders();
        assert_eq!(builders.len(), 1);
    }
}
