/// Lower metasyn AST to regex-matchable IR

#[allow(unused)]
use log::trace;
use syn::visit::Visit;

use crate::trace::*;

pub struct MatchCode {
    // A trace of selected substructure of an AST traversal.
    // Untyped, binary, all data inline. Contains just enough info to locate differences.
    buf: Vec<u8>,
}
pub type Ir = MatchCode;

// escape all pseudo-Metas
// length-prefix varlens (Datum) so they compare correctly
// no distinction between parsed/unparsed data--everything is quoted anyway
// Meta is for wildcard-matchable subtrees

/*
 * matching:
 *
 * - naive interpretation: parse, keeping Subtree stack
 * - faster: exact comparison with pattern; on diff, fail if pattern is not at wildcard; if pattern
 *   is at wildcard, mismatched item must be a subgraph opener
 *   - knowing if a location is a wildcard:
 *     - keep a subtree stack as we go
 *     - use a mvar table
 *     * escape pseudo-mvars in Datums, lengths, discriminants
 *
 */

// Ast nodes -> Mc nodes
// Ast ids -> Mc ids
//
// Mc nodes -> Mc ids -> Mc pattern [reconcile]
//
// TreeRepr visitor: Mc pattern -> Ast nodes
// - count along Ast's position in Mc instead of emitting
// - open_subtree checks if pattern is at a mvar position to skip visit

struct Visitor<V> {
    inner: V,
}

trait InnerVisitor {
    fn open_expr(&mut self, _: &syn::Expr) -> Result<(), ()> { self.open_subtree(); Ok(()) }
    fn open_ident(&mut self, _: &syn::Ident) -> Result<(), ()> { self.open_subtree(); Ok(()) }
    fn open_stmt(&mut self, _: &syn::Stmt) { self.open_subtree() }
    fn open_pat(&mut self, _: &syn::Pat) { self.open_subtree() }
    fn open_lit_int(&mut self, _: &syn::LitInt) { self.open_datum() }

    fn close_expr(&mut self, _: &syn::Expr) { self.close_subtree() }
    fn close_ident(&mut self, _: &syn::Ident) { self.close_subtree() }
    fn close_stmt(&mut self, _: &syn::Stmt) { self.close_subtree() }
    fn close_pat(&mut self, _: &syn::Pat) { self.close_subtree() }
    fn close_lit_int(&mut self, _: &syn::LitInt) { self.close_datum() }

    fn open_subtree(&mut self);
    fn close_subtree(&mut self);
    fn open_datum(&mut self);
    fn close_datum(&mut self);
    fn push_byte(&mut self, x: u8);
    fn extend_bytes(&mut self, x: &[u8]);
}

#[derive(Default)]
struct MatchCodeGenerator { trace: Trace }
impl InnerVisitor for MatchCodeGenerator {
    fn open_subtree(&mut self) { self.trace.open_subtree(); }
    fn close_subtree(&mut self) { self.trace.close_subtree(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

struct Reconciler { trace: DeltaTrace }
impl InnerVisitor for Reconciler {
    fn close_ident(&mut self, x: &syn::Ident) {
        if let Ok(()) = self.trace.close_subtree() { return; }
        let x = x.to_string();
        let x_id = x.trim_left_matches("IDENT_");
        let x_expr = x.trim_left_matches("EXPR_");
        if x_id != x {
            // replace current
            self.trace.new.replacement.push_mvar(u8::from_str_radix(x_id, 10).unwrap());
            self.trace.new.rollback(0);
            self.trace.new.close_subtree();
        } else if x_expr != x {
            // replace parent
            self.trace.new.replacement.push_mvar(u8::from_str_radix(x_expr, 10).unwrap());
            self.trace.new.rollback(1);
            self.trace.new.close_subtree();
        } else {
            unreachable!();
        }
    }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

/*
struct Binder { trace: ReTrace }
impl InnerVisitor for Binder {
    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> { unimplemented!() }
    fn open_ident(&mut self, x: &syn::Ident) -> Result<(), ()> { unimplemented!() }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}
*/

trait Emitter {
    fn meta(&mut self, x: u32);
    fn item(&mut self, s: impl Display);
    fn text_item(&mut self, s: impl Display);
    fn opener(&mut self, s: impl Display);
    fn closer(&mut self);
    fn maybe_break(&mut self) {}
}

use std::io::Write;
use std::fmt::Display;

struct ReprEmitter {
    buf: std::io::Cursor<Vec<u8>>,
    sibling: bool,
}

impl ReprEmitter {
    fn new() -> Self {
        let mut buf = std::io::Cursor::new(Vec::new());
        let sibling = false;
        ReprEmitter { buf, sibling }
    }

    fn finish(mut self) -> Vec<u8> {
        self.buf.into_inner()
    }

    fn maybe_comma(&mut self) {
        if self.sibling { self.comma(); }
    }

    fn comma(&mut self) {
        write!(self.buf, " ").unwrap();
    }
}

impl Emitter for ReprEmitter {
    fn meta(&mut self, x: u32) {
        self.maybe_comma();
        write!(self.buf, "${}", x);
        self.sibling = true;
    }

    fn item(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "{}", s);
        self.sibling = true;
    }

    fn text_item(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "{}", s);
        self.sibling = true;
    }

    fn opener(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "{}{{", s);
        self.sibling = true;
    }

    fn closer(&mut self) {
        self.maybe_comma();
        write!(self.buf, "}}");
        self.sibling = true;
    }

    fn maybe_break(&mut self) {
        if self.buf.position() != 0 {
            write!(self.buf, "\n");
            self.sibling = false;
        }
    }
}

struct JsonEmitter {
    buf: std::io::Cursor<Vec<u8>>,
    sibling: bool,
}

impl JsonEmitter {
    fn new() -> Self {
        let mut buf = std::io::Cursor::new(Vec::new());
        write!(buf, "[");
        let sibling = false;
        JsonEmitter { buf, sibling }
    }

    fn finish(mut self) -> Vec<u8> {
        write!(self.buf, "]");
        self.buf.into_inner()
    }

    fn maybe_comma(&mut self) {
        if self.sibling { self.comma(); }
    }

    fn comma(&mut self) {
        write!(self.buf, ",").unwrap();
    }
}

impl Emitter for JsonEmitter {
    fn meta(&mut self, x: u32) {
        self.maybe_comma();
        write!(self.buf, "\"${}\"", x);
        self.sibling = true;
    }

    fn item(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "{}", s);
        self.sibling = true;
    }

    fn text_item(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "\"{}\"", s);
        self.sibling = true;
    }

    fn opener(&mut self, s: impl Display) {
        self.maybe_comma();
        write!(self.buf, "[\"{}\"", s);
        self.sibling = true;
    }

    fn closer(&mut self) {
        write!(self.buf, "]");
        self.sibling = true;
    }
}

struct ReprGenerator<E> {
    emitter: E,
    trace: ReTrace,
}
impl<E: Emitter> ReprGenerator<E> {
    fn new(old: Vec<u8>, emitter: E) -> Self {
        let trace = ReTrace::new(old);
        ReprGenerator { emitter, trace }
    }
}

impl<E: Emitter> InnerVisitor for ReprGenerator<E> {
    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = u32::from(self.trace.consume_meta());
            self.emitter.meta(x);
            return Err(());
        }
        self.emitter.opener(crate::names::expr_discrim(x));
        Ok(())
    }
    fn open_ident(&mut self, x: &syn::Ident) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = u32::from(self.trace.consume_meta());
            self.emitter.meta(x);
            return Err(());
        }
        self.emitter.text_item(x);
        Ok(())
    }
    fn open_stmt(&mut self, x: &syn::Stmt) {
        self.open_subtree();
        self.emitter.maybe_break();
        self.emitter.opener(crate::names::stmt_discrim(x));
    }
    fn open_pat(&mut self, x: &syn::Pat) {
        self.open_subtree();
        self.emitter.opener(crate::names::pat_discrim(x));
    }
    fn open_lit_int(&mut self, x: &syn::LitInt) {
        self.open_datum();
        self.emitter.item(x.value());
    }

    fn close_expr(&mut self, _: &syn::Expr) {
        self.close_subtree();
        self.emitter.closer();
    }
    fn close_stmt(&mut self, _: &syn::Stmt) {
        self.close_subtree();
        self.emitter.closer();
    }
    fn close_pat(&mut self, _: &syn::Pat) {
        self.close_subtree();
        self.emitter.closer();
    }

    fn open_subtree(&mut self) { self.trace.open_subtree().unwrap(); }
    fn close_subtree(&mut self) { self.trace.close_subtree().unwrap(); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }
    fn push_byte(&mut self, x: u8) { self.trace.push_byte(x); }
    fn extend_bytes(&mut self, x: &[u8]) { self.trace.extend_bytes(x); }
}

/// Outer visitor ensures AST->MatchCode lowering is consistent for different traversal modes
impl<'ast, V: InnerVisitor> Visit<'ast> for Visitor<V> {
    fn visit_stmt(&mut self, x: &syn::Stmt) {
        self.inner.open_stmt(x);
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_stmt(self, x);
        self.inner.close_stmt(x);
    }

    fn visit_expr(&mut self, x: &syn::Expr) {
        if let Err(()) = self.inner.open_expr(x) { return; }
        self.inner.push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_expr(self, x);
        self.inner.close_expr(x);
    }

    fn visit_pat(&mut self, x: &syn::Pat) {
        self.inner.open_pat(x);
        syn::visit::visit_pat(self, x);
        self.inner.close_pat(x);
    }

    fn visit_ident(&mut self, x: &syn::Ident) {
        if let Err(()) = self.inner.open_ident(x) { return; }
        self.inner.extend_bytes(x.to_string().as_bytes());
        self.inner.close_ident(x);
    }

    fn visit_lit_int(&mut self, x: &syn::LitInt) {
        self.inner.open_lit_int(x);
        // TODO: compact repr?
        self.inner.extend_bytes(format!("{}", x.value()).as_bytes());
        self.inner.close_lit_int(x);
    }
}

pub fn compile(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> MatchCode {
    let mut nodeviz = Visitor { inner: MatchCodeGenerator::default() };
    nodes.iter().for_each(|s| nodeviz.visit_stmt(s));
    trace!("nds: {:?}", nodeviz.inner.trace);
    {
        let mut idviz = Visitor { inner: MatchCodeGenerator::default() };
        ids.iter().for_each(|s| idviz.visit_stmt(s));
        trace!("ids: {:?}", idviz.inner.trace);
    }
    let mut idviz = Visitor { inner: Reconciler { trace: DeltaTrace::new(nodeviz.inner.trace.finish()) } };
    ids.iter().for_each(|s| idviz.visit_stmt(s));
    //trace!("compiled: {:?}", idviz.inner.trace.new.trace);
    //MatchCode { buf: idviz.inner.trace.new.trace.buf }
    MatchCode { buf: idviz.inner.trace.finish() }
}

pub fn debug_tree_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc.buf, JsonEmitter::new()) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.emitter.finish()).unwrap()
}

pub fn debug_flat_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile(nodes, ids);
    let mut viz = Visitor { inner: ReprGenerator::new(mc.buf, ReprEmitter::new()) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.emitter.finish()).unwrap()
}

// Common trait for visiting the substructure of an AST we care about.
/*
trait ReprBuilder {
    fn close_subtree(&mut self);
    fn open_stmt(&mut self, x: &syn::Stmt);
    fn open_expr(&mut self, x: &syn::Expr);
    fn open_pat(&mut self, x: &syn::Pat);
    fn visit_ident(&mut self, s: String);
    fn visit_lit_int(&mut self, x: u64);
    fn visit_meta(&mut self, x: u64);
}
*/

/*
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

    fn visit_lit_int(&mut self, x: u64) {
        self.body.push_str(&x.to_string());
        self.body.push(' ');
    }

    fn visit_meta(&mut self, x: u64) {

        self.body.push('$');
        self.body.push_str(&x.to_string());
        self.body.push(' ');
    }
}
*/
