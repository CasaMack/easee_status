FROM rust:alpine as build

RUN apk update
RUN apk add --no-cache musl-dev pkgconfig openssl-dev libc-dev

COPY Cargo.* ./
COPY src/ src/

ENV OPENSSL_STATIC=true
ENV RUSTFLAGS='-C target-feature=-crt-static'
RUN cargo build --release
RUN strip target/release/easee_status

# Works: FROM rust:alpine as run
FROM alpine as run

RUN apk update
RUN apk add libgcc

COPY --from=build target/release/easee_status /bin/easee_status
COPY Rocket.toml Rocket.toml

EXPOSE 8000
ENV ROCKET_ADDRESS=0.0.0.0

CMD ["/bin/easee_status"]
