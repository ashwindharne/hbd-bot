# ---------- build layer ----------
FROM rust:1.88-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# efficient cache with cargo-chef (optional but recommended)
RUN cargo install cargo-chef
WORKDIR /app
COPY common/Cargo.toml common/Cargo.toml
COPY sms-sweeper/Cargo.toml sms-sweeper/Cargo.toml
COPY web/Cargo.toml web/Cargo.toml
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook    --release --recipe-path recipe.json

# now copy source and build real binaries
COPY . .
RUN cargo build --release --bin web --bin sms-sweeper

# ---------- runtime layer ----------
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
# copy executables & assets
COPY --from=builder /app/target/release/web     /app/web
COPY --from=builder /app/target/release/sms-sweeper /app/sms-sweeper
COPY web/static/      /app/static/
COPY migrations/  /app/migrations/

# tini for signal handling
ENTRYPOINT ["/app/web"]
