FROM rust:1.84.1 AS builder
WORKDIR /usr/src/app
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /usr/src/app
RUN mkdir /data
RUN mkdir /source
RUN mkdir /destination
COPY --from=builder /usr/src/app/target/release/RustyHashBackUp ./
ENV RUST_BACKTRACE=full
ENV RUSTYHASHBACKUP_CONFIG=/data/config.json
CMD ["./RustyHashBackUp"]
