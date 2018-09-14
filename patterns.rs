macro manual_swap($t: ident, $x: expr, $y: expr) {
    let $t = $x;
    $x = $y;
    $y = $t;
}
