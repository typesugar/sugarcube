//! Desugaring for HKT type parameters (`F<_>`).
//!
//! In declarations: `F<_>` → strip `<_>`, leaving just `F`.
//! In type references within scope: `F<A>` → `$<F, A>`.

use std::collections::HashSet;
use swc_ecma_visit::{VisitMut, VisitMutWith};

/// Visitor that rewrites `F<A>` to `$<F, A>` for names in `hkt_names`.
pub struct HktRewriter {
    hkt_names: HashSet<String>,
}

impl HktRewriter {
    pub fn new(hkt_names: HashSet<String>) -> Self {
        Self { hkt_names }
    }
}

impl VisitMut for HktRewriter {
    fn visit_mut_ts_type_ref(&mut self, node: &mut swc_ecma_ast::TsTypeRef) {
        node.visit_mut_children_with(self);

        let name = match &node.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
            _ => return,
        };

        if !self.hkt_names.contains(&name) {
            return;
        }

        // F<A> → $<F, A>: wrap the original type args with F prepended.
        if let Some(type_params) = &node.type_params {
            let span = node.span;
            let f_type = Box::new(swc_ecma_ast::TsType::TsTypeRef(swc_ecma_ast::TsTypeRef {
                span,
                type_name: node.type_name.clone(),
                type_params: None,
            }));

            let mut new_params = vec![f_type];
            for param in &type_params.params {
                new_params.push(param.clone());
            }

            node.type_name = swc_ecma_ast::TsEntityName::Ident(
                swc_ecma_ast::Ident::new_no_ctxt("$".into(), span),
            );
            node.type_params = Some(Box::new(swc_ecma_ast::TsTypeParamInstantiation {
                span: type_params.span,
                params: new_params,
            }));
        }
    }
}
