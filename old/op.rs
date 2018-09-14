//! Opcodes and virtual machine

use syn::{Pat, Ident, Expr, Stmt};

use log::{trace, debug, info, error, warn, log};

// typed SSA register machine
// no(?) control flow
// destructuring ops

// coercion: raising is more general than lowering, so initial impl relies on raising;
// later as optimization add lowering coercions to compiler

// compact representation w/ normalized form:
// - implicit out params [sequential]
// - implicit S params [sequential]

#[derive(Debug, Default, Clone)]
pub struct Vm {
    // S is provided at execution time
    p: Vec<Pat>,
    x: Vec<Option<Expr>>,
    i: Vec<Ident>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Script {
    ops: Box<[Op]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Op {
    IToX(I),       // -> (X)
    //MExec(M),
    PAsI(P),       // -> (I)?
    SAsLocalX(S),  // -> (P, X)?
    SAsSemiX(S),   // -> (X)?
    TSet(u8),
    XAsAssignX(X), // -> (X, X)?
    XAsI(X),       // -> (I)?
    XEq(X, X),     // -> ()?
}

type I = u8;
type P = u8;
type S = u8;
type X = u8;
type M = u8;

/*
macro manual_swap($t: ident, $x: expr, $y: expr) {
    let $t = $x;
    $x = $y;
    $y = $t;
}

    // T_SET {END}

    // S_AS_LOCAL? S0 -> P0 X0
    // P_AS_I? P0 -> I0

    // S_AS_SEMI? S1 -> X1
    // X_AS_ASSIGN? X1 -> X2 X3
    // X_EQ? X2 X0

    // S_AS_SEMI? S2 -> X4
    // X_AS_ASSIGN? X4 -> X5 X6
    // X_EQ? X5 X3
    // I_TO_X I0 -> X7
    // X_EQ? X6 X7

    // M_EXEC M0
    // {END}
*/

pub fn manual_swap_script() -> Script {
    use self::Op::*;
    let ops = vec![
        SAsLocalX(0),
        PAsI(0),
        SAsSemiX(1),
        XAsAssignX(1),
        XEq(2, 0),
        SAsSemiX(2),
        XAsAssignX(4),
        XEq(5, 3),
        IToX(0),
        XEq(6, 7),
    ].into_boxed_slice();
    Script{ ops }
}

enum Control {
    Mark(u8),
    Jump,
    Continue,
}

impl Vm {
    pub fn new() -> Vm {
        Default::default()
    }

    /// Run the given script for the given block of statements.
    /// Any previous state is overwritten (but its storage is reused);
    /// reusing a Vm for the same script(s) minimizes allocation.
    pub fn run(&mut self, script: &Script, block: &[Stmt]) {
        self.reset();
        let mut t = 0;
        let mut i = 0;
        loop {
            let op = match script.ops.get(i) {
                Some(op) => op,
                None => break,
            };
            trace!("apply op#{} to {:?}", i, self);
            use self::Control::*;
            match op.apply(self, &block) {
                Continue => { i += 1 },
                Mark(u8) => { t = i + usize::from(u8) + 1 },
                Jump => {
                    assert!(t > i);
                    i = t;
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.p.clear();
        self.x.clear();
        self.i.clear();
    }

    pub fn i(&self, i: I) -> &Ident {
        &self.i[usize::from(i)]
    }

    pub fn x(&self, x: X) -> &Expr {
        self.x[usize::from(x)].as_ref().unwrap()
    }

    pub fn p(&self, p: P) -> &Pat {
        &self.p[usize::from(p)]
    }
}

fn i_to_x(ident: Ident) -> Expr {
    let mut segments = syn::punctuated::Punctuated::new();
    let arguments = syn::PathArguments::None;
    segments.push_value(syn::PathSegment { ident, arguments });
    let leading_colon = None;
    let path = syn::Path {
        leading_colon,
        segments,
    };
    let attrs = Vec::new();
    let qself = None;
    (syn::ExprPath { path, attrs, qself }).into()
}

fn p_as_i(p: &Pat) -> Option<Ident> {
    let p = match p {
        syn::Pat::Ident(p) => p,
        _ => return None,
    };
    // TODO: other stuff in PatIdent?
    let syn::PatIdent{ ident, .. } = p;
    Some(ident.clone())
}

fn s_as_local_x(s: &Stmt) -> Option<(Pat, Option<Expr>)> {
    let lx = match s {
        Stmt::Local(local) => local,
        _ => return None,
    };
    let syn::Local { pats, ty, init, .. } = lx;
    if pats.len() != 1 {
        // ?
        unimplemented!();
    }
    let p = (*pats.first().unwrap().value()).clone();
    let x = init.as_ref().map(|x| (&*x.1).clone());
    Some((p, x))
}

fn s_as_semi_x(s: &Stmt) -> Option<Expr> {
    Some(match s {
        syn::Stmt::Semi(x, _) => x.clone(),
        _ => return None,
    })
}

fn x_as_assign_x(x: &Expr) -> Option<(Expr, Expr)> {
    let (lhs, rhs) = match x {
        syn::Expr::Assign(syn::ExprAssign { left, right, .. }) => {
            (&**left, &**right)
        }
        _ => return None,
    };
    Some((lhs.clone(), rhs.clone()))
}

fn x_as_i(x: &Expr) -> Option<Ident> {
    let path = match x {
        syn::Expr::Path(syn::ExprPath { path, .. }) => path,
        _ => return None,
    };
    if path.segments.len() != 1 {
        return None;
    }
    let syn::PathSegment { ident, arguments } = path.segments.first().unwrap().value();
    if *arguments != syn::PathArguments::None {
        return None;
    }
    Some(ident.clone())
}

fn block_s(block: &[Stmt], s: u8) -> &Stmt {
    &block[usize::from(s)]
}

impl Op {
    fn apply(&self, state: &mut Vm, block: &[Stmt]) -> Control {
        use self::Control::*;
        use self::Op::*;
        match *self {
            IToX(i) => {
                trace!("IToX(I{})", usize::from(i));
                state.x.push(Some(i_to_x(state.i(i).clone())));
            }
            PAsI(p) => {
                trace!("PAsI(P{})", usize::from(p));
                match p_as_i(state.p(p)) {
                    Some(i) => state.i.push(i),
                    _ => return Jump,
                }
            }
            SAsLocalX(s) => {
                trace!("SAsLocalX(S{})", usize::from(s));
                let (p, x) = match s_as_local_x(block_s(block, s)) {
                    Some(px) => px,
                    None => return Jump,
                };
                state.p.push(p);
                //state.t.push(ty);
                state.x.push(x);
            }
            SAsSemiX(s) => {
                trace!("SAsSemiX(S{})", usize::from(s));
                let x = match s_as_semi_x(block_s(block, s)) {
                    Some(x) => x,
                    _ => return Jump,
                };
                state.x.push(Some(x));
            }
            TSet(u8) => return Mark(u8),
            XAsAssignX(x) => {
                trace!("XAsAssignX(X{})", usize::from(x));
                let (lhs, rhs) = match x_as_assign_x(state.x(x)) {
                    Some(lr) => lr,
                    _ => return Jump,
                };
                state.x.push(Some(lhs));
                state.x.push(Some(rhs));
            }
            XAsI(x) => {
                let i = match x_as_i(state.x(x)) {
                    Some(i) => i,
                    _ => return Jump,
                };
                state.i.push(i);
            }
            XEq(x0, x1) => {
                trace!("XEq(X{}, X{})", usize::from(x0), usize::from(x1));
                if state.x(x0) != state.x(x1) {
                    return Jump
                }
            }
            _ => unimplemented!(),
        }
        Continue
    }
}
