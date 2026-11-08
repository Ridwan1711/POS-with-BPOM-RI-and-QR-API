# syntax=docker/dockerfile:1
# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release --locked --bin pos-backend --bin seed

# ---- runtime ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && useradd --system --uid 10001 --no-create-home pos
WORKDIR /app
COPY --from=build /src/target/release/pos-backend /usr/local/bin/pos-backend
COPY --from=build /src/target/release/seed /usr/local/bin/seed
USER pos
ENV BIND_ADDR=0.0.0.0:8080 LOG_JSON=true RUST_LOG=info,sqlx=warn
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -fsS http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["/usr/local/bin/pos-backend"]
