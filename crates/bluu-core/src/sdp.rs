//! Minimal, bounds-checked Bluetooth SDP decoder used to observe *profiles* advertised
//! by a paired device. SDP does not carry A2DP codec capability; codec SEP discovery
//! happens over AVDTP and is not exposed by Windows. This module never turns a profile
//! into a codec claim.

use crate::{
    codec::Evidence,
    model::{AudioProfile, ProfileObservation},
};

#[derive(Clone, Debug, PartialEq)]
pub enum DataElement {
    Nil,
    UInt(u64),
    Int(i64),
    Uuid([u8; 16]),
    Str(String),
    Bool(bool),
    Seq(Vec<DataElement>),
    Alternative(Vec<DataElement>),
    Url(String),
}

const MAX_DEPTH: usize = 16;

fn unsigned(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

pub fn parse(bytes: &[u8]) -> Option<DataElement> {
    let mut offset = 0;
    let value = parse_at(bytes, &mut offset, 0)?;
    (offset == bytes.len()).then_some(value)
}

fn parse_at(bytes: &[u8], offset: &mut usize, depth: usize) -> Option<DataElement> {
    let header = *bytes.get(*offset)?;
    *offset += 1;
    let kind = header >> 3;
    let size_code = header & 0x07;
    let length = match size_code {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 16,
        5 => {
            let n = usize::from(*bytes.get(*offset)?);
            *offset += 1;
            n
        }
        6 => {
            let n = usize::try_from(unsigned(bytes.get(*offset..*offset + 2)?)).ok()?;
            *offset += 2;
            n
        }
        7 => {
            let n = usize::try_from(unsigned(bytes.get(*offset..*offset + 4)?)).ok()?;
            *offset += 4;
            n
        }
        _ => return None,
    };
    let data = bytes.get(*offset..offset.checked_add(length)?)?;
    *offset += length;
    Some(match kind {
        0 => DataElement::Nil,
        1 => DataElement::UInt(unsigned(data)),
        2 => DataElement::Int(unsigned(data) as i64),
        3 => match data.len() {
            2 => {
                let mut uuid = [0u8; 16];
                uuid[2..4].copy_from_slice(data);
                uuid[4..].copy_from_slice(&[
                    0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
                ]);
                DataElement::Uuid(uuid)
            }
            4 => {
                let mut uuid = [0u8; 16];
                uuid[..4].copy_from_slice(data);
                uuid[4..].copy_from_slice(&[
                    0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
                ]);
                DataElement::Uuid(uuid)
            }
            16 => DataElement::Uuid(data.try_into().ok()?),
            _ => return None,
        },
        4 | 8 => DataElement::Str(String::from_utf8_lossy(data).trim_end_matches('\0').into()),
        5 => DataElement::Bool(data.first().copied().unwrap_or_default() != 0),
        6 | 7 => {
            if depth >= MAX_DEPTH {
                return None;
            }
            let mut values = Vec::new();
            let mut inner = 0usize;
            while inner < data.len() {
                values.push(parse_at(data, &mut inner, depth + 1)?);
            }
            if kind == 6 {
                DataElement::Seq(values)
            } else {
                DataElement::Alternative(values)
            }
        }
        _ => return None,
    })
}

impl DataElement {
    pub fn seq(&self) -> &[DataElement] {
        match self {
            Self::Seq(v) | Self::Alternative(v) => v,
            _ => &[],
        }
    }

    pub fn uint(&self) -> Option<u64> {
        match self {
            Self::UInt(v) => Some(*v),
            _ => None,
        }
    }

    pub fn short_uuid(&self) -> Option<u16> {
        match self {
            Self::Uuid(u) => Some(u16::from_be_bytes([u[2], u[3]])),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ServiceRecord {
    pub attributes: Vec<(u16, DataElement)>,
}

impl ServiceRecord {
    pub fn from_attributes(attributes: Vec<(u16, DataElement)>) -> Self {
        Self { attributes }
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let DataElement::Seq(items) = parse(bytes)? else {
            return None;
        };
        let mut attributes = Vec::new();
        let (pairs, _) = items.as_chunks::<2>();
        for pair in pairs {
            if let [DataElement::UInt(id), value] = pair {
                if let Ok(id) = u16::try_from(*id) {
                    attributes.push((id, value.clone()));
                }
            }
        }
        Some(Self { attributes })
    }

    pub fn get(&self, id: u16) -> Option<&DataElement> {
        self.attributes
            .iter()
            .find_map(|(a, v)| (*a == id).then_some(v))
    }

    pub fn service_classes(&self) -> Vec<u16> {
        self.get(0x0001)
            .map(|v| v.seq().iter().filter_map(DataElement::short_uuid).collect())
            .unwrap_or_default()
    }

    pub fn profiles(&self) -> Vec<(u16, u16)> {
        self.get(0x0009)
            .map(|value| {
                value
                    .seq()
                    .iter()
                    .filter_map(|entry| {
                        let fields = entry.seq();
                        Some((fields.first()?.short_uuid()?, fields.get(1)?.uint()? as u16))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn observations(&self) -> Vec<ProfileObservation> {
        let mut out = Vec::new();
        for (uuid, version) in self.profiles() {
            let (profile, label) = match uuid {
                0x110D => (AudioProfile::A2dpSource, "A2DP"),
                0x110B => (AudioProfile::A2dpSink, "A2DP"),
                0x111E => (AudioProfile::HandsFree, "HFP"),
                0x1108 => (AudioProfile::Headset, "HSP"),
                0x110E | 0x110F => (AudioProfile::Avrcp, "AVRCP"),
                _ => continue,
            };
            out.push(ProfileObservation {
                profile,
                version: Some(format!("{}.{}", version >> 8, version & 0xFF)),
                evidence: Evidence::Observed,
                note: format!("Decoded from the paired device's cached SDP {label} record."),
            });
        }
        for class in self.service_classes() {
            let profile = match class {
                0x110D if !out.iter().any(|p| p.profile == AudioProfile::A2dpSource) => {
                    Some(AudioProfile::A2dpSource)
                }
                0x110B if !out.iter().any(|p| p.profile == AudioProfile::A2dpSink) => {
                    Some(AudioProfile::A2dpSink)
                }
                0x111E if !out.iter().any(|p| p.profile == AudioProfile::HandsFree) => {
                    Some(AudioProfile::HandsFree)
                }
                _ => None,
            };
            if let Some(profile) = profile {
                out.push(ProfileObservation {
                    profile,
                    version: None,
                    evidence: Evidence::Observed,
                    note: "Decoded from the cached SDP service class list.".into(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a2dp_sink_profile_record() {
        let record = [
            0x35, 0x18, 0x09, 0x00, 0x01, 0x35, 0x06, 0x19, 0x11, 0x0B, 0x19, 0x11, 0x0D, 0x09,
            0x00, 0x09, 0x35, 0x08, 0x35, 0x06, 0x19, 0x11, 0x0D, 0x09, 0x01, 0x04,
        ];
        let parsed = ServiceRecord::parse(&record).expect("valid SDP");
        assert_eq!(parsed.service_classes(), vec![0x110B, 0x110D]);
        assert_eq!(parsed.profiles(), vec![(0x110D, 0x0104)]);
        assert_eq!(parsed.observations()[0].profile, AudioProfile::A2dpSource);
    }

    #[test]
    fn rejects_deeply_nested_hostile_input() {
        let mut bytes = vec![0x08, 0x01];
        for _ in 0..40 {
            bytes = [vec![0x35, bytes.len() as u8], bytes].concat();
        }
        assert!(parse(&bytes).is_none());
    }

    #[test]
    fn rejects_truncated_values() {
        assert!(parse(&[0x35, 0x05, 0x09]).is_none());
        assert!(parse(&[0x09, 0x00]).is_none());
    }
}
