FUZZ_TARGET := fuzz::raft

# The fuzz test lives in the sim crate, which is the only one that links a libFuzzer runtime.
# Without -p, cargo also builds raft-rs own lib test, which still gets coverage instrumentation and
# then fails to link on undefined __sanitizer_cov_* symbols.
FUZZ_ARGS := -p sim --sanitizer NONE

.PHONY: fuzz bugs

# Fuzz for a bounded time. Anything found is written to the crashes dir and replays on every plain
# cargo test from then on.
fuzz:
	cargo bolero test $(FUZZ_TARGET) $(FUZZ_ARGS) -T 30s

bugs:
	RUST_BACKTRACE=1 cargo test -p sim --color always -- bug
