use crate::blst::signer::ForkData;
use crate::blst::SignedRoot;
use crate::blst::{public_key::BlsPublicKey, signature::BlsSignature};
use crate::types::Address;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};
use tree_hash_derive::TreeHash;

/// Details of a validator registration.
#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub struct ValidatorRegistration {
    /// The registration message.
    pub message: ValidatorRegistrationMessage,
    /// The signature for the registration.
    pub signature: BlsSignature,
}

/// Represents the message of a validator registration.
#[serde_as]
#[derive(PartialEq, Debug, Serialize, Deserialize, Clone, TreeHash)]
pub struct ValidatorRegistrationMessage {
    /// The fee recipient's address.
    pub fee_recipient: Address,

    /// The gas limit for the registration.
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,

    /// The timestamp of the registration.
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: u64,

    /// The public key of the validator.
    pub pubkey: BlsPublicKey,
}

impl SignedRoot for ValidatorRegistrationMessage {}

impl ValidatorRegistration {
    /// Verify the signature of the registration.
    pub fn verify_signature(&self, fork_data: ForkData) -> bool {
        let signing_root = self
            .message
            .signing_root(fork_data.compute_builder_domain().into());
        self.signature.verify(&self.message.pubkey, signing_root)
    }
}

#[cfg(test)]
mod tests {
    use super::{ValidatorRegistration, ValidatorRegistrationMessage};
    use crate::blst::signer::BlsSigner;
    use crate::types::Address;
    use std::str::FromStr;

    #[test]
    fn verified_sig() {
        let signer = BlsSigner::default();
        let pubkey = signer.public_key();

        let mut message = ValidatorRegistrationMessage {
            fee_recipient: Address::from_str("0x0000000000000000000000000000000000000001").unwrap(),
            gas_limit: 30_000_000,
            timestamp: 1711287496,
            pubkey,
        };

        let signature = signer.sign(&mut message);

        let validator = ValidatorRegistration {
            message,
            signature,
        };

        let fork_data = signer.fork_data_builder().clone();
        assert!(validator.verify_signature(fork_data));
    }
}
