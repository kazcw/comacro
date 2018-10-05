/// Lower metasyn AST to regex-matchable IR

#[allow(unused)]
use log::trace;
use syn::visit::Visit;

use crate::repr::*;
use crate::trace::*;
use crate::visitor::Visitor as InnerVisitor;

pub struct MatchCode {
    // A trace of selected substructure of an AST traversal.
    // Untyped, binary, all data inline. Contains just enough info to locate differences.
    buf: Vec<u8>,
}
pub type Ir = MatchCode;

// escape all pseudo-Metas
// length-prefix varlens (Datum) so they compare correctly
// no distinction between parsed/unparsed data--everything is quoted anyway
// Meta is for wildcard-matchable subtrees

/*
 * matching:
 *
 * - naive interpretation: parse, keeping Subtree stack
 * - faster: exact comparison with pattern; on diff, fail if pattern is not at wildcard; if pattern
 *   is at wildcard, mismatched item must be a subgraph opener
 *   - knowing if a location is a wildcard:
 *     - keep a subtree stack as we go
 *     - use a mvar table
 *     * escape pseudo-mvars in Datums, lengths, discriminants
 *
 */

// Ast nodes -> Mc nodes
// Ast ids -> Mc ids
//
// Mc nodes -> Mc ids -> Mc pattern [reconcile]
//
// TreeRepr visitor: Mc pattern -> Ast nodes
// - count along Ast's position in Mc instead of emitting
// - open_subtree checks if pattern is at a mvar position to skip visit

struct Visitor<V> {
    inner: V,
}

#[derive(Default)]
struct MatchCodeGenerator { trace: Trace }
impl InnerVisitor for MatchCodeGenerator {
    fn open_subtree(&mut self) { self.trace.open_subtree(); }
    fn close_subtree(&mut self) { self.trace.close_subtree(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

struct Reconciler { trace: DeltaTrace }
impl InnerVisitor for Reconciler {
    fn close_ident(&mut self, x: &syn::Ident) {
        if let Ok(()) = self.trace.close_subtree() { return; }
        let x = x.to_string();
        let x_id = x.trim_left_matches("IDENT_");
        let x_expr = x.trim_left_matches("EXPR_");
        if x_id != x {
            // replace current
            self.trace.new.replacement.push_mvar(u8::from_str_radix(x_id, 10).unwrap());
            self.trace.new.rollback(0);
            self.trace.new.close_subtree();
        } else if x_expr != x {
            // replace parent
            self.trace.new.replacement.push_mvar(u8::from_str_radix(x_expr, 10).unwrap());
            self.trace.new.rollback(1);
            self.trace.new.close_subtree();
        } else {
            unreachable!("no such placeholder");
        }
    }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

/*
struct Binder { trace: ReTrace }
impl InnerVisitor for Binder {
    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> { unimplemented!() }
    fn open_ident(&mut self, x: &syn::Ident) -> Result<(), ()> { unimplemented!() }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}
*/

/// Outer visitor ensures AST->MatchCode lowering is consistent for different traversal modes
impl<'ast, V: InnerVisitor> Visit<'ast> for Visitor<V> {
    fn visit_stmt(&mut self, x: &syn::Stmt) {
        self.inner.open_stmt(x);
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_stmt(self, x);
        self.inner.close_stmt(x);
    }

    fn visit_expr(&mut self, x: &syn::Expr) {
        if let Err(()) = self.inner.open_expr(x) { return; }
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_expr(self, x);
        self.inner.close_expr(x);
    }

    fn visit_pat(&mut self, x: &syn::Pat) {
        self.inner.open_pat(x);
        syn::visit::visit_pat(self, x);
        self.inner.close_pat(x);
    }

    fn visit_ident(&mut self, x: &syn::Ident) {
        if let Err(()) = self.inner.open_ident(x) { return; }
        self.inner.extend_bytes(x.to_string().as_bytes());
        self.inner.close_ident(x);
    }

    fn visit_lit_int(&mut self, x: &syn::LitInt) {
        self.inner.open_lit_int(x);
        // TODO: compact repr?
        self.inner.extend_bytes(format!("{}", x.value()).as_bytes());
        self.inner.close_lit_int(x);
    }
}

pub fn compile(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> MatchCode {
    let mut nodeviz = Visitor { inner: MatchCodeGenerator::default() };
    nodes.iter().for_each(|s| nodeviz.visit_stmt(s));
    trace!("nds: {:?}", nodeviz.inner.trace);
    {
        let mut idviz = Visitor { inner: MatchCodeGenerator::default() };
        ids.iter().for_each(|s| idviz.visit_stmt(s));
        trace!("ids: {:?}", idviz.inner.trace);
    }
    let mut idviz = Visitor { inner: Reconciler { trace: DeltaTrace::new(nodeviz.inner.trace.finish()) } };
    ids.iter().for_each(|s| idviz.visit_stmt(s));
    //trace!("compiled: {:?}", idviz.inner.trace.new.trace);
    //MatchCode { buf: idviz.inner.trace.new.trace.buf }
    MatchCode { buf: idviz.inner.trace.finish() }
}

pub fn debug_tree_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc.buf, JsonEmitter::new()) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.finish()).unwrap()
}

pub fn debug_flat_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc.buf, ReprEmitter::new()) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.finish()).unwrap()
}
