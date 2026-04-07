.PHONY: help build run test bench clean fmt lint doc install check

# Detect OS
ifeq ($(OS),Windows_NT)
    SHELL := powershell.exe
    .SHELLFLAGS := -NoProfile -Command
    CARGO := cargo
else
    CARGO := cargo
endif

help: ## Display this help message
ifeq ($(OS),Windows_NT)
	@Get-Content Makefile | Select-String -Pattern '^[a-zA-Z_-]+:.*?## .*$$' | ForEach-Object { $$match = $$_.Line -match '^([a-zA-Z_-]+):.*?## (.*)$$'; $$name = $$matches[1]; $$desc = $$matches[2]; Write-Host ("{0,-20}" -f $$name) -NoNewline -ForegroundColor Cyan; Write-Host $$desc }
else
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'
endif

build: ## Build the project
	$(CARGO) build

build-release: ## Build optimized release binary
	$(CARGO) build --release

run: ## Run the application
	$(CARGO) run

run-release: ## Run the release build
	$(CARGO) run --release

test: ## Run tests
	$(CARGO) test

test-verbose: ## Run tests with verbose output
	$(CARGO) test -- --nocapture --test-threads=1

bench: ## Run benchmarks
	$(CARGO) bench

clean: ## Clean build artifacts
ifeq ($(OS),Windows_NT)
	@powershell -Command "if (Test-Path target) { Remove-Item -Recurse -Force target }; if (Test-Path Cargo.lock) { Remove-Item -Force Cargo.lock }"
else
	$(CARGO) clean
endif

fmt: ## Format code
	$(CARGO) fmt --all

fmt-check: ## Check code formatting
	$(CARGO) fmt --all -- --check

lint: ## Run clippy linter
	$(CARGO) clippy --all-targets --all-features -- -D warnings

lint-fix: ## Fix clippy warnings automatically
	$(CARGO) clippy --fix --allow-dirty --allow-staged

check: ## Check code without building
	$(CARGO) check --all-targets --all-features

doc: ## Generate documentation
	$(CARGO) doc --no-deps --open

doc-private: ## Generate documentation including private items
	$(CARGO) doc --no-deps --document-private-items --open

install: ## Install the binary
	$(CARGO) install --path .

watch: ## Watch for changes and run tests
	$(CARGO) watch -x test

coverage: ## Generate test coverage report
ifeq ($(OS),Windows_NT)
	@powershell -Command "if (-not (Get-Command cargo-tarpaulin -ErrorAction SilentlyContinue)) { Write-Host 'Installing cargo-tarpaulin...' -ForegroundColor Yellow; cargo install cargo-tarpaulin }; cargo tarpaulin --out Html --output-dir coverage"
else
	@command -v cargo-tarpaulin >/dev/null 2>&1 || { echo "Installing cargo-tarpaulin..."; cargo install cargo-tarpaulin; }
	cargo tarpaulin --out Html --output-dir coverage
endif

audit: ## Audit dependencies for security vulnerabilities
ifeq ($(OS),Windows_NT)
	@powershell -Command "if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) { Write-Host 'Installing cargo-audit...' -ForegroundColor Yellow; cargo install cargo-audit }; cargo audit"
else
	@command -v cargo-audit >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit; }
	cargo audit
endif

bloat: ## Check binary size and dependencies
ifeq ($(OS),Windows_NT)
	@powershell -Command "if (-not (Get-Command cargo-bloat -ErrorAction SilentlyContinue)) { Write-Host 'Installing cargo-bloat...' -ForegroundColor Yellow; cargo install cargo-bloat }; cargo bloat --release"
else
	@command -v cargo-bloat >/dev/null 2>&1 || { echo "Installing cargo-bloat..."; cargo install cargo-bloat; }
	cargo bloat --release
endif

all: fmt lint test build ## Run formatter, linter, tests, and build

.DEFAULT_GOAL := help