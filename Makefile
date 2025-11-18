# Metarepo Makefile
# Build, install, and publish the metarepo packages

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
	@echo "  $(GREEN)make install-hooks$(NC) - Install git pre-commit hooks"
	@echo ""
	@echo "$(YELLOW)Publishing:$(NC)"
	@echo "  $(GREEN)make publish-dry$(NC) - Dry run all package publishing"
	@echo "  $(GREEN)make publish-all$(NC) - Publish all packages to crates.io"
	@echo "  $(GREEN)make publish-core$(NC) - Publish metarepo-core only"
	@echo ""
	@echo "$(YELLOW)Version Management:$(NC)"
	@echo "  $(GREEN)make check-versions$(NC) - Check version consistency"
	@echo "  $(GREEN)make bump-version V=X.Y.Z$(NC) - Update all versions"
	@echo ""
	@echo "$(YELLOW)Issue Management:$(NC)"
	@echo "  $(GREEN)make issue-bug$(NC)     - Create bug report (interactive)"
	@echo "  $(GREEN)make issue-feature$(NC) - Create feature request (interactive)"
	@echo "  $(GREEN)make issue-idea$(NC)    - Quick idea capture"
	@echo "  $(GREEN)make list-issues$(NC)   - List recent issues"
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
	@grep "^version" meta/Cargo.toml | head -1 | cut -d'"' -f2

# Watch for changes and rebuild
.PHONY: watch
watch:
	@echo "$(CYAN)👁️  Watching for changes...$(NC)"
	@cargo watch -x "build --bin $(BINARY_NAME)"

# Install git hooks
.PHONY: install-hooks
install-hooks:
	@echo "$(CYAN)📎 Installing git hooks...$(NC)"
	@./scripts/install-hooks.sh

.PHONY: all
all: fmt check test build

# ============================================================================
# Issue Management Commands
# ============================================================================

# Create a bug report using gh CLI
.PHONY: issue-bug
issue-bug:
	@if ! command -v gh &> /dev/null; then \
		echo "$(RED)❌ GitHub CLI (gh) is not installed.$(NC)"; \
		echo "$(CYAN)Install from: https://cli.github.com/$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)🐛 Creating bug report...$(NC)"
	@.github/scripts/new-bug.sh

# Create a feature request using gh CLI
.PHONY: issue-feature
issue-feature:
	@if ! command -v gh &> /dev/null; then \
		echo "$(RED)❌ GitHub CLI (gh) is not installed.$(NC)"; \
		echo "$(CYAN)Install from: https://cli.github.com/$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)✨ Creating feature request...$(NC)"
	@.github/scripts/new-feature.sh

# Quick idea capture
.PHONY: issue-idea
issue-idea:
	@if ! command -v gh &> /dev/null; then \
		echo "$(RED)❌ GitHub CLI (gh) is not installed.$(NC)"; \
		echo "$(CYAN)Install from: https://cli.github.com/$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)💡 Quick idea capture$(NC)"
	@read -p "Idea: " idea && .github/scripts/new-idea.sh "$$idea"

# List recent issues
.PHONY: list-issues
list-issues:
	@if ! command -v gh &> /dev/null; then \
		echo "$(RED)❌ GitHub CLI (gh) is not installed.$(NC)"; \
		echo "$(CYAN)Install from: https://cli.github.com/$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)📋 Recent issues:$(NC)"
	@gh issue list --limit 10

# ============================================================================
# Publishing Commands
# ============================================================================

# Check version consistency across all packages
.PHONY: check-versions
check-versions:
	@echo "$(CYAN)🔍 Checking version consistency...$(NC)"
	@VERSION=$$(grep "^version" meta-core/Cargo.toml | head -1 | cut -d'"' -f2); \
	echo "Core version: $$VERSION"; \
	META_VERSION=$$(grep "^version" meta/Cargo.toml | head -1 | cut -d'"' -f2); \
	if [ "$$META_VERSION" != "$$VERSION" ]; then \
		echo "$(RED)❌ Version mismatch in meta: $$META_VERSION != $$VERSION$(NC)"; \
		exit 1; \
	fi; \
	echo "$(GREEN)✅ Both packages have version $$VERSION$(NC)"

# Bump version for all packages
.PHONY: bump-version
bump-version:
	@if [ -z "$(V)" ]; then \
		echo "$(RED)❌ Please specify version: make bump-version V=X.Y.Z$(NC)"; \
		exit 1; \
	fi
	@echo "$(CYAN)📝 Updating packages to version $(V)...$(NC)"
	@sed -i '' 's/^version = ".*"/version = "$(V)"/' meta-core/Cargo.toml
	@sed -i '' 's/^version = ".*"/version = "$(V)"/' meta/Cargo.toml
	@sed -i '' 's/metarepo-core = { version = "[^"]*"/metarepo-core = { version = "$(V)"/' meta/Cargo.toml
	@echo "$(GREEN)✅ Both packages updated to version $(V)$(NC)"

# Publishing commands (only 2 packages now!)
.PHONY: publish-core
publish-core:
	@echo "$(CYAN)📦 Publishing metarepo-core...$(NC)"
	@cd meta-core && cargo publish 2>/dev/null && echo "$(GREEN)✅ Published metarepo-core$(NC)" || echo "$(YELLOW)⚠️  metarepo-core already published or error occurred$(NC)"

.PHONY: publish-main
publish-main:
	@echo "$(CYAN)📦 Publishing metarepo (main package with built-in plugins)...$(NC)"
	@cd meta && cargo publish 2>/dev/null && echo "$(GREEN)✅ Published metarepo$(NC)" || echo "$(YELLOW)⚠️  metarepo already published or error occurred$(NC)"

# Smart publish - only publishes packages that aren't already published
.PHONY: publish
publish: check-versions
	@echo "$(CYAN)🔍 Checking which packages need publishing...$(NC)"
	@CORE_VERSION=$$(grep '^version' meta-core/Cargo.toml | head -1 | cut -d'"' -f2); \
	META_VERSION=$$(grep '^version' meta/Cargo.toml | head -1 | cut -d'"' -f2); \
	CORE_PUBLISHED=$$(cargo search metarepo-core --limit 1 2>/dev/null | grep "^metarepo-core = " | cut -d'"' -f2); \
	META_PUBLISHED=$$(cargo search metarepo --limit 1 2>/dev/null | grep "^metarepo = " | cut -d'"' -f2); \
	echo "Local versions: metarepo-core=$$CORE_VERSION, metarepo=$$META_VERSION"; \
	echo "Published versions: metarepo-core=$$CORE_PUBLISHED, metarepo=$$META_PUBLISHED"; \
	if [ "$$CORE_VERSION" != "$$CORE_PUBLISHED" ]; then \
		echo "$(CYAN)Publishing metarepo-core $$CORE_VERSION...$(NC)"; \
		cd meta-core && cargo publish && echo "$(GREEN)✅ Published metarepo-core $$CORE_VERSION$(NC)" || echo "$(RED)❌ Failed to publish metarepo-core$(NC)"; \
		sleep 10; \
	else \
		echo "$(YELLOW)metarepo-core $$CORE_VERSION already published$(NC)"; \
	fi; \
	if [ "$$META_VERSION" != "$$META_PUBLISHED" ]; then \
		echo "$(CYAN)Publishing metarepo $$META_VERSION...$(NC)"; \
		cd meta && cargo publish && echo "$(GREEN)✅ Published metarepo $$META_VERSION$(NC)" || echo "$(RED)❌ Failed to publish metarepo$(NC)"; \
	else \
		echo "$(YELLOW)metarepo $$META_VERSION already published$(NC)"; \
	fi

# Dry run for all packages
.PHONY: publish-dry
publish-dry:
	@echo "$(CYAN)🧪 Dry run: Testing package publishing...$(NC)"
	@echo "$(YELLOW)Testing metarepo-core...$(NC)"
	@cd meta-core && cargo publish --dry-run
	@echo "$(YELLOW)Testing metarepo (main with built-in plugins)...$(NC)"
	@cd meta && cargo publish --dry-run
	@echo "$(GREEN)✅ Both packages passed dry run$(NC)"

# Publish all packages in dependency order (with error handling)
.PHONY: publish-all
publish-all: check-versions
	@echo "$(CYAN)🚀 Publishing packages to crates.io...$(NC)"
	@echo "$(YELLOW)⚠️  This will publish the following packages:$(NC)"
	@echo "  1. metarepo-core (plugin API)"
	@echo "  2. metarepo (main CLI with built-in plugins)"
	@echo ""
	@printf "$(YELLOW)Continue? (y/N): $(NC)" && read confirm && [ "$$confirm" = "y" ] || exit 1
	@echo "$(CYAN)Starting publish sequence...$(NC)"
	@echo "$(BLUE)[1/2] Publishing metarepo-core...$(NC)"
	@if cd meta-core && cargo publish 2>/dev/null; then \
		echo "$(GREEN)✅ Published metarepo-core$(NC)"; \
		sleep 10; \
	else \
		echo "$(YELLOW)⚠️  metarepo-core v$$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2) already published or error occurred$(NC)"; \
		echo "$(CYAN)Continuing with metarepo...$(NC)"; \
	fi
	@echo "$(BLUE)[2/2] Publishing metarepo (main package with built-in plugins)...$(NC)"
	@if cd meta && cargo publish 2>/dev/null; then \
		echo "$(GREEN)✅ Published metarepo$(NC)"; \
		echo ""; \
		echo "$(GREEN)🎉 Successfully published metarepo!$(NC)"; \
	else \
		echo "$(YELLOW)⚠️  metarepo v$$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2) already published or error occurred$(NC)"; \
		echo "$(YELLOW)Check if both packages are already at the desired version on crates.io$(NC)"; \
	fi


# Publish with pre-checks
.PHONY: publish-safe
publish-safe: fmt check test publish-dry publish-all
