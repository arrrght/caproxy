#FROM rust:latest as cargo-build
FROM clux/muslrust as cargo-build
#ENV PKG_CONFIG_ALLOW_CROSS=1
RUN apt-get update
RUN apt-get install libssl-dev
#RUN apt-get install libssl-dev musl-tools -y
#rust:latestRUN rustup target add x86_64-unknown-linux-musl


WORKDIR /usr/src/caproxy
COPY . .
#RUN cargo build --release --target=x86_64-unknown-linux-musl --features=vendored
RUN cargo build --release

FROM alpine:latest
RUN apk add openssl
COPY --from=cargo-build /usr/src/caproxy/target/x86_64-unknown-linux-musl/release/caproxy /usr/local/bin/caproxy

CMD ["/usr/local/bin/caproxy"]
