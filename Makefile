# Private Poker - Makefile for common development tasks
#
# Usage:
#   make help          - Show this help message
#   make build         - Build release binaries
#   make dev           - Build dev binaries
#   make test          - Run all tests
#   make test-coverage - Run tests with coverage report
#   make clean         - Clean build artifacts
#   make db-setup      - Set up PostgreSQL database
#   make db-migrate    - Run database migrations
#   make db-reset      - Reset database (drop and recreate)
#   make run-server    - Run the poker server
#   make run-client    - Run the poker client
#   make fmt           - Format code with rustfmt
#   make lint          - Run clippy lints
#   make check         - Run fmt + lint + test
#   make docker-build  - Build Docker image
#   make docker-run    - Run Docker container

.PHONY: help build dev test test-coverage clean db-setup db-migrate db-reset run-server run-client fmt lint check docker-build docker-run

# Default target
.DEFAULT_GOAL := help

# Environment variables
DATABASE_URL ?= postgresql://postgres:7794951@localhost:5432/poker_db
RUST_LOG ?= info
BIND_ADDR ?= 0.0.0.0:8080
SERVER_URL ?= http://localhost:8080

# Colors for output
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(GREEN)Private Poker - Development Makefile$(NC)"
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-20s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "Environment variables:"
	@echo "  DATABASE_URL=$(DATABASE_URL)"
	@echo "  RUST_LOG=$(RUST_LOG)"
	@echo "  BIND_ADDR=$(BIND_ADDR)"
	@echo "  SERVER_URL=$(SERVER_URL)"

build: ## Build release binaries
	@echo "$(GREEN)Building release binaries...$(NC)"
	cargo build --release
	@echo "$(GREEN)Build complete! Binaries in target/release/$(NC)"

dev: ## Build dev binaries (faster, with debug info)
	@echo "$(GREEN)Building dev binaries...$(NC)"
	cargo build
	@echo "$(GREEN)Build complete! Binaries in target/debug/$(NC)"

test: ## Run all tests
	@echo "$(GREEN)Running all tests...$(NC)"
	DATABASE_URL="$(DATABASE_URL)" cargo test --workspace
	@echo "$(GREEN)All tests passed!$(NC)"

test-coverage: ## Run tests with coverage report (requires cargo-llvm-cov)
	@echo "$(GREEN)Running tests with coverage...$(NC)"
	@if ! command -v cargo-llvm-cov &> /dev/null; then \
		echo "$(RED)cargo-llvm-cov not found. Install with: cargo install cargo-llvm-cov$(NC)"; \
		exit 1; \
	fi
	DATABASE_URL="$(DATABASE_URL)" cargo llvm-cov --workspace --html
	@echo "$(GREEN)Coverage report generated at target/llvm-cov/html/index.html$(NC)"

clean: ## Clean build artifacts
	@echo "$(YELLOW)Cleaning build artifacts...$(NC)"
	cargo clean
	@echo "$(GREEN)Clean complete!$(NC)"

db-setup: ## Set up PostgreSQL database
	@echo "$(GREEN)Setting up database: poker_db$(NC)"
	-psql -U postgres -c "CREATE DATABASE poker_db;"
	@echo "$(GREEN)Database created!$(NC)"

db-migrate: ## Run database migrations
	@echo "$(GREEN)Running database migrations...$(NC)"
	@if ! command -v sqlx &> /dev/null; then \
		echo "$(RED)sqlx-cli not found. Install with: cargo install sqlx-cli --no-default-features --features postgres$(NC)"; \
		exit 1; \
	fi
	DATABASE_URL="$(DATABASE_URL)" sqlx migrate run
	@echo "$(GREEN)Migrations complete!$(NC)"

db-reset: ## Reset database (drop and recreate)
	@echo "$(YELLOW)Resetting database...$(NC)"
	-psql -U postgres -c "DROP DATABASE IF EXISTS poker_db;"
	psql -U postgres -c "CREATE DATABASE poker_db;"
	DATABASE_URL="$(DATABASE_URL)" sqlx migrate run
	@echo "$(GREEN)Database reset complete!$(NC)"

run-server: ## Run the poker server
	@echo "$(GREEN)Starting poker server on $(BIND_ADDR)...$(NC)"
	DATABASE_URL="$(DATABASE_URL)" RUST_LOG="$(RUST_LOG)" cargo run --release --bin pp_server -- --bind $(BIND_ADDR)

run-client: ## Run the poker client
	@echo "$(GREEN)Starting poker client (connecting to $(SERVER_URL))...$(NC)"
	cargo run --release --bin pp_client -- --server $(SERVER_URL)

fmt: ## Format code with rustfmt
	@echo "$(GREEN)Formatting code...$(NC)"
	cargo fmt --all
	@echo "$(GREEN)Formatting complete!$(NC)"

lint: ## Run clippy lints
	@echo "$(GREEN)Running clippy...$(NC)"
	cargo clippy --workspace --all-targets -- -D warnings
	@echo "$(GREEN)No lint issues found!$(NC)"

check: fmt lint test ## Run fmt + lint + test (full check)
	@echo "$(GREEN)All checks passed!$(NC)"

docker-build: ## Build Docker image
	@echo "$(GREEN)Building Docker image...$(NC)"
	docker build -t private-poker:latest .
	@echo "$(GREEN)Docker image built: private-poker:latest$(NC)"

docker-run: ## Run Docker container
	@echo "$(GREEN)Running Docker container...$(NC)"
	docker run -d \
		-p 8080:8080 \
		-e DATABASE_URL="$(DATABASE_URL)" \
		-e RUST_LOG="$(RUST_LOG)" \
		--name private-poker \
		private-poker:latest
	@echo "$(GREEN)Container started: private-poker$(NC)"

# Development workflow targets
.PHONY: dev-setup dev-start dev-stop

dev-setup: db-setup db-migrate ## Complete dev environment setup
	@echo "$(GREEN)Development environment ready!$(NC)"

dev-start: ## Start server and client in background
	@echo "$(GREEN)Starting development environment...$(NC)"
	@DATABASE_URL="$(DATABASE_URL)" RUST_LOG="$(RUST_LOG)" cargo run --release --bin pp_server -- --bind $(BIND_ADDR) &
	@sleep 2
	@cargo run --release --bin pp_client -- --server $(SERVER_URL)

dev-stop: ## Stop all running processes
	@echo "$(YELLOW)Stopping all poker processes...$(NC)"
	-pkill -f pp_server
	-pkill -f pp_client
	@echo "$(GREEN)All processes stopped!$(NC)"
