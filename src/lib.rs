mod ir;
mod names;
mod tokens;
mod trace;

pub use proc_macro2;
pub use crate::ir::Ir;

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
        let nodes: Stmts = syn::parse2(self.nodes).map_err(|_| Error)?;
        let ids: Stmts = syn::parse2(self.ids).map_err(|_| Error)?;
        Ok(Pattern{ nodes: nodes.0, ids: ids.0 })
    }
}

pub struct Pattern{ nodes: Vec<syn::Stmt>, ids: Vec<syn::Stmt> }
impl Pattern {
    pub fn compile(&self) -> Ir {
        ir::compile(&self.nodes, &self.ids)
    }

    pub fn debug_tree_repr(&self) -> String {
        ir::debug_tree_repr(&self.nodes, &self.ids)
    }

    pub fn debug_flat_repr(&self) -> String {
        ir::debug_flat_repr(&self.nodes, &self.ids)
    }
}

impl Display for PatternDef {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        write!(fmt, "PatternDef {{ nodes: {}, ids: {} }}", &self.nodes, &self.ids)
    }
}
