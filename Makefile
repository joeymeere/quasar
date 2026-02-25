SHELL := /usr/bin/env bash
NIGHTLY_TOOLCHAIN := nightly

.PHONY: format \
	clippy test check-features build all-checks nightly-version


# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

format:
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all -- --check

format-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all

clippy:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets -- -D warnings

clippy-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets --fix --allow-dirty --allow-staged -- -D warnings

check-features:
	@cargo hack --feature-powerset --no-dev-deps check

build:
	@cargo build

test:
	@$(MAKE) build
	@cargo test --all-features

# Run all checks in sequence
all-checks:
	@echo "Running all checks..."
	@$(MAKE) format
	@$(MAKE) clippy
	@$(MAKE) check-features
	@$(MAKE) test
	@echo "All checks passed!"