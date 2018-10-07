//! A regex language for trees

use log::trace;
use std::fmt::Debug;

#[derive(PartialEq, Eq, Clone)]
pub struct Trace {
    buf: Vec<u8>
}

const META: u8 = 255;
const OPEN: u8 = 254;
const CLOSE: u8 = 253;

#[derive(Debug)]
pub struct IndexedTrace {
    trace: Trace,
    indexes: Vec<usize>,
}

impl IndexedTrace {
    fn traces(&self) -> Traces {
        Traces { master: &self, i: 0 }
    }

    // XXX this should not need to exist
    pub fn deindex(self) -> Trace {
        self.trace
    }
}

struct Traces<'a> {
    master: &'a IndexedTrace,
    i: usize,
}

impl<'a> Iterator for Traces<'a> {
    type Item = (usize, Symbols<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(&offs) = self.master.indexes.get(self.i) {
            let i = self.i;
            self.i += 1;
            Some((i, Symbols { buf: &self.master.trace.buf[offs..] }))
        } else {
            None
        }
    }
}

pub struct ToplevelMatches<'a, 'b> {
    pattern: Symbols<'a>,
    inputs: Traces<'b>,
}

impl Iterator for ToplevelMatches<'_, '_> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        for (i, mut input) in self.inputs.by_ref() {
            if is_match(self.pattern.clone(), &mut input) {
                return Some(i);
            }
        }
        None
    }
}

#[derive(PartialEq, Eq, Debug)]
enum Symbol {
    Meta(u8),
    Literal(u8),
    Open,
    Close,
}

#[derive(Clone)]
struct Symbols<'a> { buf: &'a [u8] }

impl<'a> Iterator for Symbols<'a> {
    type Item = Symbol;

    fn next(&mut self) -> Option<Symbol> {
        if let Some((&x, rest)) = self.buf.split_first() {
            if x == META {
                let ((&x, rest)) = rest.split_first().unwrap();
                self.buf = rest;
                Some(match x {
                    META => Symbol::Literal(META),
                    OPEN => Symbol::Open,
                    CLOSE => Symbol::Close,
                    x => Symbol::Meta(x),
                })
            } else {
                self.buf = rest;
                Some(Symbol::Literal(x))
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // from 0 to half the characters remaining, rounded down, are metas that won't become
        // independent symbols
        ((self.buf.len() + 1) / 2, Some(self.buf.len()))
    }
}

impl Trace {
    fn symbols(&self) -> Symbols {
        Symbols { buf: &self.buf }
    }
}

impl Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.symbols())
    }
}

impl Debug for Symbols<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for x in self.clone() {
            match x {
                Symbol::Literal(x) => write!(f, "{:02x}", x)?,
                Symbol::Meta(x) => write!(f, "${}", x as u32)?,
                Symbol::Open => write!(f, "[")?,
                Symbol::Close => write!(f, "]")?,
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Tracer {
    buf: Vec<u8>,
    datum: Option<usize>,
    // IndexedTrace
    indexes: Vec<usize>,
}

impl Tracer {
    pub fn push_byte(&mut self, data: u8) {
        self.buf.push(data);
        if data == META {
            self.buf.push(data);
        }
    }

    pub fn extend_bytes(&mut self, data: &[u8]) {
        // TODO: less pessimal impl?
        for x in data { self.push_byte(*x); }
    }

    pub fn open_subtree(&mut self) {
        self.buf.push(META);
        self.buf.push(OPEN);
    }

    pub fn close_subtree(&mut self) {
        self.buf.push(META);
        self.buf.push(CLOSE);
    }

    pub fn open_datum(&mut self) {
        self.buf.push(0);
        assert!(self.datum.is_none());
        self.datum = Some(self.buf.len() - 1);
    }

    pub fn close_datum(&mut self) {
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

    pub fn push_mvar(&mut self, mvar: u8) {
        assert!(mvar != META);
        assert!(mvar != OPEN);
        assert!(mvar != CLOSE);
        self.buf.push(META);
        self.buf.push(mvar);
    }

    pub fn finish(self) -> Trace {
        Trace { buf: self.buf }
    }
}

// basic tracer also builds IndexedTrace
impl Tracer {
    pub fn expect_marks(&mut self, n: usize) {
        self.indexes.reserve_exact(n);
    }

    pub fn mark(&mut self) {
        let pos = self.buf.len();
        self.indexes.push(pos);
    }

    pub fn finish_with_indexes(self) -> IndexedTrace {
        IndexedTrace { trace: Trace { buf: self.buf }, indexes: self.indexes }
    }
}

// TODO: borrow bufs for read-only Tracers
// XXX: this should be pub(crate)
/// instead of emitting a Trace, follow along with a previous, noting any differing subtrees
pub struct ReTracer {
    buf: Vec<u8>,
    datum: Option<usize>,
    i: usize,
    diff_depth: usize,
}

impl ReTracer {
    pub fn new(trace: Trace) -> Self {
        let buf = trace.buf;
        ReTracer { buf, datum: None, i: 0, diff_depth: 0 }
    }
    pub fn finish(self) {
        // it's a programming error to attempt to complete a trace inside a subtree
        assert!(self.datum.is_none());
        assert_eq!(self.diff_depth, 0);
        // old trace is expected to match structurally, so this should also not happen in a
        // completed run
        assert_eq!(self.i, self.buf.len());
    }

    pub fn push_byte(&mut self, data: u8) {
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

    pub fn extend_bytes(&mut self, data: &[u8]) {
        // TODO: less pessimal impl?
        for x in data { self.push_byte(*x); }
    }

    /// on failure, nothing has been consumed
    pub fn open_subtree(&mut self) -> Result<(), ()> {
        //trace!("ReTracer::open_subtree: diff_depth={}", self.diff_depth);
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
    pub fn close_subtree(&mut self) -> Result<(), ()> {
        //trace!("ReTracer::close_subtree: diff_depth={}", self.diff_depth);
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

    pub fn open_datum(&mut self) {
        assert!(self.datum.is_none());
        self.datum = Some(self.i + usize::from(self.buf[self.i]));
        self.i += 1;
    }

    pub fn close_datum(&mut self) {
        assert_eq!(self.datum, Some(self.i));
        self.datum = None;
    }

    pub fn consume_meta(&mut self) -> u8 {
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

/// transactional Tracer builder that can >/dev/null the current subtree to a specified depth and
/// emit a replacement
#[derive(Default)]
pub(crate) struct TxTracer {
    trace: Tracer,
    pub replacement: Tracer,
    stack: Vec<usize>,
    rollbacks: usize,
}

impl TxTracer {
    pub fn new(trace: Tracer) -> Self {
        TxTracer { trace, replacement: Tracer::default(), stack: Vec::new(), rollbacks: 0 }
    }
    pub fn finish(self) -> Trace {
        assert!(self.stack.is_empty());
        assert!(self.replacement.buf.is_empty());
        assert_eq!(self.rollbacks, 0);
        self.trace.finish()
    }

    pub fn push_byte(&mut self, data: u8) { self.trace.push_byte(data); }
    pub fn extend_bytes(&mut self, data: &[u8]) { self.trace.extend_bytes(data); }
    pub fn open_datum(&mut self) { self.trace.open_datum(); }
    pub fn close_datum(&mut self) { self.trace.close_datum(); }

    pub fn open_subtree(&mut self) {
        self.stack.push(self.trace.buf.len());
        self.trace.open_subtree();
        //trace!("TxTracer::open_subtree: {:?}", self.trace);
    }
    pub fn close_subtree(&mut self) {
        self.trace.close_subtree();
        let start = self.stack.pop().unwrap();
        if self.rollbacks > 0 {
            //trace!("TxTracer::close_subtree: rollback: before: {:?}", self.trace);
            self.trace.buf.truncate(start);
            self.rollbacks -= 1;
            //trace!("TxTracer::close_subtree: rollback:  after: {:?}", self.trace);
            if self.rollbacks == 0 {
                self.trace.buf.extend(&self.replacement.buf);
                self.replacement.buf.clear();
            }
            //trace!("TxTracer::close_subtree: rollback: splice: {:?}", self.trace);
        } else {
            //trace!("TxTracer::close_subtree: {:?}", self.trace);
        }
    }
    pub fn rollback(&mut self, depth: usize) {
        assert_eq!(self.rollbacks, 0);
        self.rollbacks = depth + 1;
        //trace!("TxTracer::rollback: {}", self.rollbacks);
    }
}

/// build a new Tracer, following along with a previous trace and noting differences
pub(crate) struct DeltaTracer {
    old: ReTracer,
    pub new: TxTracer,
}

impl DeltaTracer {
    pub fn new(old: Trace) -> Self {
        let old_len = old.buf.len();
        let old = ReTracer::new(old);
        let new = TxTracer::new (Tracer { buf: Vec::with_capacity(old_len), ..Tracer::default()});
        DeltaTracer { old, new }
    }
    pub fn finish(self) -> Trace {
        self.old.finish();
        self.new.finish()
    }

    pub fn push_byte(&mut self, data: u8) {
        self.old.push_byte(data);
        self.new.push_byte(data);
    }

    pub fn extend_bytes(&mut self, data: &[u8]) {
        self.old.extend_bytes(data);
        self.new.extend_bytes(data);
    }

    pub fn open_subtree(&mut self) -> Result<(), ()> {
        self.old.open_subtree()?;
        self.new.open_subtree();
        Ok(())
    }

    pub fn close_subtree(&mut self) -> Result<(), ()> {
        self.old.close_subtree()?;
        self.new.close_subtree();
        Ok(())
    }

    pub fn open_datum(&mut self) {
        self.old.open_datum();
        self.new.open_datum();
    }

    pub fn close_datum(&mut self) {
        self.old.close_datum();
        self.new.close_datum();
    }
}

impl Trace {
    pub fn toplevel_len(&self) -> usize {
        let mut syms = self.symbols();
        let mut n = 0;
        while let Some(s) = syms.next() {
            if let Symbol::Open = s {
                n += 1;
                close_subtree(&mut syms);
            }
        }
        n
    }
}

impl Trace {
    /// Test for an (anchored) match
    pub fn is_match(&self, input: &Trace) -> bool {
        // performance: parsing symbols is probably slow. Faster to compare optimistically, and
        // backtrack 1 byte on mismatch to check meta status
        is_match(self.symbols(), &mut input.symbols())
    }

    /// Return iterator of top-level matches
    pub fn toplevel_matches<'s, 'i>(&'s self, input: &'i IndexedTrace) -> ToplevelMatches<'s, 'i> {
        ToplevelMatches { pattern: self.symbols(), inputs: input.traces() }
    }

    /// Search the input for this pattern at any depth. Return an iterator that, for each match,
    /// yields Trace that will show a visitor where to find the matching expression in the tree.
    pub fn internal_matches<'s, 'i>(&'s self, input: &'i IndexedTrace) -> InternalMatches<'s, 'i> {
        let in_len = input.trace.buf.len();
        let input = input.trace.symbols();
        InternalMatches::new(self.symbols(), input, in_len)
    }
}

pub struct InternalMatches<'p, 'i> {
    pattern: Symbols<'p>,
    orig_input: Symbols<'i>,
    input: Symbols<'i>,
    in_len: usize,
}

impl<'p, 'i> InternalMatches<'p, 'i> {
    fn new(pattern: Symbols<'p>, input: Symbols<'i>, in_len: usize) -> Self {
        // Degenerate case, should prevent at pattern compilation time.
        assert!(pattern.buf.len() != 0);
        InternalMatches {
            pattern,
            orig_input: input.clone(),
            input,
            in_len,
        }
    }
}

impl<'p, 'i> Iterator for InternalMatches<'p, 'i> {
    type Item = Trace;

    fn next(&mut self) -> Option<Self::Item> {
        // A pattern can't match anything shorter than itself.
        while self.input.buf.len() >= self.pattern.buf.len() {
            let remaining = self.input.buf.len();
            let mut here = self.input.clone();
            // Consume a symbol.
            self.input.next().unwrap();
            // Performance note in is_match applies times N here.
            if is_match(self.pattern.clone(), &mut here) {
                let pos = self.in_len - remaining;
                let mut tracer = Tracer::default();
                tracer.buf.extend_from_slice(&self.orig_input.buf[..pos]);
                tracer.push_mvar(1);
                tracer.buf.extend_from_slice(&here.buf);
                let trace = tracer.finish();
                trace!("reconstructed: {:?}", trace);
                return Some(trace);
            }
        }
        return None;
    }
}


/////// The Matching. Expected Hot Path <<EOF

/// consume symbols until the current subtree has been closed
fn close_subtree(syms: &mut Symbols) {
    let mut n = 1;
    while n != 0 {
        match syms.next().unwrap() {
            Symbol::Open => n += 1,
            Symbol::Close => n -= 1,
            _ => (),
        }
    }
}

const CLOSER_LEN: usize = 2;

// NB. for efficiency, caller should handle quitting when input is shorter than pattern
fn is_match(pattern: Symbols, input: &mut Symbols) -> bool {
    //trace!("is_match:\n\tptn: {:?}\n\tinp: {:?}", pattern.clone(), input.clone());
    let mut bindings = Vec::new();
    for p in pattern {
        match (p, input.next()) {
            (ref x, Some(ref y)) if x == y => (),
            (Symbol::Meta(x), Some(Symbol::Open)) => {
                let x = x as usize;
                if x >= bindings.len() {
                    bindings.resize(x + 1, None);
                }
                if let Some(matched) = bindings[x] {
                    if !input.buf.starts_with(matched) {
                        //trace!("is_match: false; binding differed: {}", x);
                        return false;
                    }
                    //trace!("is_match: binding rematched: {}", x);
                    let (_, rest) = input.buf.split_at(matched.len() + CLOSER_LEN);
                    input.buf = rest;
                } else {
                    // consume the subtree and remember its dimensions
                    //trace!("is_match: bound a binding: {}", x);
                    let (from_match, from_match_len) = (input.clone(), input.buf.len());
                    close_subtree(input);
                    let subtree_len = from_match_len - input.buf.len() - CLOSER_LEN;
                    let (matched, _) = from_match.buf.split_at(subtree_len);
                    bindings[x] = Some(matched);
                }
            }
            (ref _x, ref _y) => {
                //trace!("is_match: false; structure differed {:02x?} // {:02x?}", _x, _y);
                return false;
            }
        }
    }
    //trace!("is_match: true!");
    return true;
}
