set -e

cargo fmt
cargo +nightly clippy -- -D warnings -A clippy::uninlined-format-args

cargo test

