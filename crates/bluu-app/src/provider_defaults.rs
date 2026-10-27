use std::collections::BTreeMap;

use bluu_core::{CodecId, ProviderRecord};

pub fn defaults_for_order(record: &ProviderRecord, order: &[CodecId]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for codec in order {
        if let Some(entry) = record.manifest.codecs.iter().find(|c| c.codec == *codec) {
            for parameter in &entry.parameters {
                out.insert(parameter.key().to_string(), parameter.default_value());
            }
        }
    }
    out
}

pub fn operation_label(operation: bluu_core::ProviderAction) -> &'static str {
    match operation {
        bluu_core::ProviderAction::Describe => "Reading provider capabilities",
        bluu_core::ProviderAction::Apply => "Applying preference",
        bluu_core::ProviderAction::Rollback => "Rolling back",
        bluu_core::ProviderAction::Verify => "Verifying active codec",
    }
}
