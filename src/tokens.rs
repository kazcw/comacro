/// A TokenStream can represent:
/// 1: standard syntax
/// 2: a Macros 2.0-style metavars declaration
/// 3: (1) extended with metavars declared in a separate stream of type (2)
/// 4: (1) extended with self-describing metavars
/// This module supports using (2) + (3) to produce (4).
use proc_macro2::{TokenStream, TokenTree};

use log::trace;
use std::collections::HashMap;

type DefMap = HashMap<String, MetaDef>;

#[derive(Debug)]
enum MacBodyState {
    AwaitingDollar,
    AwaitingIdent,
    Cont,
}

struct MacBodyTransducer<'a, It, F> {
    ts: It,
    defs: &'a DefMap,
    state: MacBodyState,
    cont: Vec<TokenTree>,
    tokenize: &'a F,
}

impl<'a, It, F> MacBodyTransducer<'a, It, F> {
    fn new(ts: It, defs: &'a DefMap, tokenize: &'a F) -> Self {
        let state = MacBodyState::AwaitingDollar;
        let cont = Vec::new();
        MacBodyTransducer {
            ts,
            defs,
            state,
            cont,
            tokenize,
        }
    }
}

#[derive(Debug)]
pub enum NodeType {
    Expr,
    Ident,
}

#[derive(Debug)]
struct MetaDef {
    node: NodeType,
    id: u32,
}

impl MetaDef {
    // placeholder token that must:
    // - parse as the right syntactic type
    // - always differ from the corresponding id_token
    fn node_token(&self) -> TokenTree {
        match self.node {
            self::NodeType::Ident => {
                syn::Ident::new("__IDENT", proc_macro2::Span::call_site()).into()
            }
            self::NodeType::Expr => {
                syn::Ident::new("__EXPR", proc_macro2::Span::call_site()).into()
            }
        }
    }

    // placeholder token that must:
    // - always differ from the corresponding node_token
    // - parse as the right syntactic type
    // - encode a mvar id
    fn id_token(&self) -> TokenTree {
        match self.node {
            self::NodeType::Ident => syn::Ident::new(
                &format!("IDENT_{}", self.id),
                proc_macro2::Span::call_site(),
            ).into(),
            //self::NodeType::Expr => proc_macro2::Literal::u32_suffixed(self.id).into()
            self::NodeType::Expr => {
                syn::Ident::new(&format!("EXPR_{}", self.id), proc_macro2::Span::call_site()).into()
            }
        }
    }
}

impl<'a, It: Iterator<Item = TokenTree>, F: Fn(&MetaDef) -> TokenTree> Iterator
    for MacBodyTransducer<'a, It, F>
{
    type Item = TokenTree;

    fn next(&mut self) -> Option<TokenTree> {
        use self::MacBodyState::*;
        use proc_macro2::TokenTree::*;
        if let Cont = self.state {
            if self.cont.is_empty() {
                self.state = AwaitingDollar;
            } else {
                return self.cont.pop();
            }
        }
        let tt = self.ts.next();
        match (&self.state, tt) {
            (AwaitingDollar, Some(Punct(ref c))) if c.as_char() == '$' => {
                self.state = AwaitingIdent;
                self.next()
            }
            (AwaitingDollar, Some(Group(ref g))) => {
                let delim = g.delimiter();
                let ts = MacBodyTransducer::new(g.stream().into_iter(), self.defs, self.tokenize)
                    .collect();
                Some(proc_macro2::Group::new(delim, ts).into())
            }
            (AwaitingDollar, x) => x,
            (AwaitingIdent, Some(Ident(id))) => {
                self.state = AwaitingDollar;
                Some((self.tokenize)(&self.defs[&id.to_string()]))
            }
            (AwaitingIdent, Some(Punct(ref c))) if c.as_char() == '$' => {
                self.state = AwaitingDollar;
                Some(Punct(c.clone()))
            }
            (AwaitingIdent, _) => {
                panic!("macro body parse failure: after '$', expected one of: identifier, '$'")
            }
            (Cont, _) => unreachable!("Cont handled before advancing ts"),
        }
    }
}

pub struct MetaContext {
    bindings: DefMap,
}

fn parse_args(ts: TokenStream) -> DefMap {
    let mut ts = ts.into_iter();
    let mut args = HashMap::new();
    loop {
        use proc_macro2::TokenTree::*;
        match ts.next() {
            Some(Punct(ref c)) if c.as_char() == '$' => (),
            None => break,
            _ => panic!(),
        }
        let id = match ts.next() {
            Some(Ident(id)) => id.to_string(),
            _ => panic!(),
        };
        match ts.next() {
            Some(Punct(ref c)) if c.as_char() == ':' => (),
            _ => panic!(),
        }
        let node = match ts.next() {
            Some(Ident(typ)) => typ,
            _ => panic!(),
        };
        let node = match node.to_string().as_ref() {
            "ident" => self::NodeType::Ident,
            "expr" => self::NodeType::Expr,
            _ => panic!(),
        };
        let def = MetaDef {
            node,
            id: (args.len() + 1) as u32,
        };
        let prev_def = args.insert(id, def);
        assert!(prev_def.is_none());
        match ts.next() {
            Some(Punct(ref c)) if c.as_char() == ',' => (),
            None => break,
            _ => panic!(),
        }
    }
    args
}

impl MetaContext {
    /// A macro's args declaration creates a context associating AST types with metavar names.
    pub fn new(ts: TokenStream) -> Self {
        let bindings = parse_args(ts);
        trace!("bindings={:?}", bindings);
        MetaContext { bindings }
    }

    pub fn apply(&self, ts: TokenStream) -> (TokenStream, TokenStream) {
        let nodes =
            MacBodyTransducer::new(ts.clone().into_iter(), &self.bindings, &MetaDef::node_token);
        let ids = MacBodyTransducer::new(ts.into_iter(), &self.bindings, &MetaDef::id_token);
        (nodes.collect(), ids.collect())
    }
}

// for ergonomics, need to support $_ at least when its unambiguously parsable
//let $baz: $_ = $foo;

// macro type system
/*
item
block
stmt
pat
expr
ty
ident
path
lifetime
meta: the contents of an attribute
tt: a token tree (a single token by matching (), [], or {})
*/

// equivalence classes are more composable than backrefs
