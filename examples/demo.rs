use log::*;

use comacro::PatternDef;

fn main() {
    env_logger::init();

    use std::env;
    use std::fs::File;
    use std::io::Read;
    use std::process;

    let mut args = env::args();
    let _ = args.next(); // executable name
    let (patterns, input) = match (args.next(), args.next(), args.next()) {
        (Some(patterns), Some(input), None) => (patterns, input),
        _ => {
            error!("Usage: dump-syntax patterns.rs input.rs");
            process::exit(1);
        }
    };
    let mut src = String::new();
    File::open(&input)
        .expect("Unable to open input file")
        .read_to_string(&mut src)
        .expect("Unable to read input file");
    let insyn = syn::parse_file(&src).expect("Unable to parse input file");
    src.clear();
    File::open(&patterns)
        .expect("Unable to open pattern file")
        .read_to_string(&mut src)
        .expect("Unable to read pattern file");
    let patsyn = syn::parse_file(&src).expect("Unable to parse pattern file");
    let pats: Vec<_> = patsyn.items.iter().map(compile_pat).collect();
    for it in insyn.items {
        use syn::Item::*;
        match it {
            Fn(f) => {
                let input = &f.block.stmts;
                let in_trace = comacro::compile_input(input);
                for (i, pat) in pats.iter().enumerate() {
                    for m in pat.matches(input, &in_trace) {
                        println!("pattern {} is a match: {:?}", i, m);
                    }
                }
            }
            _ => unimplemented!("non-fn item in input"),
        }
    }
}

fn compile_pat(item: &syn::Item) -> comacro::Ir {
    let pat = if let syn::Item::Macro2(mac) = item {
        PatternDef::lex(mac.args.clone(), mac.body.clone())
    } else {
        unimplemented!()
    };
    let pat = pat.parse().unwrap();
    /*
    let pat = match pat.parse() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("error: {}", e.annotate(&src));
            panic!()
        }
    };
    */
    trace!("debug_tree_repr: {}", pat.debug_tree_repr());
    trace!("debug_flat_repr:\n{}", pat.debug_flat_repr());
    pat.compile()
}
