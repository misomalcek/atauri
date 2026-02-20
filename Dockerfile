# Tauri Code Web — Runtime image
# Uses pre-built ARM64 binary + Svelte dist/
# Build the binary first:
#   cargo build --release -p tauri-code-web --target aarch64-unknown-linux-gnu
# Then build this image:
#   docker build -t tauri-code .

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pre-built ARM64 binary
COPY target/aarch64-unknown-linux-gnu/release/tauri-code-web .

# Svelte frontend build
COPY dist/ ./dist/

EXPOSE 3000

ENV PORT=3000

CMD ["./tauri-code-web"]
