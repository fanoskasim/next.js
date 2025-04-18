//! Intermediate tree shaking that uses global information but not good as the full tree shaking.

use anyhow::{bail, Context, Result};
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbopack_core::{
    module_graph::{ModuleGraph, SingleModuleGraph},
    resolve::ExportUsage,
};

use crate::chunk::EcmascriptChunkPlaceable;

#[turbo_tasks::function]
pub async fn is_export_used(
    graph: ResolvedVc<ModuleGraph>,
    module: ResolvedVc<Box<dyn EcmascriptChunkPlaceable>>,
    export_name: RcStr,
) -> Result<Vc<bool>> {
    let export_usage_info = compute_export_usage_info(graph)
        .resolve_strongly_consistent()
        .await?;

    let export_usage_info = export_usage_info.await?;
    let Some(exports) = export_usage_info.used_exports.get(&module) else {
        bail!(
            "module not found in export usage info. Something is wrong with the export usage info."
        );
    };

    for export in exports {
        match export {
            ExportUsage::Named(rc_str) => {
                if rc_str == &export_name {
                    return Ok(Vc::cell(true));
                }
            }
            ExportUsage::Evaluation => {}
            ExportUsage::All => {
                return Ok(Vc::cell(true));
            }
        }
    }

    Ok(Vc::cell(false))
}

#[turbo_tasks::function(operation)]
pub async fn compute_export_usage_info(
    graph: ResolvedVc<ModuleGraph>,
) -> Result<Vc<ExportUsageInfo>> {
    let mut results = Vec::new();
    for g in &graph.await?.graphs {
        results.push(compute_export_usage_info_single(**g));
    }

    let mut result = ExportUsageInfo::default();

    for item in results {
        for (k, v) in &item.await?.used_exports {
            result
                .used_exports
                .entry(*k)
                .or_insert_with(Vec::new)
                .extend(v.clone());
        }
    }

    Ok(result.cell())
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
                    used_exports
                        .entry(target_module)
                        .or_insert_with(Vec::new)
                        .push(ref_data.export.clone());
                }
            }

            turbopack_core::module_graph::GraphTraversalAction::Continue
        })
        .context("failed to traverse module graph")?;

    Ok(ExportUsageInfo { used_exports }.cell())
}

#[turbo_tasks::value]
#[derive(Default)]
pub struct ExportUsageInfo {
    used_exports: FxHashMap<ResolvedVc<Box<dyn EcmascriptChunkPlaceable>>, Vec<ExportUsage>>,
}
