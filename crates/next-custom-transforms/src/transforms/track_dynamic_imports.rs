use serde::Deserialize;
use swc_core::{
    common::{util::take::Take, SyntaxContext},
    ecma::{
        ast::*,
        utils::{private_ident, quote_ident, quote_str},
        visit::{noop_visit_mut_type, visit_mut_pass, VisitMut, VisitMutWith},
    },
    quote,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Config {}

pub fn track_dynamic_imports(unresolved_ctxt: SyntaxContext) -> impl VisitMut + Pass {
    visit_mut_pass(ImportReplacer::new(unresolved_ctxt))
}

struct ImportReplacer {
    unresolved_ctxt: SyntaxContext,
    track_dynamic_import_local_ident: Ident,
    track_async_function_local_ident: Ident,
    has_dynamic_import: bool,
    has_webpack_load: bool,
    has_turbopack_load: bool,
}

impl ImportReplacer {
    pub fn new(unresolved_ctxt: SyntaxContext) -> Self {
        ImportReplacer {
            unresolved_ctxt,
            track_dynamic_import_local_ident: private_ident!("$$trackDynamicImport__"),
            track_async_function_local_ident: private_ident!("$$trackAsyncFunction__"),
            has_dynamic_import: false,
            has_webpack_load: false,
            has_turbopack_load: false,
        }
    }
}

impl VisitMut for ImportReplacer {
    noop_visit_mut_type!(); // TODO: what does this do?

    fn visit_mut_module_items(&mut self, stmts: &mut Vec<ModuleItem>) {
        stmts.visit_mut_children_with(self);

        // import()

        if self.has_dynamic_import {
            // if we found an import() while visiting children, we need to import the helper
            stmts.insert(
                0,
                quote!(
                    "import { trackDynamicImport as $wrapper_fn } from \
                     'private-next-rsc-track-dynamic-import'" as ModuleItem,
                    wrapper_fn = self.track_dynamic_import_local_ident.clone()
                ),
            );
        }

        // bundler globals

        let mut did_insert_import_for_track_async_function = false;
        let mut maybe_insert_import_for_track_async_function = |stmts: &mut Vec<ModuleItem>| {
            stmts.insert(
                0,
                quote!(
                    "import { trackAsyncFunction as $wrapper_fn } from \
                     'private-next-rsc-track-dynamic-import'" as ModuleItem,
                    wrapper_fn = self.track_async_function_local_ident.clone()
                ),
            );
            did_insert_import_for_track_async_function = true;
        };

        let add_load_wrapper = |stmts: &mut Vec<ModuleItem>, name: &str| {
            stmts.insert(
                0,
                quote!(
                    "{\n
                        if (typeof $name === 'function') {\n
                            $name = $wrapper_fn($name_string, $name)\n
                        }\n
                    }\n" as ModuleItem,
                    name = quote_ident!(self.unresolved_ctxt, name).into(),
                    name_string: Expr = quote_str!(name).into(),
                    wrapper_fn = self.track_async_function_local_ident.clone()
                ),
            );
        };

        if self.has_turbopack_load {
            add_load_wrapper(stmts, "__turbopack_load__");
            maybe_insert_import_for_track_async_function(stmts);
        }

        if self.has_webpack_load {
            add_load_wrapper(stmts, "__webpack_load__");
            maybe_insert_import_for_track_async_function(stmts);
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        // before: `import(...)`
        // after:  `$$trackDynamicImport__(import(...))`

        if let Expr::Call(CallExpr {
            callee: Callee::Import(_),
            ..
        }) = expr
        {
            self.has_dynamic_import = true;
            *expr = quote!(
                "$wrapper_fn($expr)" as Expr,
                wrapper_fn = self.track_dynamic_import_local_ident.clone(),
                expr: Expr = expr.take()
            )
        }
    }

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        // find references to bundler globals
        //
        // "globals" like this use the unresolved syntax context
        // https://rustdoc.swc.rs/swc_core/ecma/transforms/base/fn.resolver.html#unresolved_mark
        // if it's not unresolved, then there's a local redefinition which we don't want to touch
        if ident.ctxt == self.unresolved_ctxt {
            if ident.sym == "__webpack_load__" {
                self.has_webpack_load = true;
            } else if ident.sym == "__turbopack_load__" {
                self.has_turbopack_load = true;
            }
        }
    }
}
