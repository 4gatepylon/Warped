# `Makefile` builds, runs, tests, and formats and lints the starter client.
#
# Interface:
#     make help: list the targets.
#     make run-client MANAGEMENT_SERVER_URL=<url>: build and play.
#     make test: run the tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#
# Example:
#     make run-client MANAGEMENT_SERVER_URL=http://10.0.0.5:8000
MANAGEMENT_SERVER_URL ?= http://127.0.0.1:8000

.PHONY: help run-client test fmt clean

help:
	@echo "Targets:"
	@echo "  make run-client MANAGEMENT_SERVER_URL=<url>   Build and play (default $(MANAGEMENT_SERVER_URL))"
	@echo "  make test                                     Run the tests"
	@echo "  make fmt                                      Format with rustfmt and lint with clippy"
	@echo "  make clean                                    Remove .build/"
	@echo ""
	@echo "Example: make run-client MANAGEMENT_SERVER_URL=http://10.0.0.5:8000"

run-client:
	cargo run --release -- --management-server-url $(MANAGEMENT_SERVER_URL)

test:
	cargo test --release

fmt:
	cargo fmt
	cargo clippy --release -- -D warnings

clean:
	rm -rf .build
