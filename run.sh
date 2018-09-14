#RUSTFLAGS='--cfg procmacro2_semver_exempt'
RUST_LOG=comacro=trace RUST_BACKTRACE=1 cargo run --example demo -- patredux.rs input.rs
