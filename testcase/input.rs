fn baz() {
    let mut thing1 = 23;
    let mut thing2 = 42;

    // let's manually swap some stuff!
    let temp = thing1;
    thing1 = thing2;
    thing2 = temp;

    // there are two things
    let _ = thing1 + thing2;

    // now let's do things with parentheses!
    let something_else_happening = (thing1 + thing1);
}
