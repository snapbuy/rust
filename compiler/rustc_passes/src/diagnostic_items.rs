//! Detecting diagnostic items.
//!
//! Diagnostic items are items that are not language-inherent, but can reasonably be expected to
//! exist for diagnostic purposes. This allows diagnostic authors to refer to specific items
//! directly, without having to guess module paths and crates.
//! Examples are:
//!
//! * Traits like `Debug`, that have no bearing on language semantics
//!
//! * Compiler internal types like `Ty` and `TyCtxt`

use rustc_ast as ast;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir as hir;
use rustc_hir::itemlikevisit::ItemLikeVisitor;
use rustc_middle::ty::query::Providers;
use rustc_middle::ty::TyCtxt;
use rustc_session::Session;
use rustc_span::def_id::{CrateNum, DefId, LocalDefId, LOCAL_CRATE};
use rustc_span::symbol::{sym, Symbol};

struct DiagnosticItemCollector<'tcx> {
    // items from this crate
    items: FxHashMap<Symbol, DefId>,
    tcx: TyCtxt<'tcx>,
}

impl<'v, 'tcx> ItemLikeVisitor<'v> for DiagnosticItemCollector<'tcx> {
    fn visit_item(&mut self, item: &hir::Item<'_>) {
        self.observe_item(item.def_id);
    }

    fn visit_trait_item(&mut self, trait_item: &hir::TraitItem<'_>) {
        self.observe_item(trait_item.def_id);
    }

    fn visit_impl_item(&mut self, impl_item: &hir::ImplItem<'_>) {
        self.observe_item(impl_item.def_id);
    }

    fn visit_foreign_item(&mut self, foreign_item: &hir::ForeignItem<'_>) {
        self.observe_item(foreign_item.def_id);
    }
}

impl<'tcx> DiagnosticItemCollector<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> DiagnosticItemCollector<'tcx> {
        DiagnosticItemCollector { tcx, items: Default::default() }
    }

    fn observe_item(&mut self, def_id: LocalDefId) {
        let hir_id = self.tcx.hir().local_def_id_to_hir_id(def_id);
        let attrs = self.tcx.hir().attrs(hir_id);
        if let Some(name) = extract(&self.tcx.sess, attrs) {
            // insert into our table
            collect_item(self.tcx, &mut self.items, name, def_id.to_def_id());
        }
    }
}

fn collect_item(
    tcx: TyCtxt<'_>,
    items: &mut FxHashMap<Symbol, DefId>,
    name: Symbol,
    item_def_id: DefId,
) {
    // Check for duplicates.
    if let Some(original_def_id) = items.insert(name, item_def_id) {
        if original_def_id != item_def_id {
            let mut err = match tcx.hir().span_if_local(item_def_id) {
                Some(span) => tcx.sess.struct_span_err(
                    span,
                    &format!("duplicate diagnostic item found: `{}`.", name),
                ),
                None => tcx.sess.struct_err(&format!(
                    "duplicate diagnostic item in crate `{}`: `{}`.",
                    tcx.crate_name(item_def_id.krate),
                    name
                )),
            };
            if let Some(span) = tcx.hir().span_if_local(original_def_id) {
                err.span_note(span, "the diagnostic item is first defined here");
            } else {
                err.note(&format!(
                    "the diagnostic item is first defined in crate `{}`.",
                    tcx.crate_name(original_def_id.krate)
                ));
            }
            err.emit();
        }
    }
}

/// Extract the first `rustc_diagnostic_item = "$name"` out of a list of attributes.
fn extract(sess: &Session, attrs: &[ast::Attribute]) -> Option<Symbol> {
    attrs.iter().find_map(|attr| {
        if sess.check_name(attr, sym::rustc_diagnostic_item) {
            attr.value_str()
        } else {
            None
        }
    })
}

/// Traverse and collect the diagnostic items in the current
fn diagnostic_items<'tcx>(tcx: TyCtxt<'tcx>, cnum: CrateNum) -> FxHashMap<Symbol, DefId> {
    assert_eq!(cnum, LOCAL_CRATE);

    // Initialize the collector.
    let mut collector = DiagnosticItemCollector::new(tcx);

    // Collect diagnostic items in this crate.
    tcx.hir().krate().visit_all_item_likes(&mut collector);

    for m in tcx.hir().krate().exported_macros {
        collector.observe_item(m.def_id);
    }

    collector.items
}

/// Traverse and collect all the diagnostic items in all crates.
fn all_diagnostic_items<'tcx>(tcx: TyCtxt<'tcx>, (): ()) -> FxHashMap<Symbol, DefId> {
    // Initialize the collector.
    let mut collector = FxHashMap::default();

    // Collect diagnostic items in other crates.
    for &cnum in tcx.crates().iter().chain(std::iter::once(&LOCAL_CRATE)) {
        for (&name, &def_id) in tcx.diagnostic_items(cnum).iter() {
            collect_item(tcx, &mut collector, name, def_id);
        }
    }

    collector
}

pub fn provide(providers: &mut Providers) {
    providers.diagnostic_items = diagnostic_items;
    providers.all_diagnostic_items = all_diagnostic_items;
}
