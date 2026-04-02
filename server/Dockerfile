# Stage 1: Builder
FROM rust:1.70-slim as builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copia os arquivos Cargo
COPY server/Cargo.toml server/Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Copia o código fonte
COPY server/src ./src
COPY server/templates ./templates
COPY server/payloads ./payloads

# Recompila com o código real
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copia o binário compilado
COPY --from=builder /app/target/release/c2-server /app/

# Copia os arquivos estáticos
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/payloads /app/payloads

# Cria diretório para dados
RUN mkdir -p /app/data

# Expõe a porta
EXPOSE 8443

# Comando para rodar
CMD ["/app/c2-server"]