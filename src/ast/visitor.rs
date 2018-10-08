pub(crate) trait Visitor<'ast> {
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
}
