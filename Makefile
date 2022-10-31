TESTFLAGS = -- --nocapture --test-threads=1
ARGS =
BASE = .

test:
	RUST_BACKTRACE=1 cargo test ${TESTFLAGS} ${ARGS}

bench:
	RUST_BACKTRACE=1 cargo bench ${ARGS}

profile = RUSTFLAGS='-g' cargo build --release --bin $(1); \
	valgrind --tool=callgrind --callgrind-out-file=callgrind.out	\
		--collect-jumps=yes --simulate-cache=yes		\
		${BASE}/target/release/$(1)
doc:
	cargo doc --no-deps ${ARGS}

profile.read_out:
	$(call profile,read_out)

profile.write_in:
	$(call profile,write_in)
