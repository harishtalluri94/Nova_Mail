#!/bin/bash
set -euo pipefail

# Build all Docker images for Nova Mail services

REGISTRY=${REGISTRY:-nova-mail}
TAG=${TAG:-latest}

echo "Building Nova Mail Docker images..."
echo "Registry: $REGISTRY"
echo "Tag: $TAG"
echo ""

SERVICES=(
    "admin-api"
    "lmtp-gateway"
    "jmap-service"
    "delivery-worker"
    "indexer"
    "search-gateway"
    "previewer"
    "ai-assistant"
    "link-service"
    "antiabuse"
)

build_service() {
    local service=$1
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Building: $service"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    docker build \
        -t "$REGISTRY/$service:$TAG" \
        -f "services/$service/Dockerfile" \
        . || {
            echo "❌ Failed to build $service"
            return 1
        }

    echo "✓ Built $REGISTRY/$service:$TAG"
    echo ""
}

# Build all services
for service in "${SERVICES[@]}"; do
    build_service "$service"
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✓ All services built successfully!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Images built:"
for service in "${SERVICES[@]}"; do
    echo "  - $REGISTRY/$service:$TAG"
done
