# ---- Build Stage ----
FROM rust:1.89-bookworm as builder

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

# Common runtime libs for TLS + time + DNS metadata
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates tzdata libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nb_blog_api /app/nb_blog_api

ENV RUST_LOG=nb_blog_api=info,nb_lib=info

EXPOSE 52001

CMD ["./nb_blog_api"]
