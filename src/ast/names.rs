pub(crate) trait Discrim {
    fn discrim(&self) -> &'static str;
}

impl Discrim for syn::Expr {
    fn discrim(&self) -> &'static str {
        use syn::Expr::*;
        match self {
            Box(..) => "Box",
            InPlace(..) => "InPlace",
            Array(..) => "Array",
            Call(..) => "Call",
            MethodCall(..) => "MethodCall",
            Tuple(..) => "Tuple",
            Binary(..) => "Binary",
            Unary(..) => "Unary",
            Lit(..) => "Lit",
            Cast(..) => "Cast",
            Type(..) => "Type",
            Let(..) => "Let",
            If(..) => "If",
            While(..) => "While",
            ForLoop(..) => "ForLoop",
            Loop(..) => "Loop",
            Match(..) => "Match",
            Closure(..) => "Closure",
            Unsafe(..) => "Unsafe",
            Block(..) => "Block",
            Assign(..) => "Assign",
            AssignOp(..) => "AssignOp",
            Field(..) => "Field",
            Index(..) => "Index",
            Range(..) => "Range",
            Path(..) => "Path",
            Reference(..) => "Reference",
            Break(..) => "Break",
            Continue(..) => "Continue",
            Return(..) => "Return",
            Macro(..) => "Macro",
            Struct(..) => "Struct",
            Repeat(..) => "Repeat",
            Paren(..) => "Paren",
            Group(..) => "Group",
            Try(..) => "Try",
            Async(..) => "Async",
            TryBlock(..) => "TryBlock",
            Yield(..) => "Yield",
            Verbatim(..) => "Verbatim",
        }
    }
}

impl Discrim for syn::Stmt {
    fn discrim(&self) -> &'static str {
        use syn::Stmt::*;
        match self {
            Local(..) => "Local",
            Item(..) => "Item",
            Expr(..) => "Expr",
            Semi(..) => "Semi",
        }
    }
}

impl Discrim for syn::Pat {
    fn discrim(&self) -> &'static str {
        use syn::Pat::*;
        match self {
            Wild(..) => "Wild",
            Ident(..) => "Ident",
            Struct(..) => "Struct",
            TupleStruct(..) => "TupleStruct",
            Path(..) => "Path",
            Tuple(..) => "Tuple",
            Box(..) => "Box",
            Ref(..) => "Ref",
            Lit(..) => "Lit",
            Range(..) => "Range",
            Slice(..) => "Slice",
            Macro(..) => "Macro",
            Verbatim(..) => "Verbatim",
        }
    }
}
