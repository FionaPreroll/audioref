#############################################
# Makefile for AudioRef
#
# Targets:
#   all           : Run tests and bundle the plugin (default target).
#   build         : Build the Cargo workspace in debug mode.
#   bundle        : Build the plugin in release mode and bundle it as VST3 and CLAP in target/bundled.
#   lint          : Check formatting (cargo fmt), run the linter (cargo clippy), check the docs (cargo doc) and that audioref-core stays free of plugin APIs, GUI and I/O.
#   lint_fix      : Automatically apply suggestions of the compiler and Clippy, and format the code.
#   check         : Everything a pull request must pass: lint, coverage, deny and, if installed, zizmor.
#   test          : Run the Cargo tests.
#   coverage      : Measure the test coverage and fail below COVERAGE_MIN percent of lines or functions (needs cargo-llvm-cov).
#   deny          : Check the dependencies' advisories, licenses and sources (needs cargo-deny).
#   zizmor        : Audit the GitHub Actions workflows for security problems (needs zizmor).
#   fuzz          : Run each fuzz target for FUZZ_SECONDS seconds (needs nightly Rust and cargo-fuzz).
#   screenshots   : Take screenshots of the window, in English and German, to target/screenshots (Linux).
#   clean         : Clean build artifacts using Cargo's built-in clean command.
#   version       : Prepare a release: set VERSION in Cargo.toml, Cargo.lock and CHANGELOG.md.
#
# Usage Examples:
#   make check
#       Runs all checks of a pull request.
#
#   make bundle
#       Writes AudioRef.vst3 and AudioRef.clap to target/bundled.
#
#   make fuzz FUZZ_SECONDS=600
#       Runs each fuzz target for ten minutes.
#
#   make screenshots
#       Writes en-correction.png, de-correction.png and the others to target/screenshots.
#
#   make version VERSION=0.1.0
#       Prepares the release of version 0.1.0.
#############################################

.PHONY: all build bundle lint lint_fix check test coverage deny zizmor fuzz screenshots clean version

# Seconds to run each fuzz target for, see the target fuzz.
FUZZ_SECONDS ?= 60

# The share of lines, and of functions, that the tests must cover, in percent, see the target
# coverage.
COVERAGE_MIN ?= 100

# Crates that audioref-core must not depend on, not even indirectly: plugin APIs, GUIs and the
# platform belong to the plugin, see crates/audioref-core/src/lib.rs.
CORE_FORBIDDEN = nih_plug|nih-plug|clap-sys|vst3|baseview|egui|vizia|iced|sys-locale|libc|windows|objc

# Crates whose tests can't reach all of their code without a host, and which coverage leaves out:
# the plugin, which only translates between nih-plug and audioref-core, and xtask, which hands
# over to nih-plug's bundler or runs curl (fetch-profiles). clap-validator and pluginval check the plugin in CI instead, see
# Tests and Coverage in docs/DEVELOPMENT.md.
COVERAGE_EXCLUDE = --exclude audioref --exclude xtask

# Default target: test, then bundle the plugin.
all: test bundle

# ------------------------------
# Build targets.
# ------------------------------
# Build the Cargo workspace (debug mode).
# * (This target is called by CI as well.)
build:
	@echo "Building Cargo workspace (debug mode)..."
	@cargo build --workspace $(ARGS)

# Build the plugin in release mode and bundle it as AudioRef.vst3 and AudioRef.clap.
# * (This target is called by CI as well, with bundle-universal on macOS.)
bundle:
	@echo "Bundling the plugin (release mode)..."
	@cargo xtask bundle audioref --release $(ARGS)
	@ls -l target/bundled

# ------------------------------
# Lint targets.
# ------------------------------
# Check the formatting, run Clippy with all warnings as errors, and build the docs, whose
# warnings, e.g. broken links, are errors too.
# * (This target is called by CI as well.)
lint:
	@echo "Checking Rust formatting (cargo fmt)..."
	@cargo fmt --check
	@echo "Running Rust linter (cargo clippy)..."
	@cargo clippy --workspace --all-targets --all-features $(ARGS) -- -D warnings
	@echo "Checking the documentation (cargo doc)..."
	@RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features $(ARGS)
	@echo "Checking that audioref-core stays free of plugin APIs, GUI and I/O (cargo tree)..."
	@if cargo tree --package audioref-core --edges normal --prefix none $(ARGS) | grep -E '^($(CORE_FORBIDDEN))[a-z0-9_-]* v'; then \
		echo "audioref-core must not depend on these crates, see crates/audioref-core/src/lib.rs."; \
		exit 1; \
	fi

# Automatically fix lint issues.
lint_fix:
	@echo "Running cargo clippy --fix to automatically apply suggestions..."
	@cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets --all-features
	@echo "Running cargo fmt to format Rust files..."
	@cargo fmt

# Everything that CI checks for a pull request, except the bundles and their validation.
# zizmor runs offline here, without asking GitHub; CI's run asks it.
check: lint coverage deny
	@if command -v zizmor >/dev/null 2>&1; then \
		$(MAKE) --no-print-directory zizmor ZIZMOR_ARGS=--offline; \
	else \
		echo "zizmor is not installed: skipping the audit of the workflows, which CI runs."; \
	fi

# ------------------------------
# Test targets.
# ------------------------------
# Run the Cargo tests.
# * (This target is called by CI as well.)
test:
	@echo "Running Cargo tests..."
	@cargo test --workspace $(ARGS)

# Measure the coverage of the Cargo tests, without the crates in COVERAGE_EXCLUDE, and fail if
# they cover less than COVERAGE_MIN percent of the lines or of the functions. Prints a summary
# per file and the lines that no test runs, and writes an HTML report with the covered lines to
# target/llvm-cov/html and an LCOV report to target/llvm-cov/lcov.info.
# Install the tools with: rustup component add llvm-tools-preview && cargo install cargo-llvm-cov
# * (This target is called by CI as well.)
coverage:
	@echo "Measuring test coverage..."
	@# Data of earlier runs, e.g. of files that were moved since, would count too.
	@cargo llvm-cov clean --workspace
	@cargo llvm-cov --workspace $(COVERAGE_EXCLUDE) --all-features --no-report $(ARGS)
	@cargo llvm-cov report --html
	@cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info
	@cargo llvm-cov report --show-missing-lines \
		--fail-under-lines $(COVERAGE_MIN) --fail-under-functions $(COVERAGE_MIN)

# Check the dependencies for known vulnerabilities, unmaintained crates, licenses that don't go
# with the GPL and sources other than crates.io and the pinned git repositories, as configured in
# deny.toml.
# Install the tool with: cargo install cargo-deny
# * (This target is called by CI as well.)
deny:
	@echo "Checking dependencies (cargo deny)..."
	@cargo deny --locked check

# Audit the GitHub Actions workflows, e.g. for template injection or more permissions than they
# need. With a token in GH_TOKEN, also with what GitHub knows, e.g. actions with known
# vulnerabilities or pins to commits that aren't in the action's repository. Options for zizmor
# go in ZIZMOR_ARGS, as ARGS holds those for cargo, e.g. in make check ARGS=--locked.
# Install the tool with: pipx install zizmor
# * (This target is called by CI as well.)
zizmor:
	@echo "Auditing the workflows (zizmor)..."
	@zizmor $(ZIZMOR_ARGS) .github/

# Run each fuzz target in fuzz/ for FUZZ_SECONDS seconds, e.g. make fuzz FUZZ_SECONDS=600.
# Stops at the first failure; fuzz/README.md describes how to reproduce it.
# Install the tools with: rustup toolchain install nightly && cargo install cargo-fuzz
# * (This target is called by CI as well.)
fuzz:
	@cd fuzz && for target in $$(cargo +nightly fuzz list); do \
		echo "Fuzzing $$target for $(FUZZ_SECONDS) seconds..."; \
		cargo +nightly fuzz run $$target -- -max_total_time=$(FUZZ_SECONDS) -dict=audioref.dict || exit 1; \
	done

# Take screenshots of the window, without a DAW or a graphics card: the plugin runs as a
# standalone program on Xvfb, see Screenshots in docs/DEVELOPMENT.md. Linux only.
# Install the tools with: utils/install-linux-deps.sh --screenshots
# * (This target is called by CI as well.)
screenshots:
	@./utils/screenshots.sh

# Prepare a release, e.g. make version VERSION=0.1.0, see Changelog and versions in
# docs/DEVELOPMENT.md.
version:
	@./utils/set-version.sh "$(VERSION)"

# ------------------------------
# Clean target.
# ------------------------------
# Clean build artifacts using Cargo's built-in clean command.
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
