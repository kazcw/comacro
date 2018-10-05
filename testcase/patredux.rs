macro manual_swap($x: expr, $y: expr) {
    let temp = $x;
    $x = $y;
    $y = temp;
}
