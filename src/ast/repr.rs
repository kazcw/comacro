use std::fmt::Display;
use std::io::Write;

use crate::ast::{Binding, Bindings};
use crate::ast::names::Discrim;
use crate::ast::visit::{Visitor, Visitable};
use crate::trace::*;

pub(crate) trait Emitter {
    fn meta(&mut self, x: u32);
    fn item(&mut self, s: impl Display);
    fn text_item(&mut self, s: impl Display);
    fn opener(&mut self, s: impl Display);
    fn closer(&mut self);
    fn maybe_break(&mut self) {}
    fn finish(self) -> Vec<u8>;
}

pub(crate) struct ReprEmitter {
    buf: std::io::Cursor<Vec<u8>>,
    sibling: bool,
}

impl ReprEmitter {
    pub fn new() -> Self {
        let buf = std::io::Cursor::new(Vec::new());
        let sibling = false;
        ReprEmitter { buf, sibling }
    }

    fn maybe_comma(&mut self) {
        if self.sibling {
            self.comma();
        }
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
            writeln!(self.buf);
            self.sibling = false;
        }
    }

    fn finish(self) -> Vec<u8> {
        self.buf.into_inner()
    }
}

pub(crate) struct JsonEmitter {
    buf: std::io::Cursor<Vec<u8>>,
    sibling: bool,
    scalar_context: bool,
}

impl JsonEmitter {
    pub fn new() -> Self {
        let mut buf = std::io::Cursor::new(Vec::new());
        write!(buf, "[");
        let sibling = false;
        JsonEmitter {
            buf,
            sibling,
            scalar_context: false,
        }
    }

    pub fn new_scalar() -> Self {
        let buf = std::io::Cursor::new(Vec::new());
        let sibling = false;
        JsonEmitter {
            buf,
            sibling,
            scalar_context: true,
        }
    }

    fn maybe_comma(&mut self) {
        if self.sibling {
            self.comma();
        }
    }

    fn comma(&mut self) {
        write!(self.buf, ",").unwrap();
    }
}

impl Emitter for JsonEmitter {
    fn finish(mut self) -> Vec<u8> {
        if !self.scalar_context {
            write!(self.buf, "]");
        }
        self.buf.into_inner()
    }

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

/// An AST visitor that compares compiled MatchCode for a pattern with the AST for that pattern's
/// <nodes> or <ids> tree to emit a representation of the pattern.
pub(crate) struct ReprGenerator<'t, E> {
    emitter: E,
    trace: ReTracer<'t>,
}

impl<'t, E: Emitter> ReprGenerator<'t, E> {
    pub fn new(old: &'t Trace, emitter: E) -> Self {
        let trace = ReTracer::new(old);
        ReprGenerator { emitter, trace }
    }
}

impl<'t, 'ast, E: Emitter> Visitor<'ast> for ReprGenerator<'t, E> {
    type Output = String;

    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            let x = u32::from(self.trace.consume_meta());
            self.emitter.meta(x);
            return Err(());
        }
        self.emitter.opener(x.discrim());
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
        self.emitter.opener(x.discrim());
    }
    fn open_pat(&mut self, x: &syn::Pat) {
        self.open_subtree();
        self.emitter.opener(x.discrim());
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
        String::from_utf8(self.emitter.finish()).unwrap()
    }
}

/// Serialize a normal AST (no metavars)
pub(crate) struct PlainAstRepr<E> {
    emitter: E,
}

impl<E: Emitter> PlainAstRepr<E> {
    pub fn new(emitter: E) -> Self {
        PlainAstRepr { emitter }
    }
}

impl<E: Emitter> Visitor<'_> for PlainAstRepr<E> {
    type Output = String;

    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> {
        self.emitter.opener(x.discrim());
        Ok(())
    }
    fn open_ident(&mut self, x: &syn::Ident) -> Result<(), ()> {
        self.emitter.text_item(x);
        Ok(())
    }
    fn open_stmt(&mut self, x: &syn::Stmt) {
        self.emitter.maybe_break();
        self.emitter.opener(x.discrim());
    }
    fn open_pat(&mut self, x: &syn::Pat) {
        self.emitter.opener(x.discrim());
    }
    fn open_lit_int(&mut self, x: &syn::LitInt) {
        self.emitter.item(x.value());
    }

    fn close_expr(&mut self, _: &syn::Expr) {
        self.emitter.closer();
    }
    fn close_stmt(&mut self, _: &syn::Stmt) {
        self.emitter.closer();
    }
    fn close_pat(&mut self, _: &syn::Pat) {
        self.emitter.closer();
    }

    fn open_subtree(&mut self) {}
    fn close_subtree(&mut self) {}
    fn open_datum(&mut self) {}
    fn close_datum(&mut self) {}
    fn push_byte(&mut self, x: u8) {}
    fn extend_bytes(&mut self, x: &[u8]) {}

    fn finish(self) -> Self::Output {
        String::from_utf8(self.emitter.finish()).unwrap()
    }
}

pub fn pattern_json<'a, 'v: 'a, V>(trace: &Trace, v: &'v V) -> String where V: Visitable<'a>+?Sized {
    ReprGenerator::new(trace, JsonEmitter::new()).visit(v)
}

pub fn pattern_flat<'a, 'v: 'a, V>(trace: &Trace, v: &'v V) -> String where V: Visitable<'a>+?Sized {
    ReprGenerator::new(trace, ReprEmitter::new()).visit(v)
}

pub fn input_json<'a, 'v: 'a, V>(v: &'v V) -> String where V: Visitable<'a>+?Sized {
    PlainAstRepr::new(JsonEmitter::new_scalar()).visit(v)
}

pub fn bindings_json(bindings: &'_ Bindings) -> String {
    let f = |b: &Binding| match b {
        Binding::Ident(i) => format!("[\"Ident\",\"{}\"]", i),
        Binding::Expr(x) => format!("[\"Expr\",{}]", input_json(*x))
    };
    let mut buf = "[".to_owned();
    let mut binds = bindings.binds.iter();
    if let Some(b) = binds.next() {
        buf.push_str(&f(b));
    }
    for b in binds {
        buf.push_str(",");
        buf.push_str(&f(b));
    }
    buf.push_str("]");
    buf
}
