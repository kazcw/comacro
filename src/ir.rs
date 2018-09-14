/// Lower metasyn AST to regex-matchable IR

use log::trace;
use syn::visit::Visit;

pub struct Ir {
    // A flattened tree. Can be created and compared without unpacking.
    buf: Vec<TreeByte>,
}

impl Ir {
    fn is_match(&self, rhs: &Ir) -> bool {
        unimplemented!()
    }
}

union TreeByte {
    node: Node,
    data: u8,
}

/*
use std::fmt::Debug;
impl Debug for Ir {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use self::Node::*;
        let buf = &self.buf[..];
        let mut stack = Vec::<usize>::new();
        let mut i = 0;
        loop {
            while let Some(&x) = stack.last() {
                if x == i {
                    stack.pop();
                    write!(f, "] ")?;
                } else {
                    break;
                }
            }
            if i == buf.len() {
                break;
            }
            // Non-node data only occurs inside a Terminal,
            // which we'll consume when we get to.
            let node = unsafe { buf[i].node };
            i += 1;
            match node {
                TyTok(tt) => {
                    write!(f, "{:?} ", tt)?;
                    trace!("TyTok({:?})", tt);
                },
                Enum(tt) => {
                    let discrim = usize::from(unsafe { buf[i].data });
                    i += 1;
                    write!(f, "{:?} ", tt)?;
                    trace!("{:?}<{:?}>", tt, discrim);
                },
                Subtree => {
                    // following byte is length
                    let len = usize::from(unsafe { buf[i].data });
                    i += 1;
                    write!(f, "[ ")?;
                    trace!("Subtree({:?})", len);
                    stack.push(i + len);
                },
                Datum => {
                    // following byte is length
                    let len = usize::from(unsafe { buf[i].data });
                    i += len;
                    write!(f, "? ")?;
                    trace!("Datum({:?})", len);
                },
                Wildcard => {
                    // following byte is id
                    let id = u32::from(unsafe { buf[i].data });
                    write!(f, "*{:?} ", id)?;
                    i += 1;
                    trace!("Wildcard({:?})", id);
                },
            }
        }
        //debug_assert!(stack.is_empty());
        Ok(())
    }
}
*/

#[derive(Copy, Clone, Debug)]
enum TyTok {
    P, // rest of this Subtree is a Pattern
    N, // rest of this Datum is a signed number; 1/2/4/8/16 bytes follow
    I, // rest of this Datum is an identifier; UTF-8 bytes follow
}

#[derive(Copy, Clone, Debug)]
enum Enum {
    S, // rest of this Subtree is a Statement
    X, // rest of this Subtree is an Expression
}

#[derive(Copy, Clone)]
enum Node {
    TyTok(TyTok),
    Enum(Enum), // next byte is discriminant; rest of Subtree is that
    Wildcard, // next byte is id
    Subtree, // next byte is length until end of Subtree
    Datum, // next byte is length until end of datum
}

#[derive(Default)]
struct IrBuilder {
    buf: Vec<TreeByte>,
}

// different AST visitors:
// Matchable:
// - discriminants: binary (byte)
// - terminals: binary (varlen)
// - subtrees: seekable (length-prefix based)
// Printable:
// - discriminants: named
// - terminals: pretty
// - subtrees: readable (bracketed)
// both:
// - types: implicit in structure

impl IrBuilder {
    fn visit_block<F: FnOnce(&mut Self)>(&mut self, f: F) {
        // push a len placeholder, marking its position
        let len_pos = self.buf.len();
        self.buf.push(TreeByte { data: 0u8 });
        // insert the body
        f(self);
        // fill in the placeholder with the final len
        debug_assert!(!(self.buf.len() < len_pos));
        let len = self.buf.len() - len_pos;
        assert!(len <= std::u8::MAX as usize);
        let len = len as u8;
        self.buf[len_pos] = TreeByte { data: len };
    }

    fn visit_subtree<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.buf.push(TreeByte { node: Node::Subtree });
        self.visit_block(f);
    }

    fn visit_datum<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.buf.push(TreeByte { node: Node::Datum });
        self.visit_block(f);
    }

    fn visit_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend(bytes.iter().cloned().map(|data| TreeByte { data }));
    }
}

impl<'ast> Visit<'ast> for IrBuilder {
    fn visit_stmt(&mut self, x: &syn::Stmt) {
        self.visit_subtree(|self_| {
            self_.buf.push(TreeByte { node: Node::Enum(Enum::S) });
            let discrim = unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8;
            self_.buf.push(TreeByte { data: discrim });
            syn::visit::visit_stmt(self_, x);
        });
    }

    fn visit_expr(&mut self, x: &syn::Expr) {
        if let syn::Expr::Meta(x) = x {
            let id = x.id.value();
            self.buf.push(TreeByte { node: Node::Wildcard });
            assert!(id <= 255);
            let data = id as u8;
            self.buf.push(TreeByte { data });
            return;
        }
        self.visit_subtree(|self_| {
            self_.buf.push(TreeByte { node: Node::Enum(Enum::X) });
            let discrim = unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8;
            self_.buf.push(TreeByte { data: discrim });
            syn::visit::visit_expr(self_, x);
        });
    }

    fn visit_ident(&mut self, x: &syn::Ident) {
        self.visit_datum(|self_| {
            self_.buf.push(TreeByte { node: Node::TyTok(TyTok::I) });
            self_.visit_bytes(x.to_string().as_bytes());
        });
    }

    fn visit_lit_int(&mut self, x: &syn::LitInt) {
        self.visit_datum(|self_| {
            self_.buf.push(TreeByte { node: Node::TyTok(TyTok::N) });
            // TODO: binary
            self_.visit_bytes(format!("{}", x.value()).as_bytes());
        });
    }

    fn visit_path(&mut self, x: &syn::Path) {
        self.visit_subtree(|self_| {
            self_.buf.push(TreeByte { node: Node::TyTok(TyTok::P) });
            syn::visit::visit_path(self_, x);
        });
    }
}

pub fn compile(block: &[syn::Stmt]) -> Ir {
    let mut viz = IrBuilder::default();
    block.iter().for_each(|s| viz.visit_stmt(s));
    Ir { buf: viz.buf }
}

// TOOD:
// - use ReprBuilder to drive IrBuilder from the same ast Visitor
// - trait helps ensure they continue to represent the same info

// Common trait for visiting the substructure of an AST we care about.
trait ReprBuilder {
    fn close_subtree(&mut self);
    fn open_stmt(&mut self, x: &syn::Stmt);
    fn open_expr(&mut self, x: &syn::Expr);
    fn open_pat(&mut self, x: &syn::Pat);
    fn visit_ident(&mut self, s: String);
    fn visit_int(&mut self, x: u64);
    fn visit_meta(&mut self, x: u64);
}

use serde_json::Value as JsValue;

#[derive(Default)]
struct DebugTreeBuilder {
    stack: Vec<Vec<JsValue>>,
    top: Vec<JsValue>,
}

impl DebugTreeBuilder {
    fn open_subtree(&mut self) {
        let parent = std::mem::replace(&mut self.top, Vec::new());
        self.stack.push(parent);
    }

    fn finish(self) -> Vec<JsValue> {
        assert!(self.stack.is_empty());
        self.top
    }
}

impl ReprBuilder for DebugTreeBuilder {
    fn close_subtree(&mut self) {
        let parent = self.stack.pop().unwrap();
        let child = std::mem::replace(&mut self.top, parent);
        self.top.push(child.into());
    }

    fn open_stmt(&mut self, x: &syn::Stmt) {
        self.open_subtree();
        self.top.push(crate::names::stmt_discrim(x).into())
    }

    fn open_expr(&mut self, x: &syn::Expr) {
        self.open_subtree();
        self.top.push(crate::names::expr_discrim(x).into())
    }

    fn open_pat(&mut self, x: &syn::Pat) {
        self.open_subtree();
        self.top.push(crate::names::pat_discrim(x).into());
    }

    fn visit_ident(&mut self, s: String) {
        self.top.push(s.into());
    }

    fn visit_int(&mut self, x: u64) {
        self.top.push(x.into());
    }

    fn visit_meta(&mut self, x: u64) {
        self.top.push(format!("${}", x).into());
    }
}

#[derive(Default)]
struct DebugStringBuilder {
    body: String,
}

impl DebugStringBuilder {
    fn open_subtree(&mut self) {
        self.body.push_str("{ ");
    }

    fn finish(self) -> String {
        self.body
    }
}

impl ReprBuilder for DebugStringBuilder {
    fn close_subtree(&mut self) {
        self.body.push_str("} ");
    }

    fn open_stmt(&mut self, x: &syn::Stmt) {
        if !self.body.is_empty() {
            self.body.pop();
            self.body.push('\n');
        }
        self.body.push_str(crate::names::stmt_discrim(x).into());
        self.open_subtree();
    }

    fn open_expr(&mut self, x: &syn::Expr) {
        self.body.push_str(crate::names::expr_discrim(x).into());
        self.open_subtree();
    }

    fn open_pat(&mut self, x: &syn::Pat) {
        self.body.push_str(crate::names::pat_discrim(x).into());
        self.open_subtree();
    }

    fn visit_ident(&mut self, s: String) {
        self.body.push_str(&s);
        self.body.push(' ');
    }

    fn visit_int(&mut self, x: u64) {
        self.body.push_str(&x.to_string());
        self.body.push(' ');
    }

    fn visit_meta(&mut self, x: u64) {
        self.body.push('$');
        self.body.push_str(&x.to_string());
        self.body.push(' ');
    }
}

// TODO: make this a macro instead of type-erasing
impl<'ast> Visit<'ast> for dyn ReprBuilder {
    fn visit_stmt(&mut self, x: &syn::Stmt) {
        self.open_stmt(x);
        syn::visit::visit_stmt(self, x);
        self.close_subtree();
    }

    fn visit_expr(&mut self, x: &syn::Expr) {
        if let syn::Expr::Meta(x) = x {
            let id = x.id.value();
            self.visit_meta(id);
            return;
        }
        self.open_expr(x);
        syn::visit::visit_expr(self, x);
        self.close_subtree();
    }

    fn visit_ident(&mut self, x: &syn::Ident) {
        self.visit_ident(x.to_string());
    }

    fn visit_lit_int(&mut self, x: &syn::LitInt) {
        self.visit_int(x.value());
    }

    fn visit_pat(&mut self, x: &syn::Pat) {
        self.open_pat(x);
        syn::visit::visit_pat(self, x);
        self.close_subtree();
    }

    /*
    fn visit_binop(&mut self, x: syn::BinOp) {
        let _ = &x.0.to_string();
    }
    */
}

pub fn debug_tree_repr(block: &[syn::Stmt]) -> String {
    let mut viz = DebugTreeBuilder::default();
    let vv: &mut ReprBuilder = &mut viz;
    block.iter().for_each(|s| vv.visit_stmt(s));
    serde_json::to_string(&viz.finish()).unwrap()
}

pub fn debug_flat_repr(block: &[syn::Stmt]) -> String {
    let mut viz = DebugStringBuilder::default();
    let vv: &mut ReprBuilder = &mut viz;
    block.iter().for_each(|s| vv.visit_stmt(s));
    viz.finish()
}

