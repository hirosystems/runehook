FROM rust:bullseye AS build

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates pkg-config libssl-dev libclang-11-dev libunwind-dev libunwind8 curl gnupg
RUN rustup update 1.77.1 && rustup default 1.77.1

RUN mkdir /out
COPY ./Cargo.toml /app/Cargo.toml
COPY ./Cargo.lock /app/Cargo.lock
COPY ./src /app/src
COPY ./migrations /app/migrations

RUN cargo build --features release --release
RUN cp /app/target/release/runehook /out

FROM debian:bullseye-slim

COPY --from=build /out/runehook /bin/runehook

WORKDIR /workspace

ENTRYPOINT ["runehook"]
