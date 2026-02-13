#.-.   .-..----..----. .-. .-..-.   .----.
#|  `.'  || {_  | {}  }| |/ / | |   | {_
#| |\ /| || {__ | .-. \| |\ \ | `--.| {__
#`-' ` `-'`----'`-' `-'`-' `-'`----'`----'

FROM lukemathwalker/cargo-chef:0.1.67-rust-1.79.0 AS chef

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS relay
COPY --from=planner /app/recipe.json recipe.json

# Install build dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y libclang-dev pkg-config 
RUN apt-get install -y protobuf-compiler libprotobuf-dev

RUN cargo chef cook --profile maxperf --recipe-path recipe.json

# Copy the entire workspace
COPY . .


RUN cargo build  --profile maxperf --bin relay

# Build runtime image
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install CA certificates
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the built executable from the build stage
COPY --from=relay /app/target/maxperf/relay /app/relay

ENTRYPOINT ["/app/relay"]