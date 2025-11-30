FROM rust:1.84.1 AS builder
WORKDIR /usr/src
COPY ../backend ./app/backend
COPY ../web ./app/web
WORKDIR /usr/src/app/backend
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app/backend
RUN mkdir /data
RUN mkdir /source
RUN mkdir /destination
COPY --from=builder /usr/src/app/backend/target/release/RustyHashBackUp ./
COPY --from=builder /usr/src/app/backend/Rocket.toml ./
COPY --from=builder /usr/src/app/web ../web
ENV RUST_BACKTRACE=full
ENV RUSTYHASHBACKUP_CONFIG=/data/config.json
ENV LOG_LEVEL=info
CMD ["./RustyHashBackUp"]
