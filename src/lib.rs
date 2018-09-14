mod ir;
mod names;
mod tokens;

pub use proc_macro2;
pub use crate::ir::Ir;

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

pub struct PatternDef(TokenStream);
impl PatternDef {
    pub fn lex(args: TokenStream, body: TokenStream) -> PatternDef {
        PatternDef(MetaContext::new(args).apply(body))
    }

    pub fn parse(self) -> Result<Pattern> {
        let metasyn: Stmts = syn::parse2(self.0).map_err(|e| {
            Error/*{
                message: e.to_string(),
                location: Some((e.span().start(), e.span().end())),
            }*/
        })?;
        Ok(Pattern(metasyn.0))
    }
}

pub struct Pattern(Vec<syn::Stmt>);
impl Pattern {
    pub fn compile(&self) -> Ir {
        ir::compile(&self.0)
    }

    pub fn debug_tree_repr(&self) -> String {
        ir::debug_tree_repr(&self.0)
    }

    pub fn debug_flat_repr(&self) -> String {
        ir::debug_flat_repr(&self.0)
    }
}

impl Display for PatternDef {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        write!(fmt, "{}", &self.0)
    }
}
