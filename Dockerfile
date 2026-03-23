# Fajar Lang Docker Image
# Usage: docker build -t fajarlang/fj .
#        docker run -it fajarlang/fj repl
#        docker run -v $(pwd):/work fajarlang/fj run /work/program.fj

FROM rust:1.85-bookworm AS builder

WORKDIR /build
COPY . .

RUN cargo build --release --features native \
    && strip target/release/fj

# Runtime image (minimal)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    gcc \
    libc6-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/fj /usr/local/bin/fj
COPY --from=builder /build/stdlib /usr/local/share/fj/stdlib
COPY --from=builder /build/examples /usr/local/share/fj/examples

ENV FJ_STDLIB_PATH=/usr/local/share/fj/stdlib

WORKDIR /work

ENTRYPOINT ["fj"]
CMD ["repl"]

LABEL org.opencontainers.image.title="Fajar Lang"
LABEL org.opencontainers.image.description="Systems programming language for embedded AI + OS integration"
LABEL org.opencontainers.image.url="https://github.com/fajarkraton/fajar-lang"
LABEL org.opencontainers.image.version="4.2.0"
