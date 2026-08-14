use serde::{Deserialize, Serialize};

pub const ACTIVATION_SCHEMA_VERSION: u16 = 1;
pub const ACTIVATION_PROTOCOL_VERSION: u16 = 1;
pub const RECEIPT_RETENTION_PER_WORKER: usize = 100;
pub const RECEIPT_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationState {
    Active,
    ConfiguredAwaitingProof,
    ActiveIsolated,
    NeedsTrust,
    NeedsProjectMount,
    NeedsPlugin,
    NeedsUserReview,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterCapability {
    NativePreflight,
    WrapperPreflight,
    GuidanceOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub agent_profile: String,
    pub workflow_id: String,
    pub session_id: String,
}

pub mod manifest;

pub use manifest::{
    load_manifest, load_or_create_manifest, prune_receipts, write_receipt, ActivationManifest,
    ActivationReceipt, HarnessActivation,
};
