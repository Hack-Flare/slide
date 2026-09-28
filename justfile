default: check

# Format all Rust crates.
fmt:
	cargo fmt --all

# Verify Rust formatting without changing files.
fmt-check:
	cargo fmt --all --check

# Build every workspace crate.
build-workspace:
	cargo build --workspace

# Lint every workspace crate and its test targets.
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
  cargo test --workspace --all-features

# Run formatting, linting, and tests.
check: fmt-check lint test

# Run dev
dev:
  cargo run -p slide_server -- --dev