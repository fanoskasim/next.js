use serde::Deserialize;
use swc_core::{
    common::{comments::Comments, util::take::Take, Span, SyntaxContext},
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

pub fn track_dynamic_imports<C: Comments>(
    unresolved_ctxt: SyntaxContext,
    comments: C,
) -> impl VisitMut + Pass {
    visit_mut_pass(ImportReplacer::new(unresolved_ctxt, comments))
}

struct ImportReplacer<C> {
    unresolved_ctxt: SyntaxContext,
    comments: C,
    track_dynamic_import_local_ident: Ident,
    track_async_function_local_ident: Ident,
    has_dynamic_import: bool,
    has_global: TrackedGlobals,
}

struct TrackedGlobals {
    webpack_load: bool,
    webpack_require: bool,
    turbopack_load: bool,
    turbopack_require: bool,
}

impl<C> ImportReplacer<C>
where
    C: Comments,
{
    pub fn new(unresolved_ctxt: SyntaxContext, comments: C) -> Self {
        ImportReplacer {
            unresolved_ctxt,
            comments,
            track_dynamic_import_local_ident: private_ident!("$$trackDynamicImport__"),
            track_async_function_local_ident: private_ident!("$$trackAsyncFunction__"),
            has_dynamic_import: false,
            has_global: TrackedGlobals {
                webpack_load: false,
                webpack_require: false,
                turbopack_load: false,
                turbopack_require: false,
            },
        }
    }
}

impl<C> VisitMut for ImportReplacer<C>
where
    C: Comments,
{
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

        let insert_global_fn_wrapper = |stmts: &mut Vec<ModuleItem>, name: &str| {
            let name_ident: Ident = quote_ident!(self.unresolved_ctxt, name).into();

            let replacement_expr = {
                let expr_span = Span::dummy_with_cmt();
                let mut expr: Expr = quote!(
                    "$wrapper_fn($name_string, $name)" as Expr,
                    wrapper_fn = self.track_async_function_local_ident.clone(),
                    name_string: Expr = quote_str!(name).into(),
                    name = name_ident.clone(),
                );

                // this call doesn't have any side effects, so add `/*#__PURE__*/`
                expr.set_span(expr_span);
                self.comments.add_pure_comment(expr_span.lo);
                expr
            };

            stmts.insert(
                0,
                quote!(
                    "if (typeof $name === 'function') {\
                        $name = $replacement_expr;\
                    }" as ModuleItem,
                    name = name_ident.clone(),
                    replacement_expr: Expr = replacement_expr,
                ),
            );
        };

        let mut needs_track_async_function = false;
        if self.has_global.webpack_load {
            insert_global_fn_wrapper(stmts, "__webpack_load__");
            needs_track_async_function = true;
        }

        if self.has_global.webpack_require {
            insert_global_fn_wrapper(stmts, "__webpack_require__");
            needs_track_async_function = true;
        }

        if self.has_global.turbopack_load {
            insert_global_fn_wrapper(stmts, "__turbopack_load__");
            needs_track_async_function = true;
        }
        if self.has_global.turbopack_require {
            insert_global_fn_wrapper(stmts, "__turbopack_require__");
            needs_track_async_function = true;
        }

        if needs_track_async_function {
            stmts.insert(
                0,
                quote!(
                    "import { trackAsyncFunction as $wrapper_fn } from \
                     'private-next-rsc-track-dynamic-import'" as ModuleItem,
                    wrapper_fn = self.track_async_function_local_ident.clone()
                ),
            );
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
                self.has_global.webpack_load = true;
            } else if ident.sym == "__webpack_require__" {
                self.has_global.webpack_require = true;
            } else if ident.sym == "__turbopack_load__" {
                self.has_global.turbopack_load = true;
            } else if ident.sym == "__turbopack_require__" {
                self.has_global.turbopack_require = true;
            }
        }
    }
}
