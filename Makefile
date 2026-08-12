FUZZ_TARGET := fuzz::fuzz_raft

# The fuzz test lives in the sim crate, which is the only one that links a libFuzzer runtime.
# Without -p, cargo also builds raft-rs own lib test, which still gets coverage instrumentation and
# then fails to link on undefined __sanitizer_cov_* symbols.
FUZZ_ARGS := -p sim --sanitizer NONE

.PHONY: fuzz fuzz-reduce

# Fuzz for a bounded time. Anything found is written to the crashes dir and replays on every plain
# cargo test from then on.
fuzz:
	cargo bolero test $(FUZZ_TARGET) $(FUZZ_ARGS) -T 30s

# Drop corpus inputs that reach nothing the remaining inputs already reach.
#
# Deliberately not libFuzzer -merge=1 through --engine-args. Bolero always passes corpus and
# crashes as the two positional dirs, so -merge would fold known crash inputs into the corpus and
# leave every later run failing on its own seeds. `reduce` only ever reads the corpus dir.
fuzz-reduce:
	cargo bolero reduce $(FUZZ_TARGET) $(FUZZ_ARGS)
