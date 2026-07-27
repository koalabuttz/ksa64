//! Noncanonical bridge artifact manifests used by loaders and packaging tools.
//! Version 1 remains readable for the accepted Phase 12A/12B Win64 artifact;
//! version 2 describes the platform-neutral library without changing ABI v1.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const BRIDGE_MANIFEST_V1_SCHEMA: &str = "ksa64.viewer-bridge-artifact.v1";
pub const BRIDGE_MANIFEST_V2_SCHEMA: &str = "ksa64.viewer-bridge-artifact.v2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeArtifactManifestV1 {
    pub schema: String,
    pub abi_version: u32,
    pub commit: String,
    pub file: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeArtifactManifestV2 {
    pub schema: String,
    pub abi_version: u32,
    pub build_identity: u32,
    pub source_commit: String,
    pub profile: String,
    pub library_file: String,
    pub target_triple: String,
    pub operating_system: String,
    pub architecture: String,
    pub sha256: String,
    pub catalog_identity: String,
    pub structure_sizes: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeArtifactManifest {
    LegacyV1(BridgeArtifactManifestV1),
    PortableV2(BridgeArtifactManifestV2),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    InvalidJson(String),
    UnsupportedSchema(String),
    InvalidField(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(reason) => write!(output, "invalid bridge manifest JSON: {reason}"),
            Self::UnsupportedSchema(schema) => {
                write!(output, "unsupported bridge manifest schema: {schema}")
            }
            Self::InvalidField(field) => write!(output, "invalid bridge manifest field: {field}"),
        }
    }
}

impl std::error::Error for ManifestError {}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_library_file(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
        && !value.bytes().any(|byte| byte == 0)
}

fn validate_v1(value: &BridgeArtifactManifestV1) -> Result<(), ManifestError> {
    if value.schema != BRIDGE_MANIFEST_V1_SCHEMA {
        return Err(ManifestError::UnsupportedSchema(value.schema.clone()));
    }
    if value.abi_version != crate::KSA64_VIEWER_ABI_VERSION {
        return Err(ManifestError::InvalidField("abi_version"));
    }
    if value.commit.is_empty() || !value.commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ManifestError::InvalidField("commit"));
    }
    if !valid_library_file(&value.file) {
        return Err(ManifestError::InvalidField("file"));
    }
    if !is_lower_hex(&value.sha256, 64) {
        return Err(ManifestError::InvalidField("sha256"));
    }
    Ok(())
}

pub fn expected_structure_sizes() -> BTreeMap<String, u32> {
    BTreeMap::from([
        (
            "abi_info".to_owned(),
            core::mem::size_of::<crate::AbiInfo>() as u32,
        ),
        (
            "span".to_owned(),
            core::mem::size_of::<crate::Span>() as u32,
        ),
        (
            "owned_buffer".to_owned(),
            core::mem::size_of::<crate::OwnedBuffer>() as u32,
        ),
        (
            "event".to_owned(),
            core::mem::size_of::<crate::Event>() as u32,
        ),
        (
            "snapshot".to_owned(),
            core::mem::size_of::<crate::Snapshot>() as u32,
        ),
        (
            "start_request_v1".to_owned(),
            core::mem::size_of::<crate::StartRequestV1>() as u32,
        ),
        (
            "operational_view_v1".to_owned(),
            core::mem::size_of::<crate::OperationalViewV1>() as u32,
        ),
        (
            "procedure_view_v1".to_owned(),
            core::mem::size_of::<crate::ProcedureViewV1>() as u32,
        ),
        (
            "disposition_v1".to_owned(),
            core::mem::size_of::<crate::DispositionV1>() as u32,
        ),
        (
            "action_proposal_v1".to_owned(),
            core::mem::size_of::<crate::ActionProposalV1>() as u32,
        ),
        (
            "action_receipt_v1".to_owned(),
            core::mem::size_of::<crate::ActionReceiptV1>() as u32,
        ),
        (
            "timeline_event_v1".to_owned(),
            core::mem::size_of::<crate::TimelineEventV1>() as u32,
        ),
        (
            "release_sample_v1".to_owned(),
            core::mem::size_of::<crate::ReleaseSampleV1>() as u32,
        ),
        (
            "prediction_path_header_v1".to_owned(),
            core::mem::size_of::<crate::PredictionPathHeaderV1>() as u32,
        ),
        (
            "prediction_path_point_v1".to_owned(),
            core::mem::size_of::<crate::PredictionPathPointV1>() as u32,
        ),
        (
            "transport_status_v1".to_owned(),
            core::mem::size_of::<crate::TransportStatusV1>() as u32,
        ),
        (
            "finish_status_v1".to_owned(),
            core::mem::size_of::<crate::FinishStatusV1>() as u32,
        ),
    ])
}

fn validate_v2(value: &BridgeArtifactManifestV2) -> Result<(), ManifestError> {
    if value.schema != BRIDGE_MANIFEST_V2_SCHEMA {
        return Err(ManifestError::UnsupportedSchema(value.schema.clone()));
    }
    if value.abi_version != crate::KSA64_VIEWER_ABI_VERSION {
        return Err(ManifestError::InvalidField("abi_version"));
    }
    if value.build_identity != crate::KSA64_VIEWER_BUILD_IDENTITY {
        return Err(ManifestError::InvalidField("build_identity"));
    }
    if value.source_commit.is_empty()
        || !value
            .source_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ManifestError::InvalidField("source_commit"));
    }
    if value.profile.is_empty() {
        return Err(ManifestError::InvalidField("profile"));
    }
    if !valid_library_file(&value.library_file) {
        return Err(ManifestError::InvalidField("library_file"));
    }
    if value.target_triple.is_empty() {
        return Err(ManifestError::InvalidField("target_triple"));
    }
    if value.operating_system.is_empty() {
        return Err(ManifestError::InvalidField("operating_system"));
    }
    if value.architecture.is_empty() {
        return Err(ManifestError::InvalidField("architecture"));
    }
    if !is_lower_hex(&value.sha256, 64) {
        return Err(ManifestError::InvalidField("sha256"));
    }
    if !is_lower_hex(&value.catalog_identity, 64) {
        return Err(ManifestError::InvalidField("catalog_identity"));
    }
    if value.structure_sizes != expected_structure_sizes() {
        return Err(ManifestError::InvalidField("structure_sizes"));
    }
    Ok(())
}

pub fn decode_manifest(input: &[u8]) -> Result<BridgeArtifactManifest, ManifestError> {
    let generic: serde_json::Value = serde_json::from_slice(input)
        .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
    let schema = generic
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or(ManifestError::InvalidField("schema"))?
        .to_owned();
    match schema.as_str() {
        BRIDGE_MANIFEST_V1_SCHEMA => {
            let value: BridgeArtifactManifestV1 = serde_json::from_value(generic)
                .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
            validate_v1(&value)?;
            Ok(BridgeArtifactManifest::LegacyV1(value))
        }
        BRIDGE_MANIFEST_V2_SCHEMA => {
            let value: BridgeArtifactManifestV2 = serde_json::from_value(generic)
                .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
            validate_v2(&value)?;
            Ok(BridgeArtifactManifest::PortableV2(value))
        }
        other => Err(ManifestError::UnsupportedSchema(other.to_owned())),
    }
}

pub fn encode_manifest_v2(value: &BridgeArtifactManifestV2) -> Result<Vec<u8>, ManifestError> {
    validate_v2(value)?;
    let mut output = serde_json::to_vec_pretty(value)
        .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
    output.push(b'\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCEPTED_V1: &str = r#"{
  "schema": "ksa64.viewer-bridge-artifact.v1",
  "abi_version": 1,
  "commit": "423c116cf586",
  "file": "ksa64_viewer_bridge-423c116cf586-120b0001.dll",
  "sha256": "da6657a46759a028cb8901ce813af093d4d8901c76cb383f0d74601d64f26565"
}"#;

    fn portable() -> BridgeArtifactManifestV2 {
        BridgeArtifactManifestV2 {
            schema: BRIDGE_MANIFEST_V2_SCHEMA.to_owned(),
            abi_version: crate::KSA64_VIEWER_ABI_VERSION,
            build_identity: crate::KSA64_VIEWER_BUILD_IDENTITY,
            source_commit: "b9f2c79a2603".to_owned(),
            profile: "viewer".to_owned(),
            library_file: if cfg!(target_os = "windows") {
                "ksa64_viewer_bridge.dll"
            } else if cfg!(target_os = "macos") {
                "libksa64_viewer_bridge.dylib"
            } else {
                "libksa64_viewer_bridge.so"
            }
            .to_owned(),
            target_triple: env!("KSA64_TARGET_TRIPLE").to_owned(),
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            sha256: "00".repeat(32),
            catalog_identity: "11".repeat(32),
            structure_sizes: expected_structure_sizes(),
        }
    }

    #[test]
    fn accepted_v1_manifest_remains_readable() {
        match decode_manifest(ACCEPTED_V1.as_bytes()).unwrap() {
            BridgeArtifactManifest::LegacyV1(value) => {
                assert_eq!(value.commit, "423c116cf586");
                assert!(value.file.ends_with(".dll"));
            }
            BridgeArtifactManifest::PortableV2(_) => panic!("decoded legacy manifest as v2"),
        }
    }

    #[test]
    fn v2_round_trip_is_deterministic() {
        let value = portable();
        let first = encode_manifest_v2(&value).unwrap();
        let second = encode_manifest_v2(&value).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            decode_manifest(&first).unwrap(),
            BridgeArtifactManifest::PortableV2(value)
        );
    }

    #[test]
    fn malformed_or_ambiguous_manifests_fail_closed() {
        let mut value = portable();
        value.library_file = "../bridge.dll".to_owned();
        assert_eq!(
            encode_manifest_v2(&value),
            Err(ManifestError::InvalidField("library_file"))
        );
        let mut wrong_size = portable();
        wrong_size.structure_sizes.insert("snapshot".to_owned(), 1);
        assert_eq!(
            encode_manifest_v2(&wrong_size),
            Err(ManifestError::InvalidField("structure_sizes"))
        );
        let unknown = br#"{"schema":"ksa64.viewer-bridge-artifact.v3"}"#;
        assert!(matches!(
            decode_manifest(unknown),
            Err(ManifestError::UnsupportedSchema(_))
        ));
        let extra = ACCEPTED_V1.replace("\n}", ",\n  \"unexpected\": 1\n}");
        assert!(matches!(
            decode_manifest(extra.as_bytes()),
            Err(ManifestError::InvalidJson(_))
        ));
    }
}
