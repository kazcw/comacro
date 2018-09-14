/*

comacros: search code for patterns, by example

comacros are the dual of macros

as such, they are not used in compilation, but in static analysis: with a macro the user tells the
compiler to apply a pattern; with a comacro the compiler tells the user a pattern has been seen

Uses:
    - declarative lints
        - incl a generalization of a common lint type: search for anything that could be simplified
          with a libstd function
        - decl lints are a fraction the size and decoupled from compiler interals
    - RLS: search for things that could be "uninlined" to the given function/macro
    - search for patterns that could be factored into a utility (or made unnecessary with API changes?)

Execution model:
    - RLS: read a pattern from raw text, search some files
    - clippy: efficiently search for hundreds of patterns

*/

/*
 * - in pattern, normalize $placeholders to sequentially-numbered private identifiers
 *
 * diff patterns match in diff contexts:
 * e.g.
 * - stmt[+] -- matches a window of a sequence of statements
 * - item[+] -- matches a window of a sequence of items
 * - expr -- matches a subtree of an expr
 */

/*

type of expression:
- no quantification
- no alternation
- (no grouping)
- backreferences!

a stmt has a type and terms
    - shape of 2 stmts can be:
        - disjoin(A, B)
        - subset(A, B)
        - equiv(A, B)
    if a term is a backref, context may also be necessary to compare
FSM based on shape alone, then try unification?

 */

/*
backreferences form an equivalence relation
    set of terms X
    equiv relation partitions X into a family of sets α
    goal: identify patterns whose partitions are refinements of α
    - without calculating more of α than needed by matches
    - this can be handled with a binary decision tree that traverses the refinement poset, with
      merges cut and any paths that don't lead to any additional matches
        - e.g. at each step either branch to a state where a particular equivalence may hold, or doesn't
*/

/*
($x: expr, $y: expr) => { $x >= $y + 1 } => { lint_binop(GE, RT) }
($x: expr, $y: expr) => { $x >= 1 + $y } => { lint_binop(GE, RT) }
($x: expr, $y: expr) => { $x - 1 >= $y } => { lint_binop(GE, LF) }
($x: expr, $y: expr) => { -1 + $x >= $y } => { lint_binop(GE, LF) }
($x: expr, $y: expr) => { $x + 1 <= $y } => { lint_binop(LE, LF) }
($x: expr, $y: expr) => { 1 + $x <= $y } => { lint_binop(LE, LF) }
($x: expr, $y: expr) => { $x <= $y - 1 } => { lint_binop(LE, RT) }
($x: expr, $y: expr) => { $x <= -1 + $y } => { lint_binop(LE, RT) }

($v: expr, $x: expr, $b: block) => { if let Some($v) = $x.ok() $b }
*/

// no type
let $t = $x;
// any type [$ty:type]
let $t: $ty = $x;
// some type, no type, don't care
let $t: $_ = $x;
// Option<type> [$ty:type?]
let $t: $ty = $x;
// particular type
let $t: u32 = $x;


/*

match syntax-to-syntax, with out-of-band metavars:

- read patterns as TokenStreams (in Macro2 envelopes)
- scan stream for placeholders
- replace placeholders with trivial values appropriate for metavar type,
  remembering metavar positions
- parse stream to syn structures
- to match a position, first check if it's in the metavar table

later, performance improvements:

- linearize:
    - [] of unions, where context distinguishes between Kind Kinds
    enum Node { // this struct is a bitfield; whole thing fits into a scalar (u16?)
        Meta{...}
        Literal{
            arity: u8,
            kind: union Kind { // discriminate by context
                enum StmtKind {...}
                enum ExprKind {...}
            }
        }
    }
    - if not ismeta, an exact match is needed
        - can check for exact match with only arity; no switching on NodeKind
    - general traversal of this linearized structure would require a stack, but exact match and
      metavar match can be done only tracking stack _depth_
    - in pats, encode common literals as metas?
        - larger literals may need additional cell(s)
        ( probably better to just store all literals in cells? )
    - Spans etc OOB
        - i.e., operate directly on a serialized form ("homomorphic serialization")

- NFA-ish multimatch
    - it's like NFA-matching a regex, except the handling of metavars:
        - an unbound metavar always matches a single subtree, whose end can be determined by
          keeping a running total of stack depth
        - a bound mvar (backref) is more complicated...
            - could do:
                - when an unbound mvar matches, remember where (each mvar has a slot)
                - when a bound mvar matches, do the comparison and abort or continue
            - optimization:
                - an mvar not preceded by any mvar, $_, or optional always binds at a fixed offset

*/
