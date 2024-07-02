//! Implements the `dfx-orbit canister upload-http-assets` CLI command.
use ic_asset::canister_api::{
    methods::batch::compute_evidence, types::batch_upload::common::ComputeEvidenceArguments,
};
use ic_utils::canister::CanisterBuilder;
use slog::info;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::args::canister::UploadHttpAssets as Args;

/// The main entry point for the `dfx orbit` CLI.
pub async fn exec(args: Args) -> anyhow::Result<()> {
    let Args {
        canister,
        path,
        verbose: _verbose,
    } = args;
    // The path is needed in various forms.  If dirs is plural, maybe we should accept multiple dirs?
    let dir: PathBuf = PathBuf::from(&path);
    let dirs: Vec<&Path> = vec![dir.as_path()];

    let mut station_agent = crate::orbit_station_agent::StationAgent::new()?;
    let canister_id = station_agent.canister_id(&canister)?;
    let logger = station_agent.dfx.logger().clone();
    // Upload assets
    let canister_agent = CanisterBuilder::new()
        .with_agent(station_agent.dfx.agent().await?)
        .with_canister_id(canister_id)
        .build()?;
    let assets = assets_as_hash_map(&path);
    let batch_id = ic_asset::upload_and_propose(&canister_agent, assets, &logger).await?;
    println!("Proposed batch_id: {}", batch_id);
    // Compute evidence locally:
    let local_evidence = ic_asset::compute_evidence(&canister_agent, &dirs, &logger).await?;
    // Wait for the canister to compute the evidence:
    // This part is stolen from ic_asset::sync::prepare_sync_for_proposal.  Unfortunately the relevant functions are private.
    // The docs explicitly include waiting for the evidence so this should really be made easier!  See: https://github.com/dfinity/sdk/blob/2509e81e11e71dce4045c679686c952809525470/docs/design/asset-canister-interface.md?plain=1#L85

    let compute_evidence_arg = ComputeEvidenceArguments {
        batch_id: batch_id.clone(),
        max_iterations: Some(97), // 75% of max(130) = 97.5
    };
    info!(logger, "Computing evidence.");
    let evidence = loop {
        if let Some(evidence) = compute_evidence(&canister_agent, &compute_evidence_arg).await? {
            break evidence;
        }
    };
    println!("Proposed batch_id: {}", batch_id);
    println!("Local evidence: \"{}\"", escape_hex_string(&local_evidence));
    println!("Canister computed evidence: {}", blob_from_bytes(&evidence));
    // TODO: The local evidence doesn't match the canister evidence.

    // Maybe compute evidence locally and then compare?

    // TODO: Get Orbit to make the API call to commit the changes.
    Ok(())
}

/// Lists all the files at the given path.
///
/// - Links are followed.
/// - Only files are returned.
/// - The files are sorted by name.
/// - Any files that cannot be read are ignored.
/// - The path includes the prefix.
fn list_assets(path: &str) -> Vec<PathBuf> {
    WalkDir::new(path)
        .sort_by_file_name()
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect()
}

/// A hash map of all assets.
///
/// Note: Given that ordering in a HashMap is not deterministic, is this really the best API?
fn assets_as_hash_map(asset_dir: &str) -> HashMap<String, PathBuf> {
    list_assets(asset_dir)
        .into_iter()
        .map(|asset_path| {
            let relative_path = asset_path.strip_prefix(asset_dir).expect(
                "Internal error: list_assets should have returned only files in the asset_dir",
            );
            let http_path = format!(
                "/{relative_path}",
                relative_path = relative_path.to_string_lossy()
            );
            (http_path, asset_path)
        })
        .collect()
}

/// Converts a hex string into one escaped as in a candid blob.
fn escape_hex_string(s: &str) -> String {
    let mut ans = String::with_capacity(s.len() + s.len() / 2);
    for chunk in s.chars().collect::<Vec<_>>()[..].chunks(2) {
        ans.push('\\');
        for char in chunk {
            ans.push(*char);
        }
    }
    ans
}

/// Converts a byte array into one escaped as a candid blob
fn blob_from_bytes(bytes: &[u8]) -> String {
    let mut ans = String::with_capacity(bytes.len() + bytes.len() / 2);
    for byte in bytes {
        ans.push('\\');
        ans.push_str(&format!("{:02x}", byte));
    }
    ans
}