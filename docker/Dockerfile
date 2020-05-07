FROM rust:1.41.0-slim-buster as build-stage

WORKDIR "/build"

# Install and build crates
COPY Cargo.* /build/
RUN mkdir /build/src && echo "fn main() {}" > /build/src/main.rs
RUN cargo build --release

# Build app
COPY src/ /build/src/
RUN touch src/main.rs && cargo build --release

## -----------------------------------------------------------------------------
## Package
## -----------------------------------------------------------------------------
FROM debian:buster

# Install runtime dependencies

COPY --from=build-stage "/build/target/release/kruonis" "/app/kruonis"

WORKDIR "/app"
ENTRYPOINT ["/app/kruonis"]
