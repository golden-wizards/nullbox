# Nullbox API: Rust build, then a small runtime image.
FROM docker.io/library/rust:1-slim-bookworm AS build
WORKDIR /src
# rust-toolchain.toml pins the exact toolchain; rustup installs it on first use.
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain || rustup toolchain install
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p nullbox-api && cp target/release/nullbox-api /nullbox-api

FROM docker.io/library/debian:bookworm-slim
RUN useradd --system --uid 10001 --home /data nullbox && mkdir -p /data && chown nullbox /data
COPY --from=build /nullbox-api /usr/local/bin/nullbox-api
USER nullbox
ENV BIND_ADDR=0.0.0.0:3000 DATABASE_URL=sqlite:///data/nullbox.db RUST_LOG=info
EXPOSE 3000
CMD ["nullbox-api"]
