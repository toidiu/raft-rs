set -e

cargo fmt
cargo +nightly clippy -- -D warnings

cargo test

