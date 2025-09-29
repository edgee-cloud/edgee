# syntax=docker/dockerfile:1.3.1
FROM rust:1.90.0-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm
RUN apt-get update && apt-get install -y libssl-dev
WORKDIR /app
COPY --from=builder /app/target/release/edgee /app/edgee
ENTRYPOINT ["/app/edgee"]
