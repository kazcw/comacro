pattern is same, but a DAG:
    a node may have a label
    a node may be a reference to a previous label

skip over a child in constant time
    (optionally while binding it to a label)
    could have node-size prefix

a normal stack machine constructs bottom-up
    we need top-down context for matching (metas etc)

destructive machine:
    - typed stack machine
    - running a script forward would destruct an AST
    - a script is *constructive* if:
        - it doesn't contain any non-constructive operators (wildcards, partial extractors, repetitions, ..?)
        - it destructs all the way to terminals
    - running a constructive script *backward* would construct an AST
        - read it backwards and do the inverse of each operator

    a comacro pattern compiles to a script
    an input compiles to a constructive script

    a pattern can be matched against an input in linear time
        - this does not require actually executing any operations (only the
          script needs be consulted, and no additional storage is necessary

