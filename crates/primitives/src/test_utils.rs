//! Test-only constants. Not used on any real chain or account.
//! Use these in tests and examples to avoid linking the codebase to any real identity.

use crate::blst::public_key::BlsPublicKey;
use std::sync::OnceLock;

/// Test Ethereum address (20 bytes). Not a real account.
pub const TEST_ETH_ADDRESS: &str = "0x0000000000000000000000000000000000000001";

/// Test block/parent/state root hash (32 bytes). Not a real chain block.
pub const TEST_BLOCK_HASH: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000001";

static TEST_BLS_PUBKEY_INNER: OnceLock<BlsPublicKey> = OnceLock::new();
static TEST_BLS_PUBKEY_STR: OnceLock<Box<str>> = OnceLock::new();
static TEST_BUILDER_PUBKEY_INNER: OnceLock<BlsPublicKey> = OnceLock::new();
static TEST_BUILDER_PUBKEY_STR: OnceLock<Box<str>> = OnceLock::new();

fn init_test_bls_pubkey() -> &'static BlsPublicKey {
    TEST_BLS_PUBKEY_INNER.get_or_init(BlsPublicKey::random)
}

fn init_test_builder_pubkey() -> &'static BlsPublicKey {
    TEST_BUILDER_PUBKEY_INNER.get_or_init(BlsPublicKey::random)
}

/// Test BLS validator pubkey. Generated at runtime; no fixed identity in repo.
pub fn test_bls_pubkey() -> &'static BlsPublicKey {
    init_test_bls_pubkey()
}

/// Test BLS validator pubkey as hex string. Generated at runtime; no fixed identity in repo.
pub fn test_bls_pubkey_str() -> &'static str {
    TEST_BLS_PUBKEY_STR
        .get_or_init(|| init_test_bls_pubkey().to_string().into_boxed_str())
        .as_ref()
}

/// Test builder pubkey. Generated at runtime; no fixed identity in repo.
pub fn test_builder_pubkey() -> &'static BlsPublicKey {
    init_test_builder_pubkey()
}

/// Test builder pubkey as hex string. Generated at runtime; no fixed identity in repo.
pub fn test_builder_pubkey_str() -> &'static str {
    TEST_BUILDER_PUBKEY_STR
        .get_or_init(|| init_test_builder_pubkey().to_string().into_boxed_str())
        .as_ref()
}

/// Test BLS signature (96 bytes). Test fixture only.
pub const TEST_BLS_SIGNATURE: &str = "0xad92d76a40ecc96a5ab46ae0a7e62c50fc895412be3ae3e0cddad60c54be7aa2c4bbe90b40345e3c2d1a6925aa3a9c6e03f4e30e28b9a315cac4d73de4b8f77a77e0a4f63fa7d6e0a73393cb15cfe596d845eaa146fd52cd9f3b80cde3f2b27a";

/// Test BLS secret key (32 bytes). Test fixture only; never use in production.
pub const TEST_BLS_SECRET_KEY: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000001";

/// extra_data for test payloads: "test" in hex, no branding.
pub const TEST_EXTRA_DATA_HEX: &str = "0x7465737400000000000000000000000000000000000000000000000000000000";
