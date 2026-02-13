use crate::blst::SignedRoot;
use crate::blst::{public_key::BlsPublicKey, secret_key::BlsSecretKey, signature::BlsSignature};
use reth::primitives::{hex, NamedChain};
use serde_derive::{Deserialize, Serialize};
use ssz_derive::Decode;
use ssz_derive::Encode;
use std::str::FromStr;
use tracing::log::info;
use tracing::trace;
use tree_hash::{Hash256, TreeHash};
use tree_hash_derive::TreeHash;

/// Helper struct to manage multiple fork data.
/// The builder fork data uses the builder domain while
/// the proposer fork data uses the proposer domain.
#[derive(Debug, Clone)]
pub struct ForkDatas {
    builder: ForkData,
    proposer: ForkData,
}

impl ForkDatas {
    /// Create a new fork data.
    pub fn from_genesis_and_current_version(
        genesis_fork_version: [u8; 4],
        current_version: [u8; 4],
        genesis_validators_root: Hash256,
    ) -> Self {
        let builder = ForkData {
            current_version: genesis_fork_version,
            genesis_validators_root: Hash256::default(),
        };
        let proposer = ForkData {
            current_version,
            genesis_validators_root,
        };
        Self { builder, proposer }
    }
    /// Create a new fork data.
    pub fn new(builder: ForkData, proposer: ForkData) -> Self {
        Self { builder, proposer }
    }
    /// Compute the builder domain to be used for signing.
    pub fn compute_builder_domain(&self) -> Domain {
        self.builder.compute_builder_domain()
    }
    /// Compute the proposer domain to be used for signing.
    pub fn compute_proposer_domain(&self) -> Domain {
        self.proposer.compute_proposer_domain()
    }
}

impl Default for ForkDatas {
    fn default() -> Self {
        let genesis_validators_root =
            Hash256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let current_version = ForkVersion::from_str("0x20000093").unwrap().into();
        Self {
            builder: ForkData::default(),
            proposer: ForkData {
                current_version,
                genesis_validators_root,
            },
        }
    }
}

#[derive(Debug, Clone)]
/// A signer for blst signatures.
pub struct BlsSigner {
    secret_key: BlsSecretKey,
    fork_data: ForkDatas,
}

impl Default for BlsSigner {
    fn default() -> Self {
        Self {
            secret_key: BlsSecretKey::random(),
            fork_data: ForkDatas::default(),
        }
    }
}

impl BlsSigner {
    /// Create a new signer.
    pub fn new(secret_key: BlsSecretKey, fork_data: ForkDatas) -> Self {
        let pubkey = secret_key.public_key();
        info!("Bls Signer configured for {:?}", pubkey.to_string());
        Self {
            secret_key,
            fork_data,
        }
    }
    /// Sign a message used in tests.
    pub fn proposer_sign<T: SignedRoot>(&mut self, message: &mut T) -> BlsSignature {
        let domain = &self.fork_data.compute_proposer_domain();
        let signing_root = message.signing_root(domain.into());
        let sk = &self.secret_key;
        BlsSecretKey::sign(sk, signing_root)
    }
    /// Sign a message used in tests.
    pub fn sign<T: SignedRoot>(&self, message: &mut T) -> BlsSignature {
        let domain = &self.fork_data.compute_builder_domain();
        trace!("Signing domain: {:?}", hex::encode(domain));
        let signing_root = message.signing_root(domain.into());
        let sk = &self.secret_key;
        BlsSecretKey::sign(sk, signing_root)
    }
    /// Returns the public key for the signer.
    pub fn public_key(&self) -> BlsPublicKey {
        self.secret_key.public_key()
    }

    /// Returns the fork data for the signer.
    pub fn fork_data_builder(&self) -> &ForkData {
        &self.fork_data.builder
    }
    /// Returns the fork data for the signer.
    pub fn fork_data_proposer(&self) -> &ForkData {
        &self.fork_data.proposer
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Encode, Decode, TreeHash)]
/// Helper struct for signing data with blst algo.
pub struct ForkData {
    #[serde(with = "serde_utils::bytes_4_hex")]
    /// The current version of the fork.
    pub current_version: [u8; 4],
    /// The root of the genesis validators.
    pub genesis_validators_root: Hash256,
}

impl ForkData {
    /// Compute the builder domain to be used for signing.
    pub fn compute_builder_domain(&self) -> Domain {
        let fork_data_root = self.tree_hash_root();
        let mut domain = Domain::default();
        domain[..4].copy_from_slice(BuilderDomain::default().as_ref());
        domain[4..].copy_from_slice(&fork_data_root.as_ref()[..28]);
        domain
    }

    /// Compute the proposer domain to be used for signing.
    pub fn compute_proposer_domain(&self) -> Domain {
        let fork_data_root = self.tree_hash_root();
        let mut domain = Domain::default();
        domain[..4].copy_from_slice(&ProposerDomain::default().0);
        domain[4..].copy_from_slice(&fork_data_root.as_ref()[..28]);
        domain
    }
}

type Domain = [u8; 32];

/// The signing domain of the beacon proposer
#[derive(Debug, Clone, Default)]
pub struct ProposerDomain([u8; 4]);

/// The domain of the builder.
#[derive(Debug, Clone)]
pub struct BuilderDomain([u8; 4]);

impl AsRef<[u8]> for BuilderDomain {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Default for BuilderDomain {
    fn default() -> Self {
        BuilderDomain([0, 0, 0, 1])
    }
}

impl From<BuilderDomain> for [u8; 4] {
    fn from(val: BuilderDomain) -> Self {
        val.0
    }
}

#[derive(Debug, Clone, Default)]
/// The version of the fork.
pub struct ForkVersion([u8; 4]);

impl AsRef<[u8]> for ForkVersion {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl ForkVersion {
    /// Create a new fork version.
    pub fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Get the genesis fork version based on the named chain.
    pub fn try_from_chain(chain: NamedChain) -> Result<Self, String> {
        let hex_version = match chain {
            NamedChain::Mainnet => "0x00000000",
            NamedChain::Goerli => "0x00001020",
            NamedChain::Holesky => "0x01017000",
            NamedChain::Sepolia => "0x90000069",
            NamedChain::Dev => "0x20000089",
            // Add cases for other chains if needed
            _ => return Err(format!("Unsupported chain: {:?}", chain)),
        };

        Self::from_str(hex_version)
    }
}

impl FromStr for ForkVersion {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start_matches("0x");
        let bytes = hex::decode(s).map_err(|e| format!("Invalid fork version: {}", e))?;

        if bytes.len() != 4 {
            return Err("Invalid fork version length".to_string());
        }

        let arr = bytes.try_into().expect("slice with incorrect length");
        Ok(ForkVersion::new(arr))
    }
}

impl From<ForkVersion> for [u8; 4] {
    fn from(val: ForkVersion) -> Self {
        val.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_compute_b_domain() {
        let signer = BlsSigner::default();
        let domain = signer.fork_data.compute_builder_domain();
        let to_string = hex::encode(domain);
        let expected = "00000001f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a9";
        assert_eq!(to_string, expected);
        let devnet_fork_version =
            "00000001e4b3c03845c9cd15780441feed2a67053b9cc4ae1547af5747433120";
        let signer = BlsSigner::new(
            BlsSecretKey::random(),
            ForkDatas::new(
                ForkData {
                    current_version: ForkVersion([0x20, 0x00, 0x00, 0x89]).into(),
                    genesis_validators_root: Hash256::default(),
                },
                ForkData::default(),
            ),
        );
        let domain = signer.fork_data.compute_builder_domain();
        let to_string = hex::encode(domain);
        assert_eq!(to_string, devnet_fork_version);
    }

    #[derive(Default, Debug, Serialize, Deserialize, Encode, Decode, TreeHash)]
    struct SomethingElse {
        inner: u64,
    }

    impl SignedRoot for SomethingElse {}
    #[test]
    fn can_sign() {
        let sk = BlsSecretKey::random();
        let pk = &sk.public_key();

        let signer = BlsSigner::new(sk, ForkDatas::default());
        let domain = signer.fork_data.compute_builder_domain();

        let mut to_sign = SomethingElse::default();
        let sig = &signer.sign(&mut to_sign);

        let is_verified = sig.verify(pk, to_sign.signing_root(domain.into()));

        assert!(is_verified);
    }
}
