FROM alpine as build


RUN apk update
RUN apk add musl-dev pkgconfig openssl-dev libc-dev git

# Install rust tools
RUN apk add cargo
# RUN curl -sSf https://sh.rustup.rs > install.sh
# RUN sh install.sh -y
# RUN source "${HOME}/.cargo/env"

WORKDIR /.cargo/registry/index
RUN git clone --bare https://github.com/rust-lang/crates.io-index.git github.com-1285ae84e5963aae
WORKDIR /

ENV OPENSSL_STATIC=true

COPY Cargo.* ./
COPY src/ src/

RUN cargo build --release
RUN strip target/release/easee_status

FROM alpine as run

RUN apk add libgcc
COPY --from=build target/release/easee_status /bin/easee_status

ENV INFLUXDB_ADDR="http://192.168.10.102:8086"
ENV INFLUXDB_DB_NAME="Fibaro"
ENV INFLUXDB_DB_MEASUREMENT="variable_backup"

CMD ["/bin/easee_status"]
