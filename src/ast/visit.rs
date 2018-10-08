use syn::visit::Visit;

pub trait Visitor<'ast> {
    type Output;

    fn finish(self) -> Self::Output;

    fn open_expr(&mut self, _: &'ast syn::Expr) -> Result<(), ()> {
        self.open_subtree();
        Ok(())
    }
    fn open_ident(&mut self, _: &'ast syn::Ident) -> Result<(), ()> {
        self.open_subtree();
        Ok(())
    }
    fn open_stmt(&mut self, _: &'ast syn::Stmt) {
        self.open_subtree()
    }
    fn open_pat(&mut self, _: &'ast syn::Pat) {
        self.open_subtree()
    }
    fn open_lit_int(&mut self, _: &'ast syn::LitInt) {
        self.open_datum()
    }

    fn close_expr(&mut self, _: &'ast syn::Expr) {
        self.close_subtree()
    }
    fn close_ident(&mut self, _: &'ast syn::Ident) {
        self.close_subtree()
    }
    fn close_stmt(&mut self, _: &'ast syn::Stmt) {
        self.close_subtree()
    }
    fn close_pat(&mut self, _: &'ast syn::Pat) {
        self.close_subtree()
    }
    fn close_lit_int(&mut self, _: &'ast syn::LitInt) {
        self.close_datum()
    }

    fn open_subtree(&mut self);
    fn close_subtree(&mut self);
    fn open_datum(&mut self);
    fn close_datum(&mut self);
    fn push_byte(&mut self, x: u8);
    fn extend_bytes(&mut self, x: &[u8]);

    fn visit<'v: 'ast, V: Visitable<'ast>+?Sized>(mut self, v: &'v V) -> Self::Output where Self: Sized {
        v.apply(&mut self);
        self.finish()
    }

    fn apply<'v: 'ast, V: Visitable<'ast>+?Sized>(v: &'v V) -> Self::Output where Self: Sized+Default {
        let mut viz = <Self as Default>::default();
        v.apply(&mut viz);
        viz.finish()
    }
}

pub trait Visitable<'a> {
    fn apply<'v, V: Visitor<'a>>(&'a self, v: &'v mut V);
}

impl<'a> Visitable<'a> for syn::Expr {
    fn apply<'v, V: Visitor<'a>>(&'a self, v: &'v mut V) {
        let mut v = SynVis { inner: v };
        v.visit_expr(self);
    }
}

impl<'a> Visitable<'a> for syn::Stmt {
    fn apply<'v, V: Visitor<'a>>(&'a self, v: &'v mut V) {
        let mut v = SynVis { inner: v };
        v.visit_stmt(self);
    }
}

impl<'a> Visitable<'a> for [syn::Stmt] {
    fn apply<'v, V: Visitor<'a>>(&'a self, v: &'v mut V) {
        let mut v = SynVis { inner: v };
        for s in self {
            v.visit_stmt(s);
        }
    }
}

/*
// TODO: make something like this work?
impl<'a, 't, T> Visitable<'a> for T where T: AsRef<&'a [syn::Stmt]> {
    fn apply<'v, V: Visitor<'a>>(&'a self, v: &'v mut V) {
        self.as_ref().apply(v)
    }
}
*/

/// Outer visitor ensures AST->Trace lowering is consistent for different traversal modes
struct SynVis<'v, V> {
    inner: &'v mut V,
}

impl<'ast, 'v, V: Visitor<'ast>> Visit<'ast> for SynVis<'v, V> {
    fn visit_stmt(&mut self, x: &'ast syn::Stmt) {
        self.inner.open_stmt(x);
        self.inner
            .push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_stmt(self, x);
        self.inner.close_stmt(x);
    }

    fn visit_expr(&mut self, x: &'ast syn::Expr) {
        if let Err(()) = self.inner.open_expr(x) {
            return;
        }
        self.inner
            .push_byte(unsafe { std::mem::transmute::<_, u64>(std::mem::discriminant(x)) } as u8);
        syn::visit::visit_expr(self, x);
        self.inner.close_expr(x);
    }

    fn visit_pat(&mut self, x: &'ast syn::Pat) {
        self.inner.open_pat(x);
        syn::visit::visit_pat(self, x);
        self.inner.close_pat(x);
    }

    fn visit_ident(&mut self, x: &'ast syn::Ident) {
        if let Err(()) = self.inner.open_ident(x) {
            return;
        }
        self.inner.extend_bytes(x.to_string().as_bytes());
        self.inner.close_ident(x);
    }

    fn visit_lit_int(&mut self, x: &'ast syn::LitInt) {
        self.inner.open_lit_int(x);
        // TODO: compact repr?
        self.inner.extend_bytes(format!("{}", x.value()).as_bytes());
        self.inner.close_lit_int(x);
    }
}
