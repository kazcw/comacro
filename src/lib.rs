mod matchcode;
mod names;
mod repr;
mod tokens;
mod trace;
mod visitor;

pub use proc_macro2;

pub use crate::matchcode::compile_input;

use crate::trace::Trace;

use log::trace;
use proc_macro2::TokenStream;
use syn::parse::Parse;
use crate::tokens::MetaContext;
use std::fmt::{Display, Formatter};

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

struct Stmts(Vec<syn::Stmt>);
impl Parse for Stmts {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        Ok(Stmts(input.call(syn::Block::parse_within)?))
    }
}

pub struct PatternDef{ nodes: TokenStream, ids: TokenStream }
impl PatternDef {
    pub fn lex(args: TokenStream, body: TokenStream) -> PatternDef {
        let (nodes, ids) = MetaContext::new(args).apply(body);
        PatternDef{ nodes, ids }
    }

    pub fn parse(self) -> Result<Pattern> {
        trace!("nodes: {}", &self.nodes);
        trace!("ids: {}", &self.ids);
        /*{
            message: e.to_string(),
            location: Some((e.span().start(), e.span().end())),
        }*/
        // parse as a sequence of statements
        let nodes: Stmts = syn::parse2(self.nodes).map_err(|_| Error)?;
        let ids: Stmts = syn::parse2(self.ids).expect("if <nodes> succeeded <ids> must");
        let (mut nodes, mut ids) = (nodes.0, ids.0);
        if nodes.len() == 1 {
            if let syn::Stmt::Expr(_) = nodes[0] {
                if let (syn::Stmt::Expr(nodes), syn::Stmt::Expr(ids)) = (nodes.remove(0), ids.remove(0)) {
                    return Ok(Pattern::Expr{ nodes, ids });
                }
                unreachable!();
            }
        }
        Ok(Pattern::StmtSeq{ nodes, ids })
    }
}

pub enum Pattern {
    StmtSeq { nodes: Vec<syn::Stmt>, ids: Vec<syn::Stmt> },
    Expr { nodes: syn::Expr, ids: syn::Expr },
}

pub enum Ir { 
    StmtSeq { trace: Trace },
    Expr { trace: Trace },
}

enum MatchesInner<'p, 'it> {
    StmtSeq { matches: crate::trace::ToplevelMatches<'p, 'it> },
    Expr { matches: crate::trace::InternalMatches<'p, 'it> },
}

#[derive(Debug)]
pub struct Match {
    context: String,
    bindings: String,
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
                    context.push_str(&crate::matchcode::stmt_repr(s));
                }
                if !first { context.push_str(","); }
                context.push_str("$1");
                for s in &self.input[m+self.pattern.toplevel_len()..] {
                    context.push_str(",");
                    context.push_str(&crate::matchcode::stmt_repr(s));
                }
                let bindings = crate::matchcode::bind_stmts(self.pattern, &self.input[m..m + self.pattern.toplevel_len()]);
                let bindings = crate::matchcode::bindings_repr(&bindings);
                Match { context, bindings }
            }),
            MatchesInner::Expr { matches } => matches.next().map(|m| {
                let context = crate::matchcode::stmts_tree_repr_of(m.clone(), self.input);
                let extracted = crate::matchcode::bind_expr(crate::trace::ReTracer::new(m), self.input);
                let ex = if let crate::matchcode::Binding::Expr(ex) = extracted.binds[0] {
                    ex
                } else {
                    unreachable!()
                };
                let bindings = crate::matchcode::bind_expr_expr(crate::trace::ReTracer::new(self.pattern.clone()), ex);
                let bindings = crate::matchcode::bindings_repr(&bindings);
                Match { context, bindings }
            }),
        }
    }
}

impl Ir {
    pub fn matches<'p, 'i, 'it>(&'p self, input: &'i [syn::Stmt], input_trace: &'it crate::trace::IndexedTrace) -> Matches<'p, 'i, 'it> {
        let (inner, pattern) = match self {
            Ir::StmtSeq { trace } => {
                (MatchesInner::StmtSeq { matches: trace.toplevel_matches(input_trace) }, trace)
            }
            Ir::Expr { trace } => {
                (MatchesInner::Expr { matches: trace.internal_matches(input_trace) }, trace)
            }
        };
        Matches { inner, pattern, input }
    }
}

impl Pattern {
    pub fn compile(&self) -> Ir {
        match self {
            Pattern::StmtSeq { nodes, ids } => Ir::StmtSeq { trace: matchcode::compile_stmts(nodes, ids) },
            Pattern::Expr { nodes, ids } => Ir::Expr { trace: matchcode::compile_expr(nodes, ids) },
        }
    }

    pub fn debug_tree_repr(&self) -> String {
        match self {
            Pattern::StmtSeq { nodes, ids } => matchcode::stmts_tree_repr(nodes, ids),
            Pattern::Expr { nodes, ids } => matchcode::expr_tree_repr(nodes, ids),
        }
    }

    pub fn debug_flat_repr(&self) -> String {
        match self {
            Pattern::StmtSeq { nodes, ids } => matchcode::stmts_flat_repr(nodes, ids),
            Pattern::Expr { nodes, ids } => matchcode::expr_flat_repr(nodes, ids),
        }
    }
}

impl Display for PatternDef {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        write!(fmt, "PatternDef {{ nodes: {}, ids: {} }}", &self.nodes, &self.ids)
    }
}
