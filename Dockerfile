FROM rust:1.92-bullseye AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev libssl-dev openssl

WORKDIR /builder
COPY . .

RUN cargo build --target x86_64-unknown-linux-musl --release --bin nsp-k8s-operator
RUN strip target/x86_64-unknown-linux-musl/release/nsp-k8s-operator

FROM scratch
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /builder/target/x86_64-unknown-linux-musl/release/nsp-k8s-operator /
EXPOSE 8080
ENTRYPOINT ["/nsp-k8s-operator"]
