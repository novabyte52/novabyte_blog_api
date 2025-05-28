# ---- Build Stage ----
FROM rust:1.83.0 as builder

WORKDIR /app

# Copy manifests first to cache dependencies
COPY Cargo.toml Cargo.lock ./

RUN cargo fetch

# Copy the rest of your source
COPY .env .env
COPY . .

# Build for release
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

WORKDIR /app

# Install needed SSL libraries
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the release binary from the builder
COPY --from=builder /app/target/release/nb_blog_api /app/nb_blog_api

# Copy any static assets/config if needed (optional)
# COPY ./migrations ./migrations

# Set env vars (override with docker-compose or -e)
ENV RUST_LOG=nb_blog_api=info,nb_lib=info

# Expose API port
EXPOSE 52001

# Entrypoint
CMD ["./nb_blog_api"]
