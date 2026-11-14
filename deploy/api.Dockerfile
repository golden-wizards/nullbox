# Nullbox API: Rust build, then a small runtime image.
#
# Layers are ordered for the CI layer cache (deploy.yml, cache-to type=gha):
# - only Rust inputs are copied, so a web-only commit reuses the whole build;
# - dependencies are compiled from a cargo-chef recipe, so a code-only commit
#   recompiles just the workspace crates.
FROM docker.io/library/rust:1-slim-bookworm AS chef
WORKDIR /src
# rust-toolchain.toml pins the exact toolchain; rustup installs it on first use.
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain || rustup toolchain install
RUN cargo install cargo-chef --locked --version 0.1.78

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p nullbox-api --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations
COPY .sqlx .sqlx
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked -p nullbox-api && cp target/release/nullbox-api /nullbox-api

FROM docker.io/library/debian:bookworm-slim
RUN useradd --system --uid 10001 --home /data nullbox && mkdir -p /data && chown nullbox /data
COPY --from=build /nullbox-api /usr/local/bin/nullbox-api
USER nullbox
ENV BIND_ADDR=0.0.0.0:3000 DATABASE_URL=sqlite:///data/nullbox.db RUST_LOG=info
EXPOSE 3000
CMD ["nullbox-api"]
