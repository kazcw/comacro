use log::trace;
use std::fmt::Debug;

const META: u8 = 255;
const OPEN: u8 = 254;
const CLOSE: u8 = 253;

/// builder of sentences in a regex language for trees
#[derive(Default)]
pub(crate) struct Trace {
    buf: Vec<u8>,
    datum: Option<usize>,
}

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

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// instead of emitting a Trace, follow along with a previous, noting any differing subtrees
pub(crate) struct ReTrace {
    buf: Vec<u8>,
    datum: Option<usize>,
    i: usize,
    diff_depth: usize,
}

impl ReTrace {
    pub fn new(buf: Vec<u8>) -> Self {
        ReTrace { buf, datum: None, i: 0, diff_depth: 0 }
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
    pub fn close_subtree(&mut self) -> Result<(), ()> {
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

/// transactional Trace builder that can >/dev/null the current subtree to a specified depth and
/// emit a replacement
#[derive(Default)]
pub(crate) struct TxTrace {
    trace: Trace,
    pub replacement: Trace,
    stack: Vec<usize>,
    rollbacks: usize,
}

impl TxTrace {
    pub fn new(trace: Trace) -> Self {
        TxTrace { trace, replacement: Trace::default(), stack: Vec::new(), rollbacks: 0 }
    }
    pub fn finish(self) -> Vec<u8> {
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
        //trace!("TxTrace::open_subtree: {:?}", self.trace);
    }
    pub fn close_subtree(&mut self) {
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
    pub fn rollback(&mut self, depth: usize) {
        assert_eq!(self.rollbacks, 0);
        self.rollbacks = depth + 1;
        //trace!("TxTrace::rollback: {}", self.rollbacks);
    }
}

/// build a new Trace, following along with a previous trace and noting differences
pub(crate) struct DeltaTrace {
    old: ReTrace,
    pub new: TxTrace,
}

impl DeltaTrace {
    pub fn new(old: Vec<u8>) -> Self {
        let old_len = old.len();
        let old = ReTrace::new(old);
        let new = TxTrace::new (Trace { buf: Vec::with_capacity(old_len), ..Default::default()});
        DeltaTrace { old, new }
    }
    pub fn finish(self) -> Vec<u8> {
        trace!("DeltaTrace::finish: new={:?}", self.new.trace);
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
