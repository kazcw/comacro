macro manual_swap() {
    let mut foo = 42;
    let mut bar = 23;
    let temp = foo;
    foo = bar;
    bar = temp;
}
