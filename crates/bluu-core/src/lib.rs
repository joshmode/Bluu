//! Platform-neutral domain, codec feasibility, diagnostics, privacy, and persistence.
//!
//! The core deliberately does not contain Windows API calls. Platform backends produce
//! [`model::Snapshot`] values using documented APIs, and the UI renders facts that the
//! core has classified. In particular, a preference selected in the UI is never promoted
//! to an observed codec.

pub mod backend;
pub mod codec;
pub mod diagnostics;
pub mod feasibility;
pub mod model;
pub mod privacy;
pub mod provider;
pub mod sdp;
pub mod settings;
pub mod verify;

pub use backend::{
    AudioBackend, BackendCapabilities, BackendDescriptor, BackendError, ConfigOperation,
};
pub use codec::{CodecId, CodecState, Confidence, Evidence, VerificationMethod};
pub use model::*;
pub use provider::{
    ProviderAction, ProviderCodec, ProviderManifest, ProviderOutcome, ProviderParameter,
    ProviderPreference, ProviderRecord, ProviderRequest, ProviderResponse, ProviderStatus,
    ProviderValidationError,
};
pub use verify::Verifier;

pub const APP_NAME: &str = "Bluu";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
