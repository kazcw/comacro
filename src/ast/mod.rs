//! Operations on syn AST

mod names;
pub mod repr;
mod visit;

pub use self::visit::Visitor;

use crate::trace::*;

#[derive(Default)]
pub struct TraceGenerator {
    trace: Tracer,
}
impl Visitor<'_> for TraceGenerator {
    type Output = Trace;
    fn open_subtree(&mut self) {
        self.trace.open_subtree();
    }
    fn close_subtree(&mut self) {
        self.trace.close_subtree();
    }
    fn open_datum(&mut self) {
        self.trace.open_datum();
    }
    fn close_datum(&mut self) {
        self.trace.close_datum();
    }
    fn push_byte(&mut self, x: u8) {
        self.trace.push_byte(x);
    }
    fn extend_bytes(&mut self, x: &[u8]) {
        self.trace.extend_bytes(x);
    }
    fn finish(self) -> Self::Output {
        self.trace.finish()
    }
}

#[derive(Default)]
pub struct IndexedTraceGenerator {
    trace: Tracer,
}
impl IndexedTraceGenerator {
    pub fn with_hint(hint: usize) -> Self {
        let mut trace = Tracer::default();
        trace.expect_marks(hint);
        IndexedTraceGenerator { trace }
    }
}
impl Visitor<'_> for IndexedTraceGenerator {
    type Output = IndexedTrace;
    fn open_stmt(&mut self, _: &syn::Stmt) {
        self.trace.mark();
        self.trace.open_subtree();
    }
    fn open_subtree(&mut self) {
        self.trace.open_subtree();
    }
    fn close_subtree(&mut self) {
        self.trace.close_subtree();
    }
    fn open_datum(&mut self) {
        self.trace.open_datum();
    }
    fn close_datum(&mut self) {
        self.trace.close_datum();
    }
    fn push_byte(&mut self, x: u8) {
        self.trace.push_byte(x);
    }
    fn extend_bytes(&mut self, x: &[u8]) {
        self.trace.extend_bytes(x);
    }
    fn finish(self) -> Self::Output {
        self.trace.finish_with_indexes()
    }
}

pub struct Reconciler<'t> {
    trace: DeltaTracer<'t>,
}
impl<'t> Reconciler<'t> {
    pub fn new(trace: &'t Trace) -> Self {
        let trace = DeltaTracer::new(trace);
        Reconciler { trace }
    }
}
impl<'t, 'a> Visitor<'a> for Reconciler<'t> {
    type Output = Trace;
    fn close_ident(&mut self, x: &syn::Ident) {
        if let Ok(()) = self.trace.close_subtree() {
            return;
        }
        let x = x.to_string();
        let x_id = x.trim_left_matches("IDENT_");
        let x_expr = x.trim_left_matches("EXPR_");
        if x_id != x {
            // replace current
            self.trace
                .new
                .replacement
                .push_mvar(u8::from_str_radix(x_id, 10).unwrap());
            self.trace.new.rollback(0);
            self.trace.new.close_subtree();
        } else if x_expr != x {
            // replace parent
            self.trace
                .new
                .replacement
                .push_mvar(u8::from_str_radix(x_expr, 10).unwrap());
            self.trace.new.rollback(1);
            self.trace.new.close_subtree();
        } else {
            unreachable!("no such placeholder");
        }
    }

    fn open_subtree(&mut self) {
        self.trace.open_subtree().unwrap();
    }
    fn close_subtree(&mut self) {
        self.trace.close_subtree().unwrap();
    }
    fn open_datum(&mut self) {
        self.trace.open_datum();
    }
    fn close_datum(&mut self) {
        self.trace.close_datum();
    }
    fn push_byte(&mut self, x: u8) {
        self.trace.push_byte(x);
    }
    fn extend_bytes(&mut self, x: &[u8]) {
        self.trace.extend_bytes(x);
    }

    fn finish(self) -> Self::Output {
        self.trace.finish()
    }
}

#[derive(Clone)]
pub enum Binding<'ast> {
    Expr(&'ast syn::Expr),
    Ident(&'ast syn::Ident),
}

pub struct Bindings<'ast> {
    pub binds: Vec<Binding<'ast>>,
}
pub struct Binder<'ast, 't> {
    trace: ReTracer<'t>,
    bindings: Vec<Option<Binding<'ast>>>,
}
impl<'ast, 't> Binder<'ast, 't> {
    pub fn new(trace: ReTracer<'t>) -> Self {
        Binder {
            trace,
            bindings: Vec::new(),
        }
    }
}

impl<'ast, 't> Visitor<'ast> for Binder<'ast, 't> {
    type Output = Bindings<'ast>;

    fn open_expr(&mut self, expr: &'ast syn::Expr) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = usize::from(self.trace.consume_meta()) - 1;
            if x >= self.bindings.len() {
                self.bindings.resize(x + 1, None);
            }
            if self.bindings[x].is_none() {
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
            if self.bindings[x].is_none() {
                self.bindings[x] = Some(Binding::Ident(ident));
            }
            return Err(());
        }
        Ok(())
    }

    fn open_subtree(&mut self) {
        self.trace.open_subtree().unwrap();
    }
    fn close_subtree(&mut self) {
        self.trace.close_subtree().unwrap();
    }
    fn open_datum(&mut self) {
        self.trace.open_datum();
    }
    fn close_datum(&mut self) {
        self.trace.close_datum();
    }
    fn push_byte(&mut self, x: u8) {
        self.trace.push_byte(x);
    }
    fn extend_bytes(&mut self, x: &[u8]) {
        self.trace.extend_bytes(x);
    }

    fn finish(self) -> Self::Output {
        self.trace.finish();
        Bindings {
            binds: self.bindings.into_iter().map(|x| x.unwrap()).collect(),
        }
    }
}
