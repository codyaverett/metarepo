# Metarepo Makefile
# Build and install the production binary locally

# Variables
BINARY_NAME = meta
CARGO = cargo
INSTALL_PATH = $(HOME)/.local/bin
BUILD_MODE = release

# Color output
RED = \033[0;31m
GREEN = \033[0;32m
YELLOW = \033[1;33m
BLUE = \033[0;34m
MAGENTA = \033[0;35m
CYAN = \033[0;36m
WHITE = \033[1;37m
NC = \033[0m # No Color

# Default target
.PHONY: help
help:
	@echo "$(CYAN)Metarepo Build System$(NC)"
	@echo "$(WHITE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(YELLOW)Production:$(NC)"
	@echo "  $(GREEN)make install$(NC)     - Build and install production binary to ~/.local/bin"
	@echo "  $(GREEN)make build$(NC)       - Build production binary (optimized)"
	@echo "  $(GREEN)make uninstall$(NC)   - Remove installed binary"
	@echo ""
	@echo "$(YELLOW)Development:$(NC)"
	@echo "  $(GREEN)make dev$(NC)         - Build development binary (debug mode)"
	@echo "  $(GREEN)make run$(NC)         - Run development binary"
	@echo "  $(GREEN)make test$(NC)        - Run all tests"
	@echo "  $(GREEN)make check$(NC)       - Run cargo check"
	@echo "  $(GREEN)make clean$(NC)       - Clean build artifacts"
	@echo ""
	@echo "$(YELLOW)Quality:$(NC)"
	@echo "  $(GREEN)make fmt$(NC)         - Format code"
	@echo "  $(GREEN)make lint$(NC)        - Run clippy linter"
	@echo "  $(GREEN)make audit$(NC)       - Check for security vulnerabilities"
	@echo ""
	@echo "$(YELLOW)Install Paths:$(NC)"
	@echo "  Binary: $(CYAN)$(INSTALL_PATH)/$(BINARY_NAME)$(NC)"
	@echo ""

# Build production binary
.PHONY: build
build:
	@echo "$(CYAN)🔨 Building production binary...$(NC)"
	@$(CARGO) build --release --bin $(BINARY_NAME)
	@echo "$(GREEN)Build complete!$(NC)"
	@echo "$(WHITE)Binary location: target/release/$(BINARY_NAME)$(NC)"

# Install production binary to ~/.local/bin
.PHONY: install
install: build
	@echo "$(CYAN)📦 Installing $(BINARY_NAME) to $(INSTALL_PATH)...$(NC)"
	@mkdir -p $(INSTALL_PATH)
	@cp target/release/$(BINARY_NAME) $(INSTALL_PATH)/
	@chmod +x $(INSTALL_PATH)/$(BINARY_NAME)
	@echo "$(GREEN)Installation complete!$(NC)"
	@echo "$(CYAN)You can now run:$(NC) $(GREEN)$(BINARY_NAME) --help$(NC)"

# Uninstall binary
.PHONY: uninstall
uninstall:
	@echo "$(RED)🗑️  Uninstalling $(BINARY_NAME)...$(NC)"
	@rm -f $(INSTALL_PATH)/$(BINARY_NAME)
	@echo "$(GREEN)Uninstalled successfully$(NC)"

# Development build
.PHONY: dev
dev:
	@echo "$(CYAN)🔧 Building development binary...$(NC)"
	@$(CARGO) build --bin $(BINARY_NAME)
	@echo "$(GREEN)✅ Development build complete!$(NC)"

# Run development binary
.PHONY: run
run:
	@$(CARGO) run --bin $(BINARY_NAME) -- $(ARGS)

# Run tests
.PHONY: test
test:
	@echo "$(CYAN)🧪 Running tests...$(NC)"
	@$(CARGO) test --all

# Check code
.PHONY: check
check:
	@echo "$(CYAN)🔍 Checking code...$(NC)"
	@$(CARGO) check --all

# Clean build artifacts
.PHONY: clean
clean:
	@echo "$(YELLOW)🧹 Cleaning build artifacts...$(NC)"
	@$(CARGO) clean
	@echo "$(GREEN)✅ Clean complete!$(NC)"

# Format code
.PHONY: fmt
fmt:
	@echo "$(CYAN)✨ Formatting code...$(NC)"
	@$(CARGO) fmt --all
	@echo "$(GREEN)✅ Formatting complete!$(NC)"

# Run clippy linter
.PHONY: lint
lint:
	@echo "$(CYAN)📋 Running clippy...$(NC)"
	@$(CARGO) clippy --all -- -D warnings

# Security audit
.PHONY: audit
audit:
	@echo "$(CYAN)🔒 Running security audit...$(NC)"
	@$(CARGO) audit

# Quick install (skip checks, just build and install)
.PHONY: quick-install
quick-install:
	@echo "$(CYAN)⚡ Quick install...$(NC)"
	@$(CARGO) build --release --bin $(BINARY_NAME) 2>/dev/null || true
	@mkdir -p $(INSTALL_PATH)
	@cp target/release/$(BINARY_NAME) $(INSTALL_PATH)/ 2>/dev/null || (echo "$(RED)❌ Build failed$(NC)" && exit 1)
	@chmod +x $(INSTALL_PATH)/$(BINARY_NAME)
	@echo "$(GREEN)✅ Installed to $(INSTALL_PATH)/$(BINARY_NAME)$(NC)"

# Install with custom path
.PHONY: install-to
install-to: build
	@if [ -z "$(PREFIX)" ]; then \
		echo "$(RED)❌ Please specify PREFIX, e.g., make install-to PREFIX=/usr/local$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)📦 Installing to $(PREFIX)/bin...$(NC)"
	@mkdir -p $(PREFIX)/bin
	@cp target/release/$(BINARY_NAME) $(PREFIX)/bin/
	@chmod +x $(PREFIX)/bin/$(BINARY_NAME)
	@echo "$(GREEN)✅ Installed to $(PREFIX)/bin/$(BINARY_NAME)$(NC)"

# Version info
.PHONY: version
version:
	@echo "$(CYAN)Metarepo Version Information:$(NC)"
	@grep "^version" metarepo/Cargo.toml | head -1 | cut -d'"' -f2

# Watch for changes and rebuild
.PHONY: watch
watch:
	@echo "$(CYAN)👁️  Watching for changes...$(NC)"
	@cargo watch -x "build --bin $(BINARY_NAME)"

.PHONY: all
all: fmt check test build
