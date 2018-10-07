/// Lower metasyn AST to regex-matchable IR

#[allow(unused)]
use log::trace;
use syn::visit::Visit;

use crate::repr::*;
use crate::trace::*;
use crate::visitor::Visitor as InnerVisitor;

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
struct MatchCodeGenerator { trace: Tracer }
impl InnerVisitor<'_> for MatchCodeGenerator {
    fn open_subtree(&mut self) { self.trace.open_subtree(); }
    fn close_subtree(&mut self) { self.trace.close_subtree(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

struct Reconciler { trace: DeltaTracer }
impl InnerVisitor<'_> for Reconciler {
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

#[derive(Clone)]
pub enum Binding<'ast> {
    Expr(&'ast syn::Expr),
    Ident(&'ast syn::Ident),
}

pub struct Bindings<'ast> {
    pub binds: Vec<Binding<'ast>>,
}
struct Binder<'ast> {
    trace: ReTracer,
    bindings: Vec<Option<Binding<'ast>>>,
}
impl<'ast> Binder<'ast> {
    fn new(trace: ReTracer) -> Self {
        Binder { trace, bindings: Vec::new() }
    }

    fn finish(self) -> Bindings<'ast> {
        self.trace.finish();
        Bindings { binds: self.bindings.into_iter().map(|x| x.unwrap()).collect() }
    }
}

impl<'ast> InnerVisitor<'ast> for Binder<'ast> {
    fn open_expr(&mut self, expr: &'ast syn::Expr) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = usize::from(self.trace.consume_meta()) - 1;
            if x >= self.bindings.len() {
                self.bindings.resize(x + 1, None);
            }
            if let None = self.bindings[x] {
                self.bindings[x] = Some(Binding::Expr(expr));
            }
            return Err(());
        }
        Ok(())
    }
    fn open_ident(&mut self, ident: &'ast syn::Ident) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = usize::from(self.trace.consume_meta()) - 1;
            if x >= self.bindings.len() {
                self.bindings.resize(x + 1, None);
            }
            if let None = self.bindings[x] {
                self.bindings[x] = Some(Binding::Ident(ident));
            }
            return Err(());
        }
        Ok(())
    }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

/// Outer visitor ensures AST->Trace lowering is consistent for different traversal modes
impl<'ast, V: InnerVisitor<'ast>> Visit<'ast> for Visitor<V> {
    fn visit_stmt(&mut self, x: &'ast syn::Stmt) {
        self.inner.open_stmt(x);
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_stmt(self, x);
        self.inner.close_stmt(x);
    }

    fn visit_expr(&mut self, x: &'ast syn::Expr) {
        if let Err(()) = self.inner.open_expr(x) { return; }
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_expr(self, x);
        self.inner.close_expr(x);
    }

    fn visit_pat(&mut self, x: &'ast syn::Pat) {
        self.inner.open_pat(x);
        syn::visit::visit_pat(self, x);
        self.inner.close_pat(x);
    }

    fn visit_ident(&mut self, x: &'ast syn::Ident) {
        if let Err(()) = self.inner.open_ident(x) { return; }
        self.inner.extend_bytes(x.to_string().as_bytes());
        self.inner.close_ident(x);
    }

    fn visit_lit_int(&mut self, x: &'ast syn::LitInt) {
        self.inner.open_lit_int(x);
        // TODO: compact repr?
        self.inner.extend_bytes(format!("{}", x.value()).as_bytes());
        self.inner.close_lit_int(x);
    }
}

/// compile a statement-sequence search pattern
pub fn compile_stmts(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> Trace {
    let mut nodeviz = Visitor { inner: MatchCodeGenerator::default() };
    nodes.iter().for_each(|s| nodeviz.visit_stmt(s));
    let nodetrace = nodeviz.inner.trace.finish();
    trace!("nds: {:?}", nodetrace);
    let mut idviz = Visitor { inner: Reconciler { trace: DeltaTracer::new(nodetrace) } };
    ids.iter().for_each(|s| idviz.visit_stmt(s));
    //trace!("compiled: {:?}", idviz.inner.trace.new.trace);
    //Trace { buf: idviz.inner.trace.new.trace.buf }
    idviz.inner.trace.finish()
}

/// compile a subexpr search pattern
pub fn compile_expr(nodes: &syn::Expr, ids: &syn::Expr) -> Trace {
    let mut nodeviz = Visitor { inner: MatchCodeGenerator::default() };
    nodeviz.visit_expr(nodes);
    let nodetrace = nodeviz.inner.trace.finish();
    trace!("nds: {:?}", nodetrace);
    let mut idviz = Visitor { inner: Reconciler { trace: DeltaTracer::new(nodetrace) } };
    idviz.visit_expr(ids);
    //trace!("compiled: {:?}", idviz.inner.trace.new.trace);
    //Trace { buf: idviz.inner.trace.new.trace.buf }
    idviz.inner.trace.finish()
}

/// compile an input
pub fn compile_input(stmts: &[syn::Stmt]) -> IndexedTrace {
    let mut viz = Visitor { inner: MatchCodeGenerator::default() };
    viz.inner.trace.expect_marks(stmts.len());
    for s in stmts {
        viz.inner.trace.mark();
        viz.visit_stmt(s);
    }
    let trace = viz.inner.trace.finish_with_indexes();
    trace!("input: {:?}", trace);
    trace
}

pub fn stmts_tree_repr_of(trace: Trace, input: &[syn::Stmt]) -> String {
    let mut viz = Visitor { inner: ReprGenerator::new(trace, JsonEmitter::new()) };
    input.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.finish()).unwrap()
}

pub fn stmts_tree_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    stmts_tree_repr_of(compile_stmts(nodes, ids), nodes)
}

pub fn stmts_flat_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile_stmts(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc, ReprEmitter::new()) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.finish()).unwrap()
}

pub fn expr_tree_repr_of(trace: Trace, input: &syn::Expr) -> String {
    let mut viz = Visitor { inner: ReprGenerator::new(trace, JsonEmitter::new()) };
    viz.visit_expr(input);
    String::from_utf8(viz.inner.finish()).unwrap()
}

pub fn expr_tree_repr(nodes: &syn::Expr, ids: &syn::Expr) -> String {
    expr_tree_repr_of(compile_expr(nodes, ids), nodes)
}

pub fn expr_flat_repr(nodes: &syn::Expr, ids: &syn::Expr) -> String {
    let mc = compile_expr(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc, ReprEmitter::new()) };
    viz.visit_expr(nodes);
    String::from_utf8(viz.inner.finish()).unwrap()
}

pub fn bind_stmts<'a>(pat: &Trace, stmts: &'a [syn::Stmt]) -> Bindings<'a> {
    let trace = ReTracer::new(pat.clone());
    let mut viz = Visitor { inner: Binder::new(trace) };
    stmts.iter().for_each(|s| viz.visit_stmt(s));
    viz.inner.finish()
}

pub fn bind_expr(trace: ReTracer, stmts: &[syn::Stmt]) -> Bindings {
    let mut viz = Visitor { inner: Binder::new(trace) };
    stmts.iter().for_each(|s| viz.visit_stmt(s));
    viz.inner.finish()
}

pub fn bind_expr_expr(trace: ReTracer, expr: &syn::Expr) -> Bindings {
    let mut viz = Visitor { inner: Binder::new(trace) };
    viz.visit_expr(expr);
    viz.inner.finish()
}

pub fn bindings_repr(bindings: &'_ Bindings) -> String {
    let f = |b: &Binding| {
        match b {
            Binding::Ident(i) => {
                format!("[\"Ident\",\"{}\"]", i)
            }
            Binding::Expr(x) => {
                let mut viz = Visitor { inner: PlainAstRepr::new(JsonEmitter::new_scalar()) };
                viz.visit_expr(x);
                let s = String::from_utf8(viz.inner.finish()).unwrap();
                format!("[\"Expr\",{}]", s)
            }
        }
    };
    let mut buf = "[".to_owned();
    let mut binds = bindings.binds.iter();
    if let Some(b) = binds.next() {
        buf.push_str(&f(b));
    }
    for b in binds {
        buf.push_str(",");
        buf.push_str(&f(b));
    }
    buf.push_str("]");
    buf
}

pub fn stmt_repr(stmt: &syn::Stmt) -> String {
    let mut viz = Visitor { inner: PlainAstRepr::new(JsonEmitter::new_scalar()) };
    viz.visit_stmt(stmt);
    String::from_utf8(viz.inner.finish()).unwrap()
}

