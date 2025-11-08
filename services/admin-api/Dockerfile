FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/admin-api ./services/admin-api

# Build the application
RUN cargo build --release --bin admin-api

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/admin-api /usr/local/bin/admin-api

# Copy migrations
COPY services/admin-api/migrations /app/migrations

# Create non-root user
RUN useradd -m -u 1000 nova && chown -R nova:nova /app
USER nova

EXPOSE 8080

CMD ["admin-api"]
