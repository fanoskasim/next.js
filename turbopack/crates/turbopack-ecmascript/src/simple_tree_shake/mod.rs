//! Intermediate tree shaking that uses global information but not good as the full tree shaking.

use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbopack_core::{module::Module, module_graph::SingleModuleGraph};

use crate::{chunk::EcmascriptChunkPlaceable, references::esm::EsmAssetReference};

#[turbo_tasks::function]
pub async fn is_export_used(
    module: ResolvedVc<Box<dyn EcmascriptChunkPlaceable>>,
    export_name: RcStr,
    graph: ResolvedVc<SingleModuleGraph>,
) -> Result<Vc<bool>> {
    let references = module.references();

    Ok(Vc::cell(true))
}
