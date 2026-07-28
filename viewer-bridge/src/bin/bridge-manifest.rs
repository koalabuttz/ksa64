use ksa64_viewer_bridge::artifact_manifest::{
    encode_manifest_v2, expected_structure_sizes, BridgeArtifactManifestV2,
    BRIDGE_MANIFEST_V2_SCHEMA,
};
use ksa64_viewer_bridge::{KSA64_VIEWER_ABI_VERSION, KSA64_VIEWER_BUILD_IDENTITY};

fn main() {
    if let Err(error) = run() {
        eprintln!("bridge-manifest: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    if values.len() != 8 {
        return Err(String::from("usage: bridge-manifest SOURCE_COMMIT PROFILE LIBRARY_FILE TARGET_TRIPLE OS ARCH SHA256 CATALOG_IDENTITY"));
    }
    let manifest = BridgeArtifactManifestV2 {
        schema: BRIDGE_MANIFEST_V2_SCHEMA.to_owned(),
        abi_version: KSA64_VIEWER_ABI_VERSION,
        build_identity: KSA64_VIEWER_BUILD_IDENTITY,
        source_commit: values[0].clone(),
        profile: values[1].clone(),
        library_file: values[2].clone(),
        target_triple: values[3].clone(),
        operating_system: values[4].clone(),
        architecture: values[5].clone(),
        sha256: values[6].clone(),
        catalog_identity: values[7].clone(),
        structure_sizes: expected_structure_sizes(),
    };
    let bytes = encode_manifest_v2(&manifest).map_err(|error| error.to_string())?;
    print!(
        "{}",
        String::from_utf8(bytes)
            .map_err(|_| String::from("manifest encoder returned non-UTF-8"))?
    );
    Ok(())
}
