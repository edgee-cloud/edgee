.PHONY: all
MAKEFLAGS += --silent

all: help

help:
	@grep -E '^[a-zA-Z1-9\._-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| sed -e "s/^Makefile://" -e "s///" \
		| awk 'BEGIN { FS = ":.*?## " }; { printf "\033[36m%-30s\033[0m %s\n", $$1, $$2 }'

dev.wit-deps: ## Install wit-deps
	cd crates/components-runtime && wit-deps

dev.setup: ## Setup dev environment
	make dev.wit-deps
	cargo build

dev.up: ## Launch locally
	cargo run serve

ci.check: ## Check the code
	cargo check

ci.build: ## Build release version
	cargo build --release

ci.test: ## Run tests
	cargo test --locked

test.coverage:
	cargo llvm-cov --all-features

test.coverage.lcov:
	cargo llvm-cov --all-features --lcov --output-path lcov.info

test.coverage.html:
	cargo llvm-cov --all-features --open
