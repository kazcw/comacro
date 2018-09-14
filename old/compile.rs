//! Compile comacro syntax to VM ops

use quote::ToTokens;
use syn::parse::Parse;
use syn::{File, Item, ItemMacro2};

use log::{debug, error, info, log, trace};
use std::collections::HashMap;

/// find-or-insert the T into the (unsorted) Vec, and then return its index
fn smallset_index<T: PartialEq + Clone>(v: &mut Vec<T>, t: &T) -> usize {
    match v.iter().position(|x| x == t) {
        Some(n) => n,
        None => {
            v.push(t.clone());
            v.len() - 1
        }
    }
}

/// TODO: str/String shouldn't need a special case
fn smallset_str_index(v: &mut Vec<String>, t: &str) -> usize {
    match v.iter().position(|x| x == t) {
        Some(n) => n,
        None => {
            v.push(t.to_owned());
            v.len() - 1
        }
    }
}

/*
            (Local(l0), Local(l1)) => {
                // TODO: support metavars inside patterns
                if l0.pats.len() == 1 {
                    if let syn::Pat::Ident(ref id) = l0.pats[0] {
                        let syn::PatIdent { ident: id, .. } = id;
                        let id = format!("{}", id);
                        let trimmed = id.trim_left_matches("__PLACEHOLDER_IDENT_");
                        if trimmed != id {
                            smallset_str_index(&mut idents, &trimmed);
                        } else {
                            trace!("non-match: local: patterns");
                            return;
                        }
                    } else {
                        if l0.pats != l1.pats {
                            trace!("non-match: local: patterns");
                            return;
                        }
                    }
                } else {
                    if l0.pats != l1.pats {
                        trace!("non-match: local: patterns");
                        return;
                    }
                }
                if l0.init != l1.init {
                    if let Some((_, box syn::Expr::Path(syn::ExprPath { ref path, .. }))) =
                        l0.init
                    {
                        let segs = &path.segments;
                        if segs.len() == 1 {
                            // TODO: arguments?
                            let syn::PathSegment { ref ident, .. } = segs[0];
                            let id = format!("{}", ident);
                            // TODO: an IDENT metavar can also be an expression
                            let trimmed = id.trim_left_matches("__PLACEHOLDER_EXPR_");
                            if trimmed != id {
                                smallset_str_index(&mut exprs, &trimmed);
                            } else {
                                trace!("non-match: local: patterns");
                                return;
                            }
                        } else {
                            trace!("non-match: local: patterns");
                            return;
                        }
                    } else {
                        trace!("non-match: local: patterns");
                        return;
                    }
                }
            }
            (Item(i0), Item(i1)) => unimplemented!(),
            (Expr(x0), Expr(x1)) => {
                if x0 == x1 {
                    continue;
                }
                // TODO: metavars
                trace!("non-match: expr");
                return;
            }
            (Semi(x0, _), Semi(x1, _)) => {
                if x0 == x1 {
                    continue;
                }
                // TODO: metavars
                trace!("non-match: semi expr");
                return;
            }
            (_, _) => {
                trace!("non-match: shape");
                return;
            }
*/

pub fn compile_pat(pat: &Item) -> crate::op::Script {
    use syn::Item::*;
    let mac = match pat {
        Macro2(m) => m,
        _ => unimplemented!("non-macro2 item in patterns"),
    };
    let ar = parse_args(mac.args.clone().into_iter());
    let body: TokenStream = MacBodyTransducer::new(mac.body.clone().into_iter(), ar).collect();
    trace!("MacBody: {}", body);
    let Stmts { body } = syn::parse2(body).unwrap();
    let mut exprs: Vec<String> = Vec::new();
    let mut idents: Vec<String> = Vec::new();
    for (pat, target) in self.body.iter().zip(stmts) {
        use syn::Stmt::*;
        match (pat, target) {
            Local(l) => {
            }
            Item(i) => unimplemented!(),
            Expr(x) => {
            }
            Semi(x0, _) => {
            }
        }
    }
}

