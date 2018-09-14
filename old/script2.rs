
enum Op {
    // lowering ops
    PatAsIdent,
    StmtAsLocal,
    StmtAsItem,
    StmtAsExpr,
    StmtAsSemi,

    // terminals
    Ident
}

fn do_expr(x: &Expr) {
}

fn compile(stmts: &[syn::Stmt]) -> Script {
    for stmt in stmts {
        use syn::Stmt::*;
        match stmt {
            Local(syn::Local { attrs, pats, ty, init, .. }) => {
                ops.push(Op::StmtAsLocal);
                // attrs
                do_pats(pats);
                do_ty(ty);
                do_expr(init);

                ops.push(Op::StmtAsLocal(attrs.len(), pats.len(), ty.is_some(), init.is_some()));
                // ...

                // mutate to add optional fields
                ops.push(Op::StmtAsLocal(ty.is_some(), init.is_some()));
                push_pats(pats);
                // ...
            }
            Item(i) => {
                ops.push(Op::StmtAsItem);
                unimplemented!();
            }
            Expr(x) => {
                ops.push(Op::StmtAsExpr);
                do_expr(Some(x));
            }
            Semi(x, _) => {
                ops.push(Op::StmtAsSemi);
                do_expr(Some(x));
            }
        }
    }
}

