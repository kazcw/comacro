//! Operations on syn AST

mod names;
mod repr;
mod visit;

use self::visit::Visitor;

use crate::trace::*;
use crate::tokens::MetaContext;

use proc_macro2::TokenStream;
use syn::parse::Parse;

#[derive(Debug)]
pub struct Error;

/*/
pub struct Error{
    message: String,
    location: Option<(proc_macro2::LineColumn, proc_macro2::LineColumn)>,
}

impl Error {
    pub fn annotate(&self, src: &str) -> String {
        let start_line = self.location.as_ref().unwrap().0.line;
        let start_col = self.location.as_ref().unwrap().0.column;
        let src = src.lines().nth(start_line).expect("error location within source");
        let pointer = "^";
        format!("{}\n{}\n{:start_col$}", &self.message, &src, pointer, start_col = start_col)
    }
}
*/

pub type Result<T> = std::result::Result<T, Error>;


#[derive(Default)]
struct TraceGenerator {
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
struct IndexedTraceGenerator {
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

struct Reconciler<'t> {
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

    fn open_expr(&mut self, expr: &'ast syn::Expr) -> std::result::Result<(), ()> {
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
    fn open_ident(&mut self, ident: &'ast syn::Ident) -> std::result::Result<(), ()> {
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

struct Stmts(Vec<syn::Stmt>);
impl Parse for Stmts {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        Ok(Stmts(input.call(syn::Block::parse_within)?))
    }
}

/// A pattern definition.
pub enum PatternDef {
    StmtSeq {
        nodes: Vec<syn::Stmt>,
        ids: Vec<syn::Stmt>,
    },
    Expr {
        nodes: syn::Expr,
        ids: syn::Expr,
    },
}

impl PatternDef {
    pub fn parse(args: TokenStream, body: TokenStream) -> Result<Self> { 
        let (nodes, ids) = MetaContext::new(args).apply(body);
        let nodes: Stmts = syn::parse2(nodes).map_err(|_| Error)?;
        let ids: Stmts = syn::parse2(ids).expect("if <nodes> succeeded <ids> must");
        let (mut nodes, mut ids) = (nodes.0, ids.0);
        if nodes.len() == 1 {
            if let syn::Stmt::Expr(_) = nodes[0] {
                if let (syn::Stmt::Expr(nodes), syn::Stmt::Expr(ids)) =
                    (nodes.remove(0), ids.remove(0))
                {
                    return Ok(PatternDef::Expr { nodes, ids });
                }
                unreachable!();
            }
        }
        Ok(PatternDef::StmtSeq { nodes, ids })
    }
}

/// A compiled pattern.
pub enum Pattern {
    StmtSeq { trace: Trace },
    Expr { trace: Trace },
}

enum MatchesInner<'p, 'it> {
    StmtSeq {
        matches: crate::trace::ToplevelMatches<'p, 'it>,
    },
    Expr {
        matches: crate::trace::InternalMatches<'p, 'it>,
    },
}

#[derive(Debug)]
pub struct Match {
    pub context: String,
    pub bindings: String,
}

pub struct Matches<'p, 'i, 'it> {
    inner: MatchesInner<'p, 'it>,
    pattern: &'p Trace,
    input: &'i [syn::Stmt],
}

impl Iterator for Matches<'_, '_, '_> {
    type Item = Match;
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            MatchesInner::StmtSeq { matches } => matches.next().map(|m| {
                let mut context = "[".to_owned();
                let mut first = true;
                for s in &self.input[..m] {
                    if !first {
                        context.push_str(",");
                    }
                    first = false;
                    context.push_str(&repr::input_json(s));
                }
                if !first {
                    context.push_str(",");
                }
                context.push_str("\"$1\"");
                for s in &self.input[m + self.pattern.toplevel_len()..] {
                    context.push_str(",");
                    context.push_str(&repr::input_json(s));
                }
                context.push_str("]");
                let binder = Binder::new(ReTracer::new(self.pattern));
                let bindings = binder.visit(&self.input[m..m + self.pattern.toplevel_len()]);
                let bindings = repr::bindings_json(&bindings);
                Match { context, bindings }
            }),
            MatchesInner::Expr { matches } => matches.next().map(|m| {
                let context = repr::pattern_json(&m, self.input);
                let extracted = Binder::new(ReTracer::new(&m)).visit(self.input);
                let bindings = if let crate::ast::Binding::Expr(ex) = extracted.binds[0] {
                    Binder::new(ReTracer::new(&self.pattern)).visit(ex)
                } else {
                    unreachable!()
                };
                
                let bindings = repr::bindings_json(&bindings);
                Match { context, bindings }
            }),
        }
    }
}

impl Pattern {
    pub fn matches<'p, 'i, 'it>(
        &'p self,
        input: &'i [syn::Stmt],
        input_trace: &'it crate::trace::IndexedTrace,
    ) -> Matches<'p, 'i, 'it> {
        let (inner, pattern) = match self {
            Pattern::StmtSeq { trace } => (
                MatchesInner::StmtSeq {
                    matches: trace.toplevel_matches(input_trace),
                },
                trace,
            ),
            Pattern::Expr { trace } => (
                MatchesInner::Expr {
                    matches: trace.internal_matches(input_trace),
                },
                trace,
            ),
        };
        Matches {
            inner,
            pattern,
            input,
        }
    }
}

pub struct Input {
    pub stmts: Vec<syn::Stmt>,
}

impl Input {
    pub fn parse(ts: TokenStream) -> Result<Self> {
        let stmts: Stmts = syn::parse2(ts).map_err(|_| Error)?;
        let stmts = stmts.0;
        Ok(Input { stmts })
    }

    pub fn compile(&self) -> IndexedTrace {
        IndexedTraceGenerator::with_hint(self.stmts.len()).visit(&self.stmts[..])
    }

    pub fn debug_tree_repr(&self) -> String {
        repr::pattern_json(&self.compile().deindex(), &self.stmts[..])
    }
}

impl PatternDef {
    fn trace(&self) -> Trace {
        match self {
            PatternDef::StmtSeq { nodes, ids } => Reconciler::new(&TraceGenerator::apply(&nodes[..])).visit(&ids[..]),
            PatternDef::Expr { nodes, ids } => Reconciler::new(&TraceGenerator::apply(nodes)).visit(ids),
        }
    }

    pub fn compile(&self) -> Pattern {
        match self {
            PatternDef::StmtSeq { .. } => Pattern::StmtSeq { trace: self.trace() },
            PatternDef::Expr { .. } => Pattern::Expr { trace: self.trace() },
        }
    }

    pub fn debug_tree_repr(&self) -> String {
        match self {
            PatternDef::StmtSeq { ids, .. } => repr::pattern_json(&self.trace(), &ids[..]),
            PatternDef::Expr { ids, .. } => repr::pattern_json(&self.trace(), ids),
        }
    }

    pub fn debug_flat_repr(&self) -> String {
        match self {
            PatternDef::StmtSeq { ids, .. } => repr::pattern_flat(&self.trace(), &ids[..]),
            PatternDef::Expr { ids, .. } => repr::pattern_flat(&self.trace(), ids),
        }
    }

    pub fn fragment(&self) -> String {
        match self {
            PatternDef::StmtSeq { .. } => "StmtSeq".to_owned(),
            PatternDef::Expr { .. } => "Expr".to_owned(),
        }
    }
}
