# Zenth - Decentralized E2E Encrypted Messaging

> Secure, private, decentralized messaging with zero-knowledge architecture

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/react-18.0+-blue.svg)](https://reactjs.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.x-green.svg)](https://tauri.app/)

## Quick Start

```bash
# Install dependencies
bun install

# Run development server
cargo tauri dev

# Build for production
cargo tauri build
```

## Features

✅ **Zero-knowledge architecture** - Server cannot read messages or usernames
✅ **Post-quantum cryptography** - Dilithium5, Kyber1024
✅ **Double encryption** - SQLCipher + AES-256-GCM
✅ **Perfect forward secrecy** - Double Ratchet protocol
✅ **Multi-darknet routing** - Tor, I2P, Lokinet
✅ **Zero-logs policy** - No tracking, maximum privacy

## Documentation

📚 **[Complete documentation in docs/](docs/README.md)**

- [Architecture](docs/ARCHITECTURE.md) - Security model and design
- [Development Guide](docs/DEVELOPMENT.md) - Setup and workflows
- [Backend Documentation](docs/backend/) - Rust, database, API
- [Features](docs/features/) - Login, settings, messaging

## Technology Stack

- **Frontend**: React 18 + TypeScript + Vite + TailwindCSS
- **Backend**: Rust + Tauri 2.x
- **Database**: SQLite + SQLCipher
- **Crypto**: Post-quantum (Dilithium5, Kyber1024, Argon2id)

## Project Structure

```
zenth_front/
├── docs/              # Documentation
├── src/               # Frontend (React + TypeScript)
├── src-tauri/         # Backend (Rust + Tauri)
└── CLAUDE.md          # AI assistant instructions
```

## Contributing

See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for development guidelines.

## License

[Your License]
