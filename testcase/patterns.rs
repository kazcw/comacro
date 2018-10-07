macro manual_swap($t: ident, $x: expr, $y: expr) {
    let $t = $x;
    $x = $y;
    $y = $t;
}

macro test_bracketed($x:expr) {
    ($x + $x)
}

macro test_bracketed2($x:expr, $y:ident) {
    ($x + $y)
}
