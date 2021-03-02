FROM rust:1.45-alpine AS build

RUN apk add --no-cache musl-dev

RUN mkdir -p /usr/src/app
WORKDIR /usr/src/app
ADD Cargo.toml Cargo.toml
ADD Cargo.lock Cargo.lock
RUN mkdir src && touch src/lib.rs
RUN cargo build --release

RUN rm -r src
ADD src src
RUN cargo build --release

FROM pmmp/pocketmine-mp:3.17.5
USER root
RUN mkdir /bot
RUN adduser --uid 1001 --disabled-password --home /bot bthint
RUN chown 1001:1001 /bot

USER bthint
WORKDIR /bot
COPY --from=build /usr/src/app/target/release/bthint /usr/bin/bthint

ENV RUST_LOG=info
ENTRYPOINT ["bthint"]
