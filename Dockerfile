# syntax=docker/dockerfile:1.3.1
FROM rust:1.79-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm
WORKDIR /app
COPY --from=builder /app/target/release/edgee .
EXPOSE 80 443
ENTRYPOINT ["./edgee"]
