use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// The name of a fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub enum ForkName {
    /// The phase0 fork.
    Base,
    /// The altair fork.
    Altair,
    /// The bellatrix fork.
    Merge,
    /// The capella fork.
    Capella,
    /// The deneb fork.
    Deneb,
}

impl FromStr for ForkName {
    type Err = String;

    fn from_str(fork_name: &str) -> Result<Self, String> {
        Ok(match fork_name.to_lowercase().as_ref() {
            "phase0" | "base" => ForkName::Base,
            "altair" => ForkName::Altair,
            "bellatrix" | "merge" => ForkName::Merge,
            "capella" => ForkName::Capella,
            "deneb" => ForkName::Deneb,
            _ => return Err(format!("unknown fork name: {}", fork_name)),
        })
    }
}

impl TryFrom<String> for ForkName {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

impl Display for ForkName {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            ForkName::Base => "phase0".fmt(f),
            ForkName::Altair => "altair".fmt(f),
            ForkName::Merge => "bellatrix".fmt(f),
            ForkName::Capella => "capella".fmt(f),
            ForkName::Deneb => "deneb".fmt(f),
        }
    }
}

impl From<ForkName> for String {
    fn from(fork: ForkName) -> String {
        fork.to_string()
    }
}

#[derive(Deserialize, Debug)]
#[allow(missing_docs)]
pub struct GetForkResponse {
    pub data: ForkDataResp,
}

#[derive(Deserialize, Debug)]
#[allow(missing_docs)]
pub struct ForkDataResp {
    pub genesis_time: String,
    pub genesis_validators_root: String,
    pub fork: CurrentFork,
}

#[derive(Deserialize, Debug)]
#[allow(missing_docs)]
pub struct CurrentFork {
    pub current_version: String,
}
