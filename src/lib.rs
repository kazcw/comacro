mod ast;
mod tokens;
mod trace;

pub use proc_macro2;

pub use crate::ast::compile_input;

use crate::trace::Trace;

use crate::tokens::MetaContext;
use log::trace;
use proc_macro2::TokenStream;
use std::fmt::{Display, Formatter};
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

struct Stmts(Vec<syn::Stmt>);
impl Parse for Stmts {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        Ok(Stmts(input.call(syn::Block::parse_within)?))
    }
}

pub struct PatternDef {
    nodes: TokenStream,
    ids: TokenStream,
}
impl PatternDef {
    pub fn lex(args: TokenStream, body: TokenStream) -> PatternDef {
        let (nodes, ids) = MetaContext::new(args).apply(body);
        PatternDef { nodes, ids }
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
                if let (syn::Stmt::Expr(nodes), syn::Stmt::Expr(ids)) =
                    (nodes.remove(0), ids.remove(0))
                {
                    return Ok(Pattern::Expr { nodes, ids });
                }
                unreachable!();
            }
        }
        Ok(Pattern::StmtSeq { nodes, ids })
    }
}

pub enum Pattern {
    StmtSeq {
        nodes: Vec<syn::Stmt>,
        ids: Vec<syn::Stmt>,
    },
    Expr {
        nodes: syn::Expr,
        ids: syn::Expr,
    },
}

pub enum Ir {
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
                    context.push_str(&crate::ast::stmt_repr(s));
                }
                if !first {
                    context.push_str(",");
                }
                context.push_str("\"$1\"");
                for s in &self.input[m + self.pattern.toplevel_len()..] {
                    context.push_str(",");
                    context.push_str(&crate::ast::stmt_repr(s));
                }
                context.push_str("]");
                let bindings = crate::ast::bind_stmts(
                    self.pattern,
                    &self.input[m..m + self.pattern.toplevel_len()],
                );
                let bindings = crate::ast::bindings_repr(&bindings);
                Match { context, bindings }
            }),
            MatchesInner::Expr { matches } => matches.next().map(|m| {
                let context = crate::ast::stmts_tree_repr_of(m.clone(), self.input);
                let extracted =
                    crate::ast::bind_expr(crate::trace::ReTracer::new(m), self.input);
                let ex = if let crate::ast::Binding::Expr(ex) = extracted.binds[0] {
                    ex
                } else {
                    unreachable!()
                };
                let bindings = crate::ast::bind_expr_expr(
                    crate::trace::ReTracer::new(self.pattern.clone()),
                    ex,
                );
                let bindings = crate::ast::bindings_repr(&bindings);
                Match { context, bindings }
            }),
        }
    }
}

impl Ir {
    pub fn matches<'p, 'i, 'it>(
        &'p self,
        input: &'i [syn::Stmt],
        input_trace: &'it crate::trace::IndexedTrace,
    ) -> Matches<'p, 'i, 'it> {
        let (inner, pattern) = match self {
            Ir::StmtSeq { trace } => (
                MatchesInner::StmtSeq {
                    matches: trace.toplevel_matches(input_trace),
                },
                trace,
            ),
            Ir::Expr { trace } => (
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

    pub fn compile(&self) -> crate::trace::IndexedTrace {
        ast::compile_input(&self.stmts)
    }

    pub fn debug_tree_repr(&self) -> String {
        ast::stmts_tree_repr_of(ast::compile_input(&self.stmts).deindex(), &self.stmts)
    }
}

impl Pattern {
    pub fn compile(&self) -> Ir {
        match self {
            Pattern::StmtSeq { nodes, ids } => Ir::StmtSeq {
                trace: ast::compile_stmts(nodes, ids),
            },
            Pattern::Expr { nodes, ids } => Ir::Expr {
                trace: ast::compile_expr(nodes, ids),
            },
        }
    }

    pub fn debug_tree_repr(&self) -> String {
        match self {
            Pattern::StmtSeq { nodes, ids } => ast::stmts_tree_repr(nodes, ids),
            Pattern::Expr { nodes, ids } => ast::expr_tree_repr(nodes, ids),
        }
    }

    pub fn debug_flat_repr(&self) -> String {
        match self {
            Pattern::StmtSeq { nodes, ids } => ast::stmts_flat_repr(nodes, ids),
            Pattern::Expr { nodes, ids } => ast::expr_flat_repr(nodes, ids),
        }
    }

    pub fn fragment(&self) -> String {
        match self {
            Pattern::StmtSeq { .. } => "StmtSeq".to_owned(),
            Pattern::Expr { .. } => "Expr".to_owned(),
        }
    }
}

impl Display for PatternDef {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "PatternDef {{ nodes: {}, ids: {} }}",
            &self.nodes, &self.ids
        )
    }
}
