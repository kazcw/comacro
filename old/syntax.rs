// x: Any / Option<Any>
let $foo: $bar = $baz;

// _: Option<Any>
let $foo: $_ = $baz;
// _: Option<Any>
let $foo = $baz;
// _: None
let $foo: $! = $baz;
// _: None
let $foo = $baz;

// literal without explicit wildcards:
// (<nothing> signifies None)
    // _: Option<Any>
    #[$_]
    let $foo: $_ = $_;
    // _: None
    let $foo;
// - this approach compiles straightforwardly
// - "match this function" probably wants a loosening transform
// - explicit wildcards are easy to understand

// wildcard unless literal:
// (<nothing> signifies unspecified)
    // _: Option<Any>
    let $foo;
    // _: None
    #[$!]
    let $foo: $! = $!;
// - this is the only sane approach for attributes
//   - it would be nice if everything were consistent

// hybrid:
// - attributes are metadata; metadata is ignored by default
// - terms are literal unless explicitly wildcarded
//   - explicit-wildcards is probably noisier in the common case, but it's also POLA
//
// out-of-band metadata:
// - attributes
// - mapping from serialization back to syntax (suitable for presenting syn objects to onmatch fn)

let $foo: $bar = $baz;
let $foo: $_ = $baz;
let $foo: $! = $baz;

let $t = $x;
$x = $y;
$y = $t;
