# SETUP ENVIRONMENT

> Panduan Setup Google Antigravity + Claude Opus 4.6 untuk Fajar Lang Development

**Version 1.0 | Maret 2026**

---

## Daftar Isi

1. [Overview](#1-overview)
2. [Prerequisites](#2-prerequisites)
3. [Google Antigravity Setup](#3-google-antigravity-setup)
4. [Project Initialization](#4-project-initialization)
5. [Daily Development Workflow](#5-daily-development-workflow)
6. [Tips Google Antigravity](#6-tips-google-antigravity)
7. [Multi-Agent Setup di Antigravity](#7-multi-agent-setup-di-antigravity)
8. [Troubleshooting Setup](#8-troubleshooting-setup)

---

## 1. Overview

Dokumen ini menjelaskan cara setup lingkungan pengembangan Fajar Lang menggunakan Google Antigravity sebagai cloud IDE dan Claude Opus 4.6 sebagai AI development assistant melalui Claude Code.

> **Stack Pengembangan:** Google Antigravity (Cloud IDE) + Claude Code (AI Agent) + Rust Toolchain (Compiler) + Claude Opus 4.6 (Model)

---

## 2. Prerequisites

### 2.1 Akun & Akses

- Google Account dengan akses ke Google Antigravity (cloud IDE)
- Anthropic API key atau Claude Pro/Team subscription
- GitHub account untuk version control

### 2.2 System Requirements

| Komponen | Minimum | Rekomendasi |
|----------|---------|-------------|
| RAM | 4 GB | 8 GB+ |
| Storage | 2 GB free | 10 GB+ free |
| Rust Version | 1.75+ | Latest stable |
| Node.js | 18+ | 20 LTS |
| Git | 2.30+ | Latest |
| Claude Code | Latest | Latest |
| Internet | Stable connection | Broadband |

---

## 3. Google Antigravity Setup

### 3.1 Membuat Workspace Baru

Google Antigravity menyediakan cloud-based development environment yang langsung terhubung dengan Google Cloud infrastructure. Berikut langkah-langkah setup:

1. Buka Google Antigravity di browser (antigravity.google.com)
2. Klik 'New Workspace' dan pilih template 'Blank' atau 'Rust'
3. Beri nama workspace: `fajar-lang`
4. Pilih machine type: Standard (4 vCPU, 8 GB RAM) atau lebih
5. Tunggu workspace selesai provisioning (1-2 menit)

### 3.2 Install Rust Toolchain

Setelah workspace aktif, buka terminal dan jalankan:

```bash
# Install Rust (jika belum ada)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Update ke stable terbaru
rustup update stable

# Install komponen tambahan
rustup component add clippy rustfmt

# Verifikasi
cargo --version    # harus 1.75+
rustc --version
clippy-driver --version
```

### 3.3 Install Claude Code

Claude Code adalah CLI tool untuk AI-assisted development:

```bash
# Install Node.js (jika belum ada di Antigravity)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs

# Install Claude Code globally
npm install -g @anthropic-ai/claude-code

# Verifikasi
claude --version
```

### 3.4 Konfigurasi Claude Code

Setup Claude Code untuk menggunakan Opus 4.6:

```bash
# Set API key (atau login via OAuth)
export ANTHROPIC_API_KEY='sk-ant-...'

# ATAU gunakan login interaktif
claude login

# Set model ke Opus 4.6 (WAJIB)
claude config set model claude-opus-4-6

# Set effort level default
claude config set effort high

# Verifikasi konfigurasi
claude config list
```

> ⚠️ **PENTING:** Selalu gunakan Claude Opus 4.6 untuk Fajar Lang. Model lain (Sonnet/Haiku) TIDAK memiliki kapasitas arsitektur yang cukup untuk compiler development.

### 3.5 Konfigurasi Git di Antigravity

```bash
git config --global user.name 'Fajar'
git config --global user.email 'fajar@primecore.id'

# Setup SSH key untuk GitHub
ssh-keygen -t ed25519 -C 'fajar@primecore.id'
cat ~/.ssh/id_ed25519.pub
# Copy output ke GitHub Settings > SSH Keys
```

---

## 4. Project Initialization

Ikuti langkah-langkah INIT.md untuk scaffold project. Ringkasan:

```bash
# 1. Create Rust project
cargo new fajar-lang
cd fajar-lang

# 2. Initialize git
git init
git add .
git commit -m "chore: initial cargo project"
git checkout -b phase-1

# 3. Copy documentation
mkdir -p docs
# Copy semua .md files dari fajar-lang-docs/

# 4. Create directory structure
mkdir -p src/{lexer,parser,analyzer,interpreter}
mkdir -p src/runtime/{os,ml}
mkdir -p src/stdlib
mkdir -p tests examples benches

# 5. Verify build
cargo build
cargo test
```

---

## 5. Daily Development Workflow

### 5.1 Memulai Sesi

```bash
# 1. Buka Google Antigravity workspace
# 2. Buka terminal
cd fajar-lang

# 3. Pull perubahan terbaru
git pull origin phase-1

# 4. Start Claude Code
claude

# 5. First message ke Claude Code:
# "Read CLAUDE.md, then PLANNING.md, TASKS.md, RULES.md.
#  What is the next uncompleted task? Begin."
```

### 5.2 Selama Sesi

- Claude Code akan mengikuti WORKFLOW.md secara otomatis
- Setiap task: THINK → DESIGN → TEST → IMPL → VERIFY → UPDATE
- Jangan interrupt — biarkan Claude menyelesaikan satu task sebelum pindah
- Gunakan `effort high` untuk keputusan arsitektur
- Gunakan `effort medium` untuk implementasi rutin

### 5.3 Mengakhiri Sesi

```bash
# Claude Code akan otomatis:
# 1. Run cargo test
# 2. Run cargo clippy -- -D warnings
# 3. Update TASKS.md

# Setelah Claude selesai:
git add .
git commit -m "feat: complete T1.x.x - [description]"
git push origin phase-1
```

---

## 6. Tips Google Antigravity

### 6.1 Keyboard Shortcuts

| Shortcut | Fungsi |
|----------|--------|
| `Ctrl+`` ` | Toggle terminal |
| `Ctrl+Shift+P` | Command palette |
| `Ctrl+S` | Save file |
| `Ctrl+Shift+B` | Run build task (`cargo build`) |
| `Ctrl+Shift+T` | Run test task (`cargo test`) |

### 6.2 Environment Persistence

Google Antigravity menyimpan workspace state antara sesi. Namun, pastikan:

- Selalu commit dan push sebelum menutup workspace
- Environment variables (`ANTHROPIC_API_KEY`) perlu di-set ulang setiap sesi — simpan di `.bashrc`
- Cargo cache tersimpan otomatis di workspace storage

### 6.3 Resource Management

Untuk menghemat resource di Antigravity:

- Gunakan `cargo check` daripada `cargo build` untuk validasi cepat
- Jalankan `cargo test --lib` untuk unit test saja (skip integration)
- Matikan workspace saat tidak digunakan untuk menghemat quota

---

## 7. Multi-Agent Setup di Antigravity

Google Antigravity mendukung multiple terminal, sehingga bisa menjalankan multi-agent workflow:

### 7.1 Terminal Layout

- **Terminal 1:** Orchestrator Agent (main Claude Code session)
- **Terminal 2:** Language Core Agent (lexer/parser specialist)
- **Terminal 3:** Runtime Agent (OS/ML specialist)
- **Terminal 4:** `cargo watch` (auto-test on file change)

### 7.2 Auto-Test Watcher

```bash
# Install cargo-watch
cargo install cargo-watch

# Jalankan di terminal terpisah
cargo watch -x 'test --lib' -x 'clippy -- -D warnings'
```

> **Tip:** Dengan cargo-watch, setiap perubahan kode langsung di-test otomatis. Ini sangat berguna saat Claude Code sedang bekerja.

---

## 8. Troubleshooting Setup

| Masalah | Solusi |
|---------|--------|
| `cargo build` gagal: linker not found | `sudo apt-get install build-essential` |
| Claude Code: model not available | Pastikan API key valid, jalankan: `claude config set model claude-opus-4-6` |
| Permission denied saat `npm install -g` | `npm config set prefix ~/.npm-global && export PATH=~/.npm-global/bin:$PATH` |
| Workspace Antigravity lambat | Upgrade machine type atau kurangi extension yang aktif |
| Git push rejected | `git pull --rebase origin phase-1`, lalu push ulang |
| Rust compilation lambat | Gunakan `cargo check`, atau tambahkan `[profile.dev] opt-level = 0` di Cargo.toml |

*Dokumen ini mencakup setup awal. Untuk troubleshooting pengembangan, lihat TROUBLESHOOTING.md.*

---

*Setup Version: 1.0 | Stack: Google Antigravity + Claude Opus 4.6 + Rust*
