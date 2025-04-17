//! Intermediate tree shaking that uses global information but not good as the full tree shaking.

use anyhow::{Context, Result};
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbopack_core::{module_graph::SingleModuleGraph, resolve::Export};

use crate::chunk::EcmascriptChunkPlaceable;

#[turbo_tasks::function]
pub async fn is_export_used(
    graph: ResolvedVc<SingleModuleGraph>,
    module: ResolvedVc<Box<dyn EcmascriptChunkPlaceable>>,
    export_name: RcStr,
) -> Result<Vc<bool>> {
    let export_usage_info = compute_export_usage_info_single(*graph).await?;
    let Some(exports) = export_usage_info.used_exports.get(&module) else {
        // Let's be safe.
        return Ok(Vc::cell(true));
    };

    for export in exports {
        match export {
            Export::Named(rc_str) => {
                if rc_str == &export_name {
                    return Ok(Vc::cell(true));
                }
            }
            Export::All => {
                return Ok(Vc::cell(true));
            }
        }
    }

    Ok(Vc::cell(false))
}

#[turbo_tasks::function]
pub async fn compute_export_usage_info_single(
    graph: ResolvedVc<SingleModuleGraph>,
) -> Result<Vc<ExportUsageInfo>> {
    let graph = graph.await?;
    let mut used_exports = FxHashMap::default();

    // Traverse the module graph

    graph
        .traverse_edges(|(edge, target)| {
            if let Some(target_module) =
                ResolvedVc::try_downcast::<Box<dyn EcmascriptChunkPlaceable>>(target.module)
            {
                if let Some((_, ref_data)) = edge {
                    if let Some(export) = &ref_data.export {
                        used_exports
                            .entry(target_module)
                            .or_insert_with(Vec::new)
                            .push(export.clone());
                    }
                }
            }

            turbopack_core::module_graph::GraphTraversalAction::Continue
        })
        .context("failed to traverse module graph")?;

    Ok(ExportUsageInfo { used_exports }.cell())
}

#[turbo_tasks::value]
pub struct ExportUsageInfo {
    used_exports: FxHashMap<ResolvedVc<Box<dyn EcmascriptChunkPlaceable>>, Vec<Export>>,
}
