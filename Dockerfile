# Fajar Lang Official Docker Image
# Build:  docker build -t fajarlang/fj .
# Run:    docker run -it fajarlang/fj repl
# File:   docker run -v $(pwd):/work fajarlang/fj run /work/program.fj

FROM rust:1.87-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY stdlib/ stdlib/
RUN cargo build --release && strip target/release/fj

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/fj /usr/local/bin/fj
COPY --from=builder /build/stdlib /usr/local/share/fj/stdlib
ENV FJ_STDLIB_PATH=/usr/local/share/fj/stdlib
WORKDIR /work
ENTRYPOINT ["fj"]
CMD ["repl"]

LABEL org.opencontainers.image.title="Fajar Lang"
LABEL org.opencontainers.image.description="Systems programming language for embedded AI + OS integration"
LABEL org.opencontainers.image.url="https://github.com/fajarkraton/fajar-lang"
