//! Pure transition rules for manual/independent verification.
//!
//! Windows' A2DP ETW event gives codec IDs but no device identifier and no
//! parameters. A single event is therefore an *unverified observation*. A verifier
//! can provide a trace excerpt or a measurement record, but the core only promotes
//! a state when the input carries a device identity and a supported method. It never
//! turns a user selection into proof of active negotiation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::codec::{CodecId, Confidence, VerificationMethod};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub codec: CodecId,
    pub device_id: String,
    pub observed_at: DateTime<Utc>,
    pub method: VerificationMethod,
    pub source_label: String,
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationDecision {
    Verified,
    RejectedNoDevice,
    RejectedUnsupportedMethod,
    RejectedStale,
}

pub struct Verifier {
    pub max_age: chrono::Duration,
}

impl Default for Verifier {
    fn default() -> Self {
        Self {
            max_age: chrono::Duration::hours(12),
        }
    }
}

impl Verifier {
    pub fn evaluate(
        &self,
        evidence: &VerificationEvidence,
        now: DateTime<Utc>,
    ) -> VerificationDecision {
        if evidence.device_id.trim().is_empty() {
            return VerificationDecision::RejectedNoDevice;
        }
        if !matches!(
            evidence.method,
            VerificationMethod::UserProvidedTrace | VerificationMethod::ManualExternalAnalyzer
        ) {
            return VerificationDecision::RejectedUnsupportedMethod;
        }
        if now.signed_duration_since(evidence.observed_at) > self.max_age
            || evidence.observed_at > now + chrono::Duration::minutes(5)
        {
            return VerificationDecision::RejectedStale;
        }
        VerificationDecision::Verified
    }

    pub fn confidence(&self, decision: VerificationDecision) -> Confidence {
        match decision {
            VerificationDecision::Verified => Confidence::High,
            _ => Confidence::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(device: &str, method: VerificationMethod) -> VerificationEvidence {
        VerificationEvidence {
            codec: CodecId::Ldac,
            device_id: device.into(),
            observed_at: Utc::now(),
            method,
            source_label: "test trace".into(),
            note: "test".into(),
        }
    }

    #[test]
    fn device_identity_is_mandatory() {
        let v = Verifier::default();
        assert_eq!(
            v.evaluate(
                &evidence("", VerificationMethod::UserProvidedTrace),
                Utc::now()
            ),
            VerificationDecision::RejectedNoDevice
        );
    }

    #[test]
    fn etw_alone_is_not_promoted() {
        let v = Verifier::default();
        assert_eq!(
            v.evaluate(
                &evidence("dev", VerificationMethod::A2dpEtwEvent),
                Utc::now()
            ),
            VerificationDecision::RejectedUnsupportedMethod
        );
    }

    #[test]
    fn user_trace_with_device_is_verified() {
        let v = Verifier::default();
        assert_eq!(
            v.evaluate(
                &evidence("dev", VerificationMethod::UserProvidedTrace),
                Utc::now()
            ),
            VerificationDecision::Verified
        );
    }

    #[test]
    fn stale_evidence_is_rejected() {
        let v = Verifier::default();
        let mut e = evidence("dev", VerificationMethod::UserProvidedTrace);
        e.observed_at = Utc::now() - chrono::Duration::hours(13);
        assert_eq!(
            v.evaluate(&e, Utc::now()),
            VerificationDecision::RejectedStale
        );
    }
}
