/// A TokenStream can represent:
/// 1: standard syntax
/// 2: a Macros 2.0-style metavars declaration
/// 3: (1) extended with metavars declared in a separate stream of type (2)
/// 4: (1) extended with self-describing metavars
/// This module supports using (2) + (3) to produce (4).
use proc_macro2::{TokenStream, TokenTree};

use log::trace;
use std::collections::HashMap;
use std::num::NonZeroU32;

#[derive(Debug, PartialEq, Eq)]
enum NodeType {
    Ident,
    Expr,
    // ...
}

impl NodeType {
    fn to_token(&self) -> TokenTree {
        let id = match *self {
            self::NodeType::Ident => '@',
            self::NodeType::Expr => '*',
        };
        //proc_macro2::Ident::new(id, proc_macro2::Span::call_site()).into()
        proc_macro2::Punct::new(id, proc_macro2::Spacing::Alone).into()
    }
}

#[derive(Debug)]
struct Id {
    n: NonZeroU32,
}

impl Id {
    fn to_token(&self) -> TokenTree {
        proc_macro2::Literal::u32_unsuffixed(self.n.get()).into()
    }
}

#[derive(Debug)]
struct MetaDef {
    /// AST type for parser
    node: NodeType,
    /// unique identifier for each metavar
    id: Id,
}

type DefMap = HashMap<String, MetaDef>;

#[derive(Debug)]
enum MacBodyState {
    AwaitingDollar,
    AwaitingIdent,
    Cont(TokenTree),
}

struct MacBodyTransducer<'a, It> {
    ts: It,
    defs: &'a DefMap,
    state: MacBodyState,
}

impl<'a, It> MacBodyTransducer<'a, It> {
    fn new(ts: It, defs: &'a DefMap) -> Self {
        let state = MacBodyState::AwaitingDollar;
        MacBodyTransducer { ts, defs, state }
    }
}

// fast flat state machine
// or simply spew out vectors
// generators plz?

impl<'a, It: Iterator<Item = TokenTree>> Iterator for MacBodyTransducer<'a, It> {
    type Item = TokenTree;

    fn next(&mut self) -> Option<TokenTree> {
        use self::MacBodyState::*;
        use proc_macro2::TokenTree::*;
        // better way to do this? (replace value if match)
        if let Cont(_) = self.state {
            if let Cont(tt) = std::mem::replace(&mut self.state, AwaitingDollar) {
                return Some(tt);
            }
            unreachable!();
        }
        let tt = self.ts.next();
        match (&self.state, tt) {
            (AwaitingDollar, Some(Punct(ref c))) if c.as_char() == '$' => {
                self.state = AwaitingIdent;
                Some(Punct(c.clone()))
            }
            (AwaitingDollar, x) => x,
            (AwaitingIdent, Some(Ident(id))) => {
                self.state = AwaitingDollar;
                match self.defs.get(&id.to_string()) {
                    Some(MetaDef { node, id }) => {
                        if *node == self::NodeType::Ident {
                            // shoehorn into an magic identifier until I finish wrapping syn's
                            // Ident in an enum
                            self.state = AwaitingDollar;
                            Some(syn::Ident::new(&format!("__COMACRO_{}", id.n.get()), proc_macro2::Span::call_site()).into())
                        } else {
                            let (type_tok, id_tok) = (node.to_token(), id.to_token());
                            self.state = Cont(id_tok);
                            Some(type_tok)
                        }
                    }
                    None => panic!(),
                }
            }
            (AwaitingIdent, Some(Punct(ref c))) if c.as_char() == '_' => {
                self.state = AwaitingDollar;
                Some(Punct(c.clone()))
            }
            (AwaitingIdent, Some(Punct(ref c))) if c.as_char() == '$' => {
                self.state = AwaitingDollar;
                self.next()
            }
            (AwaitingIdent, _) => {
                panic!("macro body parse failure: after '$', expected one of: identifier, '_', '$'")
            }
            (Cont(_), _) => unreachable!(),
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
            id: Id {
                n: NonZeroU32::new((args.len() + 1) as u32).unwrap(),
            },
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

    /// Embed the info from the context into the TS to make the declared type info accessible to
    /// the metasyn parser.
    /// ($foo:expr, $bar:ident) { let $bar: $_ = $foo }
    /// =>
    /// { let $I 2: $_ = $X 1 }
    pub fn apply(&self, ts: TokenStream) -> TokenStream {
        let ducer = MacBodyTransducer::new(ts.into_iter(), &self.bindings);
        ducer.collect()
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
