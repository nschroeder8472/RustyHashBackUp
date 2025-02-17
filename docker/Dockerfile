FROM rust:1.84.1 AS builder
WORKDIR /usr/src/app
COPY ../Cargo.toml ../Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
COPY ../src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/LightWeightHashBackup./
CMD ["./LightWeightHashBackup"]
