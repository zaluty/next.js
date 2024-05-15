use anyhow::Result;
use turbo_tasks::{
    graph::{AdjacencyMap, GraphTraversal},
    macro_task, Completion, Completions, Vc,
};
use turbo_tasks_fs::{rebase, FileSystemPath};
use turbopack_binding::turbopack::core::{
    asset::Asset,
    output::{OutputAsset, OutputAssets},
};

/// Emits all assets transitively reachable from the given chunks, that are
/// inside the node root or the client root.
///
/// Assets inside the given client root are rebased to the given client output
/// path.
#[turbo_tasks::function]
pub fn emit_all_assets(
    assets: Vc<OutputAssets>,
    node_root: Vc<FileSystemPath>,
    client_relative_path: Vc<FileSystemPath>,
    client_output_path: Vc<FileSystemPath>,
) -> Vc<Completion> {
    emit_assets(
        all_assets_from_entries(assets),
        node_root,
        client_relative_path,
        client_output_path,
    )
}

/// Emits all assets transitively reachable from the given chunks, that are
/// inside the node root or the client root.
///
/// Assets inside the given client root are rebased to the given client output
/// path.
#[turbo_tasks::function]
pub async fn emit_assets(
    assets: Vc<OutputAssets>,
    node_root: Vc<FileSystemPath>,
    client_relative_path: Vc<FileSystemPath>,
    client_output_path: Vc<FileSystemPath>,
) -> Result<Vc<Completion>> {
    Ok(Vc::<Completions>::cell(
        assets
            .await?
            .iter()
            .copied()
            .map(|asset| {
                emit_single_asset(
                    asset,
                    node_root.clone(),
                    client_relative_path.clone(),
                    client_output_path.clone(),
                )
            })
            .collect(),
    )
    .completed())
}

#[turbo_tasks::function]
async fn emit_single_asset(
    asset: Vc<Box<dyn OutputAsset>>,
    node_root: Vc<FileSystemPath>,
    client_relative_path: Vc<FileSystemPath>,
    client_output_path: Vc<FileSystemPath>,
) -> Result<Vc<Completion>> {
    macro_task();

    if asset
        .ident()
        .path()
        .await?
        .is_inside_ref(&*node_root.await?)
    {
        return Ok(emit(asset).await?);
    } else if asset
        .ident()
        .path()
        .await?
        .is_inside_ref(&*client_relative_path.await?)
    {
        // Client assets are emitted to the client output path, which is prefixed with
        // _next. We need to rebase them to remove that prefix.
        return Ok(emit_rebase(asset, client_relative_path, client_output_path).await?);
    }

    Ok(Completion::unchanged())
}

/// Emits all assets transitively reachable from the given chunks, that are
/// inside the client root.
///
/// Assets inside the given client root are rebased to the given client output
/// path.
#[turbo_tasks::function]
pub async fn emit_client_assets(
    assets: Vc<OutputAssets>,
    client_relative_path: Vc<FileSystemPath>,
    client_output_path: Vc<FileSystemPath>,
) -> Result<Vc<Completion>> {
    Ok(Vc::<Completions>::cell(
        assets
            .await?
            .iter()
            .copied()
            .map(|asset| emit_single_client_asset(asset, client_relative_path, client_output_path))
            .collect(),
    )
    .completed())
}

/// Emits a single client asset.
#[turbo_tasks::function]
async fn emit_single_client_asset(
    asset: Vc<Box<dyn OutputAsset>>,
    client_relative_path: Vc<FileSystemPath>,
    client_output_path: Vc<FileSystemPath>,
) -> Result<Vc<Completion>> {
    macro_task();

    if asset
        .ident()
        .path()
        .await?
        .is_inside_ref(&*client_relative_path.await?)
    {
        // Client assets are emitted to the client output path, which is prefixed with
        // _next. We need to rebase them to remove that prefix.
        return Ok(emit_rebase(asset, client_relative_path, client_output_path).await?);
    }

    Ok(Completion::unchanged())
}

async fn emit(asset: Vc<Box<dyn OutputAsset>>) -> Result<Vc<Completion>> {
    let content = asset.content();
    let path = asset.ident().path();
    let content = content.resolve().await?;
    let path = path.resolve().await?;
    Ok(content.write(path))
}

async fn emit_rebase(
    asset: Vc<Box<dyn OutputAsset>>,
    from: Vc<FileSystemPath>,
    to: Vc<FileSystemPath>,
) -> Result<Vc<Completion>> {
    let content = asset.content();
    let path = asset.ident().path();
    let content = content.resolve().await?;
    let path = path.resolve().await?;
    let path = rebase(path, from, to).resolve().await?;
    Ok(content.write(path))
}

/// Walks the asset graph from multiple assets and collect all referenced
/// assets.
#[turbo_tasks::function]
pub async fn all_assets_from_entries(entries: Vc<OutputAssets>) -> Result<Vc<OutputAssets>> {
    Ok(Vc::cell(
        AdjacencyMap::new()
            .skip_duplicates()
            .visit(entries.await?.iter().copied(), get_referenced_assets)
            .await
            .completed()?
            .into_inner()
            .into_reverse_topological()
            .collect(),
    ))
}

/// Computes the list of all chunk children of a given chunk.
async fn get_referenced_assets(
    asset: Vc<Box<dyn OutputAsset>>,
) -> Result<impl Iterator<Item = Vc<Box<dyn OutputAsset>>> + Send> {
    Ok(asset
        .references()
        .await?
        .iter()
        .copied()
        .collect::<Vec<_>>()
        .into_iter())
}
