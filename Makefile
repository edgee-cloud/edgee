.PHONY: all
MAKEFLAGS += --silent

all: help

help:
	@grep -E '^[a-zA-Z1-9\._-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| sed -e "s/^Makefile://" -e "s///" \
		| awk 'BEGIN { FS = ":.*?## " }; { printf "\033[36m%-30s\033[0m %s\n", $$1, $$2 }'

dev.setup: ## Setup dev environment
	cd crates/wasmtime && wit-deps
	cargo build

dev.up: ## Launch locally
	cargo run

ci.check: ## Check the code
	cargo check

ci.build: ## Build release version
	cargo build --release

ci.test: ## Run tests
	cargo test -- --test-threads 1
