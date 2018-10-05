#RUSTFLAGS='--cfg procmacro2_semver_exempt'
RUST_LOG=demo=trace,comacro=trace RUST_BACKTRACE=1 cargo run --example demo -- testcase/patterns.rs testcase/input.rs
