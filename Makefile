.PHONY: help dev-up dev-down build test lint clean deploy-local docker-build

# Default target
help:
	@echo "Nova Mail - Development Commands"
	@echo ""
	@echo "Development:"
	@echo "  make dev-up           - Start local development environment (kind + MinIO + PostgreSQL + Redis)"
	@echo "  make dev-down         - Stop local development environment"
	@echo "  make demo             - Run full demo with sample data"
	@echo ""
	@echo "Building:"
	@echo "  make build            - Build all Rust services"
	@echo "  make build-web        - Build Next.js webmail"
	@echo "  make docker-build     - Build all Docker images"
	@echo ""
	@echo "Testing:"
	@echo "  make test             - Run all tests"
	@echo "  make test-unit        - Run unit tests only"
	@echo "  make test-integration - Run integration tests"
	@echo "  make test-e2e         - Run end-to-end tests"
	@echo ""
	@echo "Quality:"
	@echo "  make lint             - Run linters (clippy, eslint)"
	@echo "  make fmt              - Format code (rustfmt, prettier)"
	@echo "  make check            - Check code without building"
	@echo ""
	@echo "Deployment:"
	@echo "  make deploy-local     - Deploy to local kind cluster"
	@echo "  make deploy-stage     - Deploy to staging (requires credentials)"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean            - Clean build artifacts"
	@echo "  make reset            - Reset local environment (destructive)"

# Development environment
dev-up:
	@echo "Starting local development environment..."
	@kind create cluster --config ops/scripts/kind-config.yaml --name nova-mail || echo "Cluster already exists"
	@docker-compose -f docker-compose.dev.yml up -d
	@echo "Waiting for services to be ready..."
	@sleep 10
	@echo "✓ Development environment ready!"
	@echo "  PostgreSQL: localhost:5432"
	@echo "  Redis: localhost:6379"
	@echo "  MinIO: localhost:9000 (console: localhost:9001)"

dev-down:
	@echo "Stopping local development environment..."
	@docker-compose -f docker-compose.dev.yml down
	@kind delete cluster --name nova-mail || true

demo: dev-up
	@echo "Running demo setup..."
	@./ops/scripts/seed-demo-data.sh
	@echo "✓ Demo environment ready!"
	@echo "  Webmail: http://localhost:3000"
	@echo "  Admin: http://localhost:8080"
	@echo "  Test user: demo@nova.local / password: demo123"

# Building
build:
	@echo "Building Rust services..."
	@cargo build --release

build-web:
	@echo "Building webmail..."
	@cd web/webmail && npm install && npm run build

build-dev:
	@echo "Building Rust services (dev mode)..."
	@cargo build

docker-build:
	@echo "Building Docker images..."
	@./ops/scripts/docker-build-all.sh

# Testing
test:
	@echo "Running all tests..."
	@cargo test --workspace
	@cd web/webmail && npm test

test-unit:
	@echo "Running unit tests..."
	@cargo test --workspace --lib

test-integration:
	@echo "Running integration tests..."
	@cargo test --workspace --test '*'

test-e2e:
	@echo "Running E2E tests..."
	@cd tests/e2e && npm test

# Quality
lint:
	@echo "Running Rust linter..."
	@cargo clippy --workspace --all-targets -- -D warnings
	@echo "Running TypeScript linter..."
	@cd web/webmail && npm run lint

fmt:
	@echo "Formatting Rust code..."
	@cargo fmt --all
	@echo "Formatting TypeScript code..."
	@cd web/webmail && npm run format

check:
	@echo "Checking Rust code..."
	@cargo check --workspace --all-targets

# Deployment
deploy-local: build docker-build
	@echo "Deploying to local kind cluster..."
	@kind load docker-image nova-mail/lmtp-gateway:latest --name nova-mail
	@kind load docker-image nova-mail/jmap-service:latest --name nova-mail
	@kind load docker-image nova-mail/delivery-worker:latest --name nova-mail
	@kind load docker-image nova-mail/indexer:latest --name nova-mail
	@kind load docker-image nova-mail/search-gateway:latest --name nova-mail
	@kind load docker-image nova-mail/previewer:latest --name nova-mail
	@kind load docker-image nova-mail/link-service:latest --name nova-mail
	@kind load docker-image nova-mail/admin-api:latest --name nova-mail
	@kind load docker-image nova-mail/antiabuse:latest --name nova-mail
	@kubectl apply -k deploy/k8s/overlays/local
	@echo "✓ Deployed to local cluster"

# Utilities
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@cd web/webmail && rm -rf .next node_modules
	@rm -rf target/

reset: dev-down clean
	@echo "Resetting local environment..."
	@docker volume prune -f
	@kind delete cluster --name nova-mail || true
	@echo "✓ Environment reset complete"

# Database migrations
migrate:
	@echo "Running database migrations..."
	@cd services/admin-api && cargo sqlx migrate run

migrate-revert:
	@echo "Reverting last migration..."
	@cd services/admin-api && cargo sqlx migrate revert

# DKIM key generation
gen-dkim:
	@echo "Generating DKIM key pair..."
	@./ops/scripts/gen-dkim-key.sh

# Performance testing
perf-test:
	@echo "Running performance tests..."
	@cd tests/perf && k6 run jmap-load.js
