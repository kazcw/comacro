/// Lower metasyn AST to regex-matchable IR

#[allow(unused)]
use log::trace;
use syn::visit::Visit;

pub struct MatchCode {
    // A trace of selected substructure of an AST traversal.
    // Untyped, binary, all data inline. Contains just enough info to locate differences.
    buf: Vec<u8>,
}
pub type Ir = MatchCode;

const META: u8 = 255;
const OPEN: u8 = 254;
const CLOSE: u8 = 253;

// escape all pseudo-Metas
// length-prefix varlens (Datum) so they compare correctly
// no distinction between parsed/unparsed data--everything is quoted anyway
// Meta is for wildcard-matchable subtrees

// parser only needs:
//META META = escaped literal Meta
//META <id> = wildcard
//META OPEN = subtree
//META CLOSE = subtree

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

#[derive(Default)]
struct Trace {
    buf: Vec<u8>,
    datum: Option<usize>,
}

use std::fmt::Debug;
impl Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut i = 0;
        while i < self.buf.len() {
            if self.buf[i] == META {
                i += 1;
                match self.buf[i] {
                    META => write!(f, "{:02x}", META)?,
                    OPEN => write!(f, "[")?,
                    CLOSE => write!(f, "]")?,
                    x => write!(f, "${}", x as u32)?,
                }
            } else {
                write!(f, "{:02x}", self.buf[i])?;
            }
            i += 1;
        }
        Ok(())
    }
}

impl Trace {
    fn push_byte(&mut self, data: u8) {
        self.buf.push(data);
        if data == META {
            self.buf.push(data);
        }
    }

    fn extend_bytes(&mut self, data: &[u8]) {
        // TODO: less pessimal impl?
        for x in data { self.push_byte(*x); }
    }

    fn open_subtree(&mut self) {
        self.buf.push(META);
        self.buf.push(OPEN);
    }

    fn close_subtree(&mut self) {
        self.buf.push(META);
        self.buf.push(CLOSE);
    }

    fn open_datum(&mut self) {
        self.buf.push(0);
        assert!(self.datum.is_none());
        self.datum = Some(self.buf.len() - 1);
    }

    fn close_datum(&mut self) {
        let open = self.datum.expect("open before closing");
        self.datum = None;
        let diff = self.buf.len() - open;
        let smol_diff = diff as u8;
        assert_eq!(usize::from(smol_diff), diff);
        assert!(smol_diff != META);
        assert!(smol_diff != OPEN);
        assert!(smol_diff != CLOSE);
        self.buf[open] = smol_diff;
    }

    fn push_mvar(&mut self, mvar: u8) {
        assert!(mvar != META);
        assert!(mvar != OPEN);
        assert!(mvar != CLOSE);
        self.buf.push(META);
        self.buf.push(mvar);
    }
}

struct ReTrace {
    buf: Vec<u8>,
    datum: Option<usize>,
    i: usize,
    diff_depth: usize,
}

impl ReTrace {
    fn new(buf: Vec<u8>) -> Self {
        ReTrace { buf, datum: None, i: 0, diff_depth: 0 }
    }

    fn push_byte(&mut self, data: u8) {
        if self.diff_depth != 0 { return; }
        if self.buf[self.i] != data {
            self.diff_depth = 1;
            return;
        }
        self.i += 1;
        if data == META {
            if self.buf[self.i] != META {
                self.diff_depth = 1;
                return;
            }
            self.i += 1;
        }
    }

    fn extend_bytes(&mut self, data: &[u8]) {
        // TODO: less pessimal impl?
        for x in data { self.push_byte(*x); }
    }

    /// on failure, nothing has been consumed
    fn open_subtree(&mut self) -> Result<(), ()> {
        //trace!("ReTrace::open_subtree: diff_depth={}", self.diff_depth);
        if self.diff_depth != 0 {
            self.diff_depth += 1;
            // if parent mismatched, our status is neither here nor there
            return Ok(())
        }
        if self.buf[self.i] == META && self.buf[self.i + 1] == OPEN {
            self.i += 2;
            Ok(())
        } else {
            Err(())
        }
    }

    // if a mismatch occurs:
    // - consume the subtree
    // - count depth within dead subtree

    /// on failure, this mismatching subtree has been consumed
    fn close_subtree(&mut self) -> Result<(), ()> {
        //trace!("ReTrace::close_subtree: diff_depth={}", self.diff_depth);
        if self.diff_depth == 0 {
            if self.buf[self.i] != META || self.buf[self.i + 1] != CLOSE {
                self.diff_depth = 1;
            } else {
                self.i += 2;
                return Ok(());
            }
        }
        while self.diff_depth > 0 {
            if self.buf[self.i] == META || self.buf[self.i + 1] == CLOSE {
                self.i += 2;
                self.diff_depth -= 1;
            } else {
                self.i += 1;
            }
        }
        Err(())
    }

    fn open_datum(&mut self) {
        assert!(self.datum.is_none());
        self.datum = Some(self.i + usize::from(self.buf[self.i]));
        self.i += 1;
    }

    fn close_datum(&mut self) {
        assert_eq!(self.datum, Some(self.i));
        self.datum = None;
    }

    fn consume_meta(&mut self) -> u8 {
        let x = self.buf[self.i];
        assert_eq!(x, META);
        self.i += 1;
        let x = self.buf[self.i];
        assert!(x != META);
        assert!(x != OPEN);
        assert!(x != CLOSE);
        self.i += 1;
        x
    }
}

#[derive(Default)]
struct TxTrace {
    trace: Trace,
    replacement: Trace,
    stack: Vec<usize>,
    rollbacks: usize,
}

impl TxTrace {
    fn new(trace: Trace) -> Self {
        TxTrace { trace, replacement: Trace::default(), stack: Vec::new(), rollbacks: 0 }
    }

    fn push_byte(&mut self, data: u8) { self.trace.push_byte(data); }
    fn extend_bytes(&mut self, data: &[u8]) { self.trace.extend_bytes(data); }
    fn open_datum(&mut self) { self.trace.open_datum(); }
    fn close_datum(&mut self) { self.trace.close_datum(); }

    fn open_subtree(&mut self) {
        self.stack.push(self.trace.buf.len());
        self.trace.open_subtree();
        //trace!("TxTrace::open_subtree: {:?}", self.trace);
    }
    fn close_subtree(&mut self) {
        self.trace.close_subtree();
        let start = self.stack.pop().unwrap();
        if self.rollbacks > 0 {
            //trace!("TxTrace::close_subtree: rollback: before: {:?}", self.trace);
            self.trace.buf.truncate(start);
            self.rollbacks -= 1;
            //trace!("TxTrace::close_subtree: rollback:  after: {:?}", self.trace);
            if self.rollbacks == 0 {
                self.trace.buf.extend(&self.replacement.buf);
                self.replacement.buf.clear();
            }
            //trace!("TxTrace::close_subtree: rollback: splice: {:?}", self.trace);
        } else {
            //trace!("TxTrace::close_subtree: {:?}", self.trace);
        }
    }
    fn rollback(&mut self, depth: usize) {
        assert_eq!(self.rollbacks, 0);
        self.rollbacks = depth + 1;
        //trace!("TxTrace::rollback: {}", self.rollbacks);
    }
}

struct DeltaTrace {
    old: ReTrace,
    new: TxTrace,
}

impl DeltaTrace {
    fn new(old: Vec<u8>) -> Self {
        let old_len = old.len();
        let old = ReTrace::new(old);
        let new = TxTrace::new (Trace { buf: Vec::with_capacity(old_len), ..Default::default()});
        DeltaTrace { old, new }
    }

    fn push_byte(&mut self, data: u8) {
        self.old.push_byte(data);
        self.new.push_byte(data);
    }

    fn extend_bytes(&mut self, data: &[u8]) {
        self.old.extend_bytes(data);
        self.new.extend_bytes(data);
    }

    fn open_subtree(&mut self) -> Result<(), ()> {
        self.old.open_subtree()?;
        self.new.open_subtree();
        Ok(())
    }

    fn close_subtree(&mut self) -> Result<(), ()> {
        self.old.close_subtree()?;
        self.new.close_subtree();
        Ok(())
    }

    fn open_datum(&mut self) {
        self.old.open_datum();
        self.new.open_datum();
    }

    fn close_datum(&mut self) {
        self.old.close_datum();
        self.new.close_datum();
    }
}

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

struct JsonGenerator {
    trace: ReTrace,
    buf: std::io::Cursor<Vec<u8>>,
    sibling: bool,
}

impl JsonGenerator {
    fn new(old: Vec<u8>) -> Self {
        let mut buf = std::io::Cursor::new(Vec::new());
        write!(buf, "[");
        JsonGenerator {
            trace: ReTrace::new(old),
            buf,
            sibling: false,
        }
    }

    fn finish(mut self) -> Vec<u8> {
        write!(self.buf, "]");
        self.buf.into_inner()
    }

    fn maybe_comma(&mut self) {
        if self.sibling { write!(self.buf, ",").unwrap(); }
    }

    fn emit_meta(&mut self) {
        let x = u32::from(self.trace.consume_meta());
        self.maybe_comma();
        write!(self.buf, "\"${}\"", x);
        self.sibling = true;
    }
}

use std::io::Write;
impl InnerVisitor for JsonGenerator {
    fn open_expr(&mut self, x: &syn::Expr) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            self.emit_meta();
            return Err(());
        }
        self.maybe_comma();
        write!(self.buf, "[\"{}\"", crate::names::expr_discrim(x));
        self.sibling = true;
        Ok(())
    }
    fn open_ident(&mut self, x: &syn::Ident) -> Result<(), ()> {
        if let Err(()) = self.trace.open_subtree() {
            self.emit_meta();
            return Err(());
        }
        self.maybe_comma();
        write!(self.buf, "\"{}\"", x.to_string());
        self.sibling = true;
        Ok(())
    }
    fn open_stmt(&mut self, x: &syn::Stmt) {
        self.open_subtree();
        self.maybe_comma();
        write!(self.buf, "[\"{}\"", crate::names::stmt_discrim(x));
        self.sibling = true;
    }
    fn open_pat(&mut self, x: &syn::Pat) {
        self.open_subtree();
        self.maybe_comma();
        write!(self.buf, "[\"{}\"", crate::names::pat_discrim(x));
        self.sibling = true;
    }
    fn open_lit_int(&mut self, x: &syn::LitInt) {
        self.open_datum();
        self.maybe_comma();
        write!(self.buf, "{}", x.value());
        self.sibling = true;
    }

    fn close_expr(&mut self, _: &syn::Expr) {
        self.close_subtree();
        write!(self.buf, "]");
        self.sibling = true;
    }
    fn close_stmt(&mut self, _: &syn::Stmt) {
        self.close_subtree();
        write!(self.buf, "]");
        self.sibling = true;
    }
    fn close_pat(&mut self, _: &syn::Pat) {
        self.close_subtree();
        write!(self.buf, "]");
        self.sibling = true;
    }
    fn close_ident(&mut self, _: &syn::Ident) {
        self.close_subtree();
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
    let mut idviz = Visitor { inner: Reconciler { trace: DeltaTrace::new(nodeviz.inner.trace.buf) } };
    ids.iter().for_each(|s| idviz.visit_stmt(s));
    trace!("compiled: {:?}", idviz.inner.trace.new.trace);
    MatchCode { buf: idviz.inner.trace.new.trace.buf }
}

pub fn debug_tree_repr(nodes: &[syn::Stmt], ids: &[syn::Stmt]) -> String {
    let mc = compile(nodes, ids);
    let mut viz = Visitor { inner: JsonGenerator::new(mc.buf) };
    nodes.iter().for_each(|s| viz.visit_stmt(s));
    String::from_utf8(viz.inner.finish()).unwrap()
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
pub fn debug_flat_repr(block: &[syn::Stmt], todo: &[syn::Stmt]) -> String {
    unimplemented!()
    /*
    let mut viz = DebugStringBuilder::default();
    let vv: &mut ReprBuilder = &mut viz;
    block.iter().for_each(|s| vv.visit_stmt(s));
    viz.finish()
    */
}
