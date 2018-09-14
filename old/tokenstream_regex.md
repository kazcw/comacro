# TokenStream regex approach

- need to match on syntactic features
- but, simpler to compare sequences of uniform objects than trees with many
  types of node
    - so: tokens -> syntax -> tokens, to inject information about syntactic
      structure into the token layer

syn:
    extend tokens to support wildcards/backreferences, grouping/repetition, 
    extend syntax to support metavar bindings/backrefs
        - metavar impl of ToTokens produces suitable regex

- this approach seems easy to implement
    - doesn't require special handling for every type of AST node

problem: regex to recognize an Expr is hideous, no?
    - just create an AnyExpr token; don't need to lower it to an alternation of
      all concrete Expr kinds

## Challenges

- metavar matching will be slow (have to parse an Expr to know when it ends!)
    - possible to parse once for structure?

let $id: $ty = $expr;
let $id (: $ty)? (= $expr)?;

tokenization of a mvar depends on context!
    - normally, parent inserts delimiters based on present/not-present
    - mvars can be optional

------------------------------------------------

object-level:

- AnyExpr/BindAnyExpr, AnyIdent/BindAnyIdent, ...

Local {
    Vec<attrs>,
    Seq<Pat>,
    Option<Type>,
    Option<Expr>,
}
- pattern needs to be able to specify:
    match exact attrs (incl none) / match any attrs (optionally bind)

Local {
    Pattern<Vec<attrs>>,
    Pattern<Seq<Pat>>,
    Pattern<Option<Type>>,
    Pattern<Option<Expr>>,
}

Pattern<T> {
    Concrete(T),
    BindAny, // separate IgnoreAny?
    Backref(Backref<T>),
}

Local: {
    attrs: [],
    pats: ?,
    type: None,
    init: Some(Expr: { ... }),
}

Local: {
    attrs: Pattern<Vec<Attr>>::BindAny,
    pats: ?,
    type: Pattern<Option<Type>>::BindAny,
    init: Pattern<Option<Expr>>::BindAny,
}

```
Local{ attrs: ?_, pats: Pat{ id: ?t }, type: ?_, init: Expr{ ExprPath: [ PathSeg{ ident: ?x } ] } }
Semi{ Expr{ ExprAssign{ pat: [ Pat{ id: ?x } ], expr: ?y } } }
Semi{ Expr{ ExprAssign{ pat: [ Pat{ id: ?y } ], expr: Expr{ ExprPath: [ PathSeg{ ident: ?t } ] } } } }
```

serialize to self-describing TLV format
    - length allows wildcards/skipping
    - tags allow Pattern handling

