# FAJAR LANG — STRATEGIC COMPASS

**Dokumen panduan strategis untuk pengembangan berkelanjutan Fajar Lang**
**Audience: Sesi Claude Code, kontributor masa depan, dan Fajar sendiri**
**Versi: 1.0 — Mei 2026**
**Status: Living document — update hanya untuk perubahan strategis fundamental**

---

## 0. Cara Pakai Dokumen Ini

Dokumen ini adalah **kompas strategis**, bukan dokumentasi teknis. Tujuannya memberi setiap sesi pengembangan Fajar Lang konteks tentang:

1. Apa Fajar Lang sebenarnya (jujur), bukan apa yang diklaim README
2. Mengapa Fajar Lang ada dan untuk siapa
3. Bagaimana mengambil keputusan teknis ketika ada trade-off
4. Apa yang harus dikejar, apa yang harus dipangkas

**Cara membaca:**
- Baca seluruhnya di awal sesi pengembangan atau saat onboarding kontributor baru
- Konsultasikan **Section 7 (Decision Framework)** sebelum menambah fitur signifikan
- Konsultasikan **Section 5 dan 6** ketika menentukan prioritas

**Cara update:**
Dokumen ini diupdate **hanya** ketika ada perubahan strategis fundamental — misalnya validasi eksternal mengubah posisi pasar, atau hipotesis arsitektural terbukti salah. Update teknis kecil tidak perlu menyentuh file ini.

**Aturan emas:**
> *Jika sesi pengembangan akan menambahkan fitur yang tidak lulus Decision Framework di Section 7, jangan tambahkan, walaupun secara teknis menyenangkan untuk dikerjakan.*

---

## 1. Identitas Sebenarnya Fajar Lang

### 1.1 Apa Fajar Lang sebenarnya (jujur)

Fajar Lang adalah:

- **Bahasa pemrograman riset/eksperimental** yang dibangun di atas Rust dengan backend Cranelift dan LLVM
- **Proyek solo** dari Muhamad Fajar Putranto, dimulai 2025/2026
- **Eksplorasi konseptual** dari ide orisinal: pemisahan domain (`@kernel`/`@device`/`@safe`) di level type system
- **Dogfooded** lewat dua eksperimen OS kernel (FajarOS Nova x86_64, FajarOS Surya ARM64) dan implementasi parsial fitur ML

Fajar Lang adalah **bukan**:

- Bahasa production-ready (klaim "100% production-ready" di README harus dipangkas)
- Pengganti Rust, C, Mojo, atau Zig di production
- Bahasa dengan ekosistem yang siap dipakai untuk pekerjaan kritis
- Replikasi feature-by-feature dari Rust + Python + Mojo + Idris + Koka sekaligus

Pengakuan jujur ini **bukan** mengecilkan Fajar Lang—justru memberinya **fondasi untuk berkembang dengan benar**. Bahasa pemrograman besar (Rust, Go, Swift, Zig) butuh 5-10 tahun untuk matang dengan tim 5-50 orang. Klaim premature menghilangkan kredibilitas dan menciptakan ekspektasi yang tidak bisa dipenuhi.

### 1.2 Vision yang Realistis

**Apa yang ingin dijadikan Fajar Lang dalam 5 tahun ke depan:**

> *"Bahasa pemrograman pertama di mana kernel OS dan inference neural network bisa berbagi codebase, type system, dan compiler dengan jaminan keamanan domain yang dienforce di compile-time. Niche utama: embedded AI inference di MCU dan NPU dengan resource terbatas."*

**Apa yang BUKAN tujuan:**

- Menggantikan Python untuk ML training di datacenter (Mojo akan menang)
- Menggantikan Rust untuk Linux kernel modules (Rust-for-Linux sudah masuk mainline)
- Menggantikan C untuk semua firmware MCU (terlalu luas, ekosistem terlalu mature)
- Menjadi general-purpose programming language seperti Go atau Swift

### 1.3 Differentiator Inti: Domain Isolation Type System

Setelah analisis lanskap PL global, **inilah satu kontribusi yang genuinely orisinal di Fajar Lang**:

```fajar
@kernel  → konteks kernel: raw memory, IRQ, syscall, no heap, no tensor
@device  → konteks ML: tensor, autograd, no raw pointer
@safe    → bridge: bisa panggil keduanya, compiler enforce isolasi
@unsafe  → escape hatch eksplisit
```

**Mengapa ini orisinal:**

- **Rust** punya `#![no_std]` → crate-level, bukan function-level
- **Mojo** punya `fn` vs `def` → soal type-strictness, bukan domain isolation
- **Zig** punya `comptime` → soal eksekusi, bukan domain
- **F\*/Dafny** → bisa enforce ini via refinement types, tapi butuh proof obligation manual
- **OCaml/Haskell** → punya effect systems, tapi tidak ada domain kernel-vs-ML built-in

**Belum ada bahasa mainstream yang membawa pemisahan kernel-vs-ML ke level type-system dengan compiler enforcement otomatis.**

Ini adalah **satu ide** yang harus dijaga, dipertahankan, dan diperdalam. Semua keputusan arsitektural lain harus mendukung—atau setidaknya tidak melemahkan—ide ini.

---

## 2. Lanskap Kompetitif (Honest Assessment)

### 2.1 Posisi Saat Ini

Fajar Lang berkompetisi atau berbagi mindshare dengan beberapa kelompok bahasa:

| Kelompok | Bahasa | Status di pasar |
|---|---|---|
| Systems mainstream | C, C++, Rust | Dominan, ekosistem masif |
| Systems modern | Zig, Hare, Odin, Carbon | Growing niche |
| ML systems | Mojo, Julia, Swift for TF (defunct) | Mojo memimpin agresif |
| ML mainstream | Python + PyTorch/JAX | Dominan tapi bukan systems |
| Effects/PL research | Koka, OCaml 5, Eff, Frank, Effekt | Akademis |
| Dependent types | Idris 2, Agda, F\*, Lean 4 | Akademis |

### 2.2 Yang Sudah Dimenangkan oleh Siapa

- **OS kernel production**: Linux pakai C + Rust (Rust-for-Linux mainline sejak v6.1). Windows pakai C/C++ dengan Rust hadir di komponen baru. **Tidak ada slot kosong.**
- **Datacenter ML training**: Python+PyTorch dominan, Mojo bergerak agresif dengan MLIR + Modular MAX, dukungan vendor lengkap (NVIDIA H100/B200, AMD MI300 sejak Juni 2025).
- **Browser engines**: Servo (Rust), Chromium (C++), Firefox (C++/Rust). **Tidak ada slot kosong.**
- **Game engines**: C++ (Unreal), C# (Unity), Rust (Bevy). **Tidak ada slot kosong.**
- **High-frequency trading**: C++ dan increasingly Rust. **Tidak ada slot kosong.**
- **Embedded firmware umum**: C dominan, Rust bertumbuh, Zig niche. Sebagian besar slot terisi.

### 2.3 Niche yang Masih Terbuka

Yang **belum** punya pemenang yang jelas:

1. ✅ **Embedded ML inference dengan compiler-enforced safety** — niche kecil tapi tidak ada incumbent. Pasar real: medical devices, automotive ADAS, drone autopilot, IoT sensors dengan ML on-device.

2. ✅ **Educational PL untuk systems + ML hybrid teaching** — kampus butuh bahasa yang mengilustrasikan konsep type-level domain isolation untuk pengajaran.

3. ⚠️ **Indonesian-origin programming language** — branding bagus, tapi ekosistem global butuh adopsi internasional yang tidak bisa dipaksa lewat narasi nasionalis.

**Kesimpulan strategis: Fajar Lang harus menarget niche #1, dengan #2 sebagai bonus organik, dan #3 sebagai narrative tambahan—bukan inti.**

### 2.4 Komparasi Multi-Dimensi

Tabel ini adalah versi terkalibrasi dari komparasi di README:

| Dimensi | Fajar Lang | Mojo | Rust | Zig | C |
|---|---|---|---|---|---|
| Maturity (skala 1-10) | 2 | 5 | 9 | 6 | 10 |
| **Domain isolation** | **9** | 1 | 2 | 1 | 0 |
| ML built-in | 6 (klaim) | 9 | 3 (lib) | 0 | 0 |
| OS kernel capability | 7 (klaim) | 1 | 9 | 8 | 10 |
| GPU codegen | 4 (klaim) | 9 (MLIR) | 5 (lib) | 3 | 7 (CUDA) |
| Compile speed | TBD | 7 | 4 | 9 | 9 |
| Ekosistem | 1 | 4 | 10 | 4 | 10 |
| Production users | 0 | 5 | 10 | 6 | 10 |
| Foundation backing | 0 | 8 (Modular ~$130M) | 10 | 7 (ZSF) | 10 (ISO) |

**Pelajaran:** Fajar Lang menang **hanya** di domain isolation. Di semua dimensi lain, ada bahasa yang lebih matang. Strategi yang masuk akal: **double down** di domain isolation, jangan tersebar di dimensi lain.

### 2.5 Pesaing Terdekat: Detailed Notes

#### Mojo — Pesaing Konseptual Paling Langsung

Sama-sama menarget gabungan systems + ML. Kelebihan Mojo:

- **MLIR-first** (bukan LLVM langsung). MLIR didesain untuk heterogeneous hardware (CPU/GPU/TPU/ASIC/FPGA) dan adalah arah masa depan untuk compiler ML
- **Chris Lattner** sebagai chief architect (creator LLVM, Clang, Swift, MLIR)
- **Production validation**: 15% throughput gain di B200 vs vLLM untuk Gemma 4
- **Python superset goal** memberi runway adopsi besar (~10 juta dev Python)
- AMD MI300 support sejak Juni 2025

Kelemahan Mojo (peluang Fajar Lang):

- Belum sepenuhnya open-source (target 2026)
- Tidak menarget OS kernel / bare-metal — Mojo butuh runtime
- Tidak punya konsep domain-isolation seperti `@kernel`/`@device`
- Bukan untuk embedded MCU (STM32/ESP32) — di sini Fajar Lang punya niche

**Verdict:** Untuk ML production di datacenter GPU, Mojo akan menang dalam 3-5 tahun. Untuk *embedded* ML di MCU + bare-metal NPU, Fajar Lang punya angle yang Mojo tidak target.

#### Rust — Foundation yang Membentuk Fajar Lang

Fajar Lang dibangun di atas Rust (459K LOC Rust di compiler). Sintaksnya juga "Rust-inspired".

Apa yang Rust lakukan yang Fajar Lang belum bisa tandingi:

- **Borrow checker dengan lifetimes** yang sudah formally verified secara parsial (RustBelt, Stacked Borrows)
- **Production ekosistem**: Cargo, crates.io, Tokio, Serde — battle-tested miliaran request/hari
- **Linux kernel acceptance** (Rust-for-Linux sejak v6.1)
- **Foundation governance** yang netral, multi-vendor (AWS, Google, Microsoft, Mozilla, Huawei)
- **Spec & semantik** yang sedang diformalisasi (Ferrocene tersertifikasi ISO 26262)

Klaim Fajar Lang "Ownership tanpa lifetimes" perlu klarifikasi penting: lifetime di Rust bukan keribetan opsional, itu mekanisme membuktikan referensi tidak dangling. Menghapusnya berarti memilih salah satu strategi alternatif (lihat Section 6.2).

#### Zig — Filosofi Berbeda, Niche Mirip

Filosofi Zig **berlawanan** dengan Fajar Lang dalam beberapa hal:

| Aspek | Fajar Lang | Zig |
|---|---|---|
| Filosofi | Compiler enforce sebanyak mungkin | Compiler enforce seminimal mungkin |
| Memory safety | Ownership-based (klaim) | Manual + runtime checks (debug) |
| Hidden control flow | Beberapa | Nol toleransi |
| Compile speed | Belum diukur | Self-hosted, ultra cepat |
| Allocator | Implisit di `@kernel` (klaim no-heap) | **Eksplisit di setiap function signature** |
| Comptime | `const fn` + `comptime {}` | Comptime sebagai paradigma utama |
| ML focus | Inti dari desain | Bukan target |

Zig punya **production track record** yang Fajar Lang belum punya: Bun (JS runtime), TigerBeetle (financial database), Ghostty (terminal), River (Wayland compositor).

Pelajaran dari Zig untuk Fajar Lang: filosofi "tidak ada hidden allocations, allocator selalu eksplisit" sangat powerful untuk embedded. Ini patut dipertimbangkan untuk konteks `@kernel`.

---

## 3. Single Bet Strategy: Embedded AI Safety

Argumen di Section 2 berujung ke satu kesimpulan strategis: **Fajar Lang harus memilih satu pertaruhan dan mengeksekusinya 100%.**

### 3.1 Pertaruhan: Embedded AI Safety

**Niche spesifik:** Bahasa untuk **embedded AI inference** di **MCU/NPU dengan resource terbatas**, di mana keamanan domain (kernel-level vs ML-level) dienforce di compile-time, sehingga regressions yang fatal di safety-critical embedded tidak mungkin terjadi.

**Target hardware konkret:**

- ARM Cortex-M55/M85 dengan Helium (MVE) — ML inference di MCU
- Qualcomm Hexagon (Q6A, QCS6490) — sudah dikerjakan via FajarOS Surya
- Espressif ESP32-S3/P4 dengan dual-core + AI accelerator
- STMicroelectronics STM32N6 dengan Neural-ART NPU
- Kendryte K210/K230 (RISC-V + KPU)
- Renesas RA8 dengan Helium

**Target use case konkret:**

- Wearable medical device dengan ML untuk arrhythmia detection
- Automotive ADAS sensor fusion dengan ML on-device
- Industrial IoT predictive maintenance
- Drone autopilot dengan vision-based obstacle avoidance
- Smart agriculture sensor dengan crop health classification

**Yang dijual ke developer:**

> *"Kalau kompiler Fajar Lang berhasil mengcompile kode Anda untuk MCU, maka kode kernel Anda secara matematis tidak mungkin trigger heap allocation, dan kode ML Anda tidak mungkin dereference raw pointer. Kelas bug yang biasanya butuh review manual berjam-jam, dieliminasi by construction."*

### 3.2 Mengapa Niche Ini Bekerja

1. **Tidak ada incumbent dominan.** C dipakai tapi tidak ada safety guarantee. Rust no_std bisa tapi ML inference manual lewat library. Mojo tidak target embedded MCU. Zig minim ML support.

2. **Demand nyata.** Safety-critical embedded ML adalah masalah riil di automotive (ISO 26262 ASIL-D), medical (IEC 62304 Class C), dan industrial (IEC 61508 SIL 3-4). Auditor butuh bukti formal bahwa kelas bug tertentu tidak mungkin terjadi.

3. **Selaras dengan differentiator inti.** `@kernel`/`@device`/`@safe` adalah jawaban langsung untuk masalah di domain ini.

4. **Scope manageable solo.** Tidak butuh 100 backend, 10K crate ekosistem, atau 50 fitur. Butuh: 1-2 chip support yang solid, 1 model deployment toolchain, 1 set safety guarantees yang dibuktikan.

5. **Indonesia angle.** Pasar IoT industrial Indonesia sedang bertumbuh; ada peluang awal di startup lokal (Telkom IoT, Indosat, Nodeflux untuk vision, Digiserve, dll.).

### 3.3 Apa yang Tidak Dikejar (Konsekuensi Pertaruhan)

Memilih pertaruhan ini berarti **secara aktif tidak mengejar:**

- Datacenter GPU training (Mojo menang)
- General-purpose web/server backend (Rust/Go menang)
- Linux kernel modules (Rust-for-Linux sudah masuk)
- Browser engines, game engines, OS desktop (semua punya pemenang)
- Bahasa scripting umum (Python, JavaScript dominan)

Ini bukan pengakuan kekalahan—ini **fokus**. Bahasa kecil yang menang di 1 niche jauh lebih bernilai daripada bahasa besar yang kalah di 10 niche.

---

## 4. Arsitektur Inti yang Harus Dipertahankan

Komponen yang **harus tetap ada** karena mendefinisikan identitas Fajar Lang:

### 4.1 Type-Level Domain Isolation (NON-NEGOTIABLE)

`@kernel` / `@device` / `@safe` / `@unsafe` adalah inti. Setiap perubahan compiler/analyzer harus:

- Mempertahankan separasi domain
- Membuat error message yang jelas saat domain dilanggar
- Mendukung composition lewat `@safe` bridge

**Yang tidak boleh:**

- Mengizinkan `@kernel` memanggil `@device` langsung tanpa `@safe`
- Membuat domain isolation jadi opt-in (harus default-on)
- Menambah domain baru yang membingungkan (tahan diri dari `@gpu`, `@async`, dll. — pakai annotation atau effect terpisah)

### 4.2 Cranelift JIT + LLVM AOT (Pertahankan Keduanya)

- **Cranelift** untuk dev loop cepat dan REPL
- **LLVM** untuk release build dengan O2/O3/LTO/PGO

Strategi ini sama dengan Rust (rustc_codegen_cranelift di-merge ke nightly). Validasi pilihan ini.

**Yang harus dijaga:**

- Backend abstraction yang bersih (jangan bocorkan detail Cranelift ke LLVM atau sebaliknya)
- Test setiap fitur di kedua backend (regression sering muncul)

### 4.3 Native Tensor sebagai First-Class

`Tensor` harus tetap built-in dengan:

- Compile-time shape checking (sebagian besar kasus)
- Runtime fallback untuk shape dinamis (dengan annotation eksplisit)
- Dtype inference yang benar
- Operasi dasar: matmul, reshape, slice, concat, transpose, reduction

**Yang harus dijaga:**

- Tensor operations hanya valid di `@device` atau `@safe`
- Tidak ada implicit allocation di `@kernel`

### 4.4 @safe sebagai Default

Semua function tanpa annotation otomatis `@safe`. Ini memastikan:

- Ergonomis untuk casual user
- Domain checking aktif tanpa annotation eksplisit
- Naik ke `@kernel`/`@device` adalah opt-in, bukan default

### 4.5 Pipeline Operator dan Ergonomics

`|>` adalah ergonomic win yang murah. Pertahankan. Tapi tahan diri dari:

- Operator overloading kustom yang membingungkan
- Macro yang terlalu pintar (lihat pengalaman Rust `macro_rules` yang kadang sulit didebug)

### 4.6 String Interpolation `f"..."`

Ergonomic win lain yang murah. Pertahankan.

---

## 5. Yang Harus Dipangkas atau Dibekukan

Ini adalah keputusan paling sulit dan paling penting. Untuk bahasa pre-1.0 dengan satu kontributor, **scope creep adalah ancaman eksistensial**.

### 5.1 Bekukan/Sederhanakan

Fitur berikut sudah ada di README/codebase tapi **harus dibekukan atau disederhanakan drastis** sampai core stabil:

| Fitur | Status saat ini | Rekomendasi |
|---|---|---|
| Algebraic effects + handlers | Diklaim ada | **Bekukan**. Pisah ke branch `effects-research`. Kembali setelah core v1.0 stabil. |
| Dependent types (Pi/Sigma/refinement) | Diklaim ada | **Bekukan**. Pisah ke branch `deptypes-research`. Mungkin tidak akan kembali. |
| WASI P2 component model | Diklaim ada | **Bekukan**. Tidak relevan untuk niche embedded. |
| GPU codegen (SPIR-V/PTX) | Diklaim ada | **Sederhanakan**. Untuk niche embedded, NPU SDK FFI lebih penting daripada full GPU codegen. |
| Distributed runtime (Raft) | Diklaim ada | **Hapus dari core**. Tidak relevan untuk niche embedded. Jadikan side library. |
| FajarOS Nova (x86_64) | 41K LOC dogfood | **Pisah ke repo terpisah**. Bukan bagian core compiler. |
| FajarOS Surya (ARM64) | Q6A BSP | **Pertahankan tapi pisah repo**. Bukti embedded support, tapi bukan bagian core. |
| GUI runtime (winit + softbuffer) | Diklaim ada | **Pisah ke optional crate**. Tidak relevan untuk niche. |
| HTTP server framework | Diklaim ada | **Pisah ke optional crate**. Tidak relevan untuk niche. |
| WebSocket / MQTT / BLE / database | Library bindings | **Pisah ke optional crates**. Bisa jadi side projects. |
| SMT verification (DO-178C) | Diklaim ada | **Bekukan**. Butuh tim untuk certification serius. |

**Prinsip:** Setiap fitur yang tidak langsung mendukung embedded AI safety harus dipindahkan ke **optional crate** atau **repo terpisah**, atau dibekukan.

### 5.2 Pangkas Klaim README

README saat ini punya beberapa klaim yang harus dikalibrasi:

- ❌ "100% production-ready" → **hapus atau ganti dengan "experimental, pre-1.0"**
- ❌ "All 500 tasks verified" → **hapus, tidak meaningful tanpa konteks**
- ❌ "11,395 tests" → **biarkan tapi tambahkan disclaimer scope** ("most tests cover scaffolding, integration coverage TBD")
- ❌ "459K LOC compiler" → **biarkan tapi tambahkan konteks** (banyak yang scaffolding, akan dipangkas)
- ❌ "V14 Infinity all features verified E2E" → **hapus**
- ❌ Badges yang dibuat sendiri (FajarQuant 55-88% improvement, JIT 76x speedup) → **biarkan tapi link ke benchmark reproducible**
- ✅ "Made in Indonesia" → **pertahankan**, ini bagian identitas

**Aturan emas:** Setiap klaim harus diverifikasi dengan test atau benchmark yang reproducible. Jika tidak bisa diverifikasi, pangkas.

### 5.3 Versioning Strategy

Versi `v24.0.0` dalam ~1 tahun pengembangan adalah anti-pattern. Industri PL melihat ini sebagai signal alarm—Rust butuh 5 tahun dari 0.1 ke 1.0; Zig 9 tahun masih pre-1.0; Mojo masih beta.

**Rekomendasi:** Reset penomoran ke `v0.x.y` dengan semver yang jujur:

- `v0.5.0` — current state, experimental (pilih angka yang merefleksikan kemajuan tanpa inflasi)
- `v0.6.0`, `v0.7.0`, ... — increment minor untuk fitur baru atau breaking changes
- `v1.0.0` — hanya ketika ada **external production user** yang berhasil shipping product dengan Fajar Lang

Ini menyelaraskan dengan disiplin yang dihormati di komunitas PL.

---

## 6. Yang Harus Diperdalam (Depth over Breadth)

Hasil pemangkasan di Section 5 harus disalurkan ke **pendalaman** di area berikut:

### 6.1 Formal Spec @kernel/@device/@safe (Prioritas #1)

Tulis dokumen formal yang menjelaskan:

- **Type system rules** dalam notasi inferensi: `Γ ⊢ e : τ @ d`, di mana `d ∈ {kernel, device, safe, unsafe}`
- **Composition rules**: kapan bridge `@safe` valid, apa preconditions
- **Effect tracking**: heap allocation, pointer dereference, IRQ access, tensor op
- **Soundness theorem**: "well-typed program tidak melanggar domain"
- **Decidability**: type checking selalu terminate

Format: paper mini (10-15 halaman) dan/atau bab di mdBook.

**Mengapa ini penting:** Ini adalah satu-satunya cara membuktikan ke komunitas PL bahwa ide ini bukan slogan. Tanpa formal spec, klaim "compiler-enforced safety" hanya marketing.

**Target venue:** PLDI Workshop, ICFP Workshop, OOPSLA SRC, atau TyDe (Type-Driven Development).

### 6.2 Memory Model Tanpa Lifetimes

README mengklaim "ownership tanpa lifetimes". Ini perlu dijelaskan. Pilihan strategi yang masuk akal:

1. **Hylo-style mutable value semantics** — variabel adalah independent values, sharing eksplisit lewat parameter passing convention (`let`, `inout`, `sink`, `set`)
2. **Region inference (Cyclone-style)** — compiler infer lifetime, programmer tidak menulisnya
3. **Reference counting + Cell-style mutability** — mirip Swift, hidden cost di runtime

**Pilih satu, dokumentasikan formal, jangan mix-and-match.** Untuk niche embedded (no heap di kernel), **Hylo-style atau region inference** adalah pilihan yang masuk akal. RC dengan hidden cost menabrak semangat embedded.

### 6.3 Tensor Shape Checking Compile-Time

Untuk niche embedded ML, **shape error di runtime adalah bug fatal**. Fajar Lang harus excel di shape checking compile-time:

- `Tensor<f32, [B, 224, 224, 3]>` dengan const generic dimensions
- Inference shape pada operasi: `matmul(A: Tensor<f32, [M, K]>, B: Tensor<f32, [K, N]>) -> Tensor<f32, [M, N]>`
- Error message yang menunjukkan dimension mismatch dengan jelas
- Fallback ke runtime check untuk shape dinamis (dengan annotation eksplisit `Tensor<f32, ?>`)

**Rujukan:** Dex (Google), Hasktorch, Tensor-Annotated-Python.

### 6.4 Embedded Toolchain End-to-End

**Pilih satu chip dan tunjukkan end-to-end deployment yang berfungsi.**

Kandidat utama: **STM32N6 dengan Neural-ART NPU** (Cortex-M55 + dedicated NPU, dirilis 2025, growing momentum).

Deliverables:

1. Cross-compile target di LLVM (`thumbv8m.main-none-eabihf`)
2. BSP minimal: GPIO, UART, SPI, I2C, DMA, timer, NVIC
3. NPU SDK FFI working (ST Edge AI)
4. Sample app: image classification (MNIST atau CIFAR-10) yang berjalan di hardware
5. Memory budget analysis (flash + RAM consumption vs C+CMSIS-NN)
6. Latency benchmark vs C+CMSIS-NN

**Inilah yang membuktikan klaim embedded AI.** Tanpa ini, semua diskusi adalah teori.

### 6.5 Diagnostic Quality (Error Message)

Bahasa modern menang atau kalah di error message. Rust menang sebagian besar karena `rustc` punya error message terbaik.

**Investasi:**

- Error code catalog (sudah ada 80+ klaim, audit dan validasi)
- Span-aware error dengan source pointing
- Suggestion ("did you mean...?")
- Domain-specific suggestion ("this `@kernel` function tries to allocate heap; did you mean `@safe`?")
- Warna terminal yang readable

### 6.6 Documentation-Driven Development

Untuk fitur baru apapun:

1. Tulis dokumentasi dulu (spec + tutorial + API reference)
2. Tulis test berdasarkan dokumentasi
3. Implementasi sampai test pass
4. Update CHANGELOG dengan akurat

Ini lambat tapi memastikan setiap fitur punya bukti, bukan hanya kode.

---

## 7. Decision Framework untuk Setiap Sesi Coding

**Inilah aturan paling praktis yang harus dikonsultasikan setiap sesi.**

Sebelum menambahkan/mengubah fitur signifikan, jawab 5 pertanyaan ini. Jika **mayoritas jawaban "Tidak"**, jangan kerjakan.

### Pertanyaan 1: Apakah ini memperdalam differentiator inti?

Differentiator inti = `@kernel`/`@device`/`@safe` domain isolation + niche embedded AI safety.

- ✅ Ya jika: fitur memperkuat type system domain, atau membuat embedded AI lebih aman/ergonomis
- ❌ Tidak jika: fitur menambah surface area di domain umum (web, desktop, datacenter)

**Contoh ya:** Tambah cek shape tensor compile-time, tambah BSP untuk Cortex-M55
**Contoh tidak:** Tambah HTTP framework, tambah GUI widget library

### Pertanyaan 2: Apakah scope manageable untuk solo developer?

Estimasi kasar: jika butuh > 2 minggu untuk minimum viable, dan > 2 bulan untuk production-quality, ini terlalu besar.

- ✅ Ya jika: bisa diselesaikan dengan 1-2 minggu fokus
- ❌ Tidak jika: butuh tim atau berbulan-bulan

**Contoh ya:** Implementasi struktural pruning untuk tensor (1-2 minggu)
**Contoh tidak:** Implementasi distributed Raft consensus (butuh tim, berbulan-bulan)

### Pertanyaan 3: Apakah ada paper, spec, atau test yang membuktikan ini bekerja?

- ✅ Ya jika: ada referensi PL atau paper akademis, atau spec formal Fajar Lang sendiri
- ❌ Tidak jika: hanya intuisi atau "kelihatannya keren"

**Contoh ya:** Compile-time shape checking ada di Dex paper, ada di Hasktorch
**Contoh tidak:** "Quantum-inspired tensor operations" tanpa basis riset

### Pertanyaan 4: Apakah ini bisa dijelaskan dalam 5 menit ke developer baru?

Jika konsep terlalu eksotis untuk dijelaskan singkat, kemungkinan besar terlalu kompleks untuk dimaintain solo.

- ✅ Ya jika: ada elevator pitch yang jelas dan satu contoh kode yang menunjukkan value
- ❌ Tidak jika: butuh 20 halaman teori PL untuk dijelaskan

### Pertanyaan 5: Apakah ini dipakai oleh use case nyata embedded AI?

- ✅ Ya jika: ada minimal 1 use case konkret di target hardware (STM32, Cortex-M55, ESP32, Hexagon)
- ❌ Tidak jika: hanya "mungkin berguna untuk seseorang"

### Anti-pattern yang Harus Dihindari

1. **Adding features karena bahasa lain punya** — Rust punya X, jadi Fajar Lang harus punya. **Tidak.** Punya hanya jika selaras dengan niche.

2. **"V25 release dengan 100 fitur baru"** — increment versi besar dengan banyak fitur tipis lebih buruk daripada increment kecil dengan satu fitur dalam.

3. **Implementasi cepat tanpa test** — kode yang tidak ditest tidak bisa dipercaya. Untuk solo developer, test adalah satu-satunya safety net.

4. **README hyperbole** — setiap klaim harus bisa dibuktikan. "Production-ready", "world-class", "first ever" perlu evidence atau dipangkas.

5. **Ignoring external feedback** — komunitas PL kecil tapi tajam. Submit paper, dapat review, terima kritik konstruktif.

---

## 8. Roadmap Realistis 24 Bulan

### Phase 0: Konsolidasi dan Audit Jujur (Bulan 1-3)

**Tujuan:** Bersihkan codebase, kalibrasi klaim, set fondasi.

Deliverables:

- [ ] Reset versi ke `v0.5.0` (atau angka realistis lainnya)
- [ ] Pangkas README — hapus klaim yang tidak bisa diverifikasi
- [ ] Pisahkan FajarOS Nova dan Surya ke repo terpisah (tetap milik Fajar)
- [ ] Pisahkan optional crates (HTTP, GUI, MQTT, BLE, database) ke workspace member terpisah atau repo terpisah
- [ ] Bekukan branch `effects-research` dan `deptypes-research`
- [ ] Audit test suite yang sebenarnya pass dan yang flaky
- [ ] Tulis `HONEST_STATUS.md` di root yang menjelaskan apa yang bekerja dan apa yang aspirational

### Phase 1: Formal Spec dan Core Stability (Bulan 3-6)

**Tujuan:** Buktikan klaim type system dengan spec formal, stabilkan core.

Deliverables:

- [ ] **Paper draft v1**: "Domain-Isolated Type System for Embedded AI Safety" (10-15 halaman)
- [ ] mdBook chapter formal: type rules, composition, soundness sketch
- [ ] Compile-time shape checking yang solid untuk operasi tensor dasar
- [ ] Memory model document (pilih satu strategi: Hylo-style atau region inference)
- [ ] Error message audit dan improvement (target: parity dengan Rust untuk kasus umum)
- [ ] Cranelift backend stable untuk subset core (tanpa fitur eksotis)

### Phase 2: Embedded Niche Showcase (Bulan 6-12)

**Tujuan:** Bukti end-to-end bahwa Fajar Lang bisa ship embedded AI di hardware nyata.

Deliverables:

- [ ] Pilih satu chip target: **STM32N6 dengan Neural-ART** (rekomendasi)
- [ ] BSP minimal: GPIO, UART, SPI, I2C, DMA, timer, NVIC
- [ ] NPU SDK FFI working (ST Edge AI)
- [ ] Sample app: MNIST classification end-to-end di hardware nyata
- [ ] Benchmark vs C+CMSIS-NN: latency, flash, RAM
- [ ] Tutorial blog post + video demo
- [ ] Submit ke Hacker News / Lobsters / r/embedded untuk feedback

### Phase 3: External Validation (Bulan 12-18)

**Tujuan:** Dapat validasi eksternal yang nyata.

Deliverables:

- [ ] Submit paper ke workshop PL (PLDI SRC, ICFP student research, TyDe, atau OOPSLA SRC)
- [ ] Cari 3-5 early adopter (mahasiswa S2/S3 PL, riset BRIN, startup IoT)
- [ ] GitHub stars target: 100+ organic (bukan dipromosikan paksa)
- [ ] External contributor pertama (PR yang di-merge dari non-Fajar)
- [ ] Talk di konferensi lokal (KOMNAS Informatika, PIONIR ITB, kampus mitra)

### Phase 4: v1.0 yang Jujur (Bulan 18-24)

**Tujuan:** Ship v1.0 dengan scope sempit tapi solid.

Kriteria v1.0:

- [ ] Minimal 1 production user shipping product dengan Fajar Lang
- [ ] Spec formal complete (type system + memory model)
- [ ] Embedded toolchain stable untuk minimal 2 chip families
- [ ] Documentation complete (mdBook 100+ halaman, tutorial seri)
- [ ] Test coverage > 80% untuk core compiler (real coverage, bukan inflated)
- [ ] Zero known soundness bug di domain isolation

**Catatan:** v1.0 adalah commitment besar. Lebih baik telat 6 bulan dengan kualitas terjaga daripada ship v1.0 dengan bug fundamental.

---

## 9. Metrik Kesuksesan yang Honest

### 9.1 Anti-metrik (Yang Tidak Boleh Dipakai)

- ❌ **LOC compiler.** 459K LOC bukan tanda kemajuan; bisa jadi tanda scope creep. Linux kernel tumbuh ~3% LOC per tahun.
- ❌ **Jumlah test.** 11K test tanpa konteks scope tidak meaningful. 1K test yang menguji feature inti lebih bernilai dari 10K yang scaffolding.
- ❌ **Version number.** v24.0.0 dalam 1 tahun adalah signal alarm. Industri menghormati semver yang disiplin.
- ❌ **Klaim "100% production-ready".** Tidak ada bahasa pre-1.0 yang production-ready.
- ❌ **Self-rated badges di README.** Achievement badges yang dihasilkan sendiri tidak bermakna eksternal.

### 9.2 Metrik yang Sebenarnya Bermakna

- ✅ **External GitHub stars** (organik, bukan promosi)
- ✅ **External contributors** (PR di-merge dari orang lain)
- ✅ **External users** (proyek di GitHub yang depend on Fajar Lang)
- ✅ **GitHub Discussions activity** (pertanyaan dari pengguna nyata, bukan author)
- ✅ **Issue close ratio** (bug report dari eksternal yang di-fix)
- ✅ **Konferensi/paper acceptance** (workshop PLDI, ICFP, OOPSLA, TyDe)
- ✅ **Citation count** (jika paper diterima)
- ✅ **Benchmark wins di domain spesifik** (mis. embedded ML latency vs C+CMSIS-NN)
- ✅ **Real product shipped** (medical device, automotive ECU, IoT sensor yang pakai Fajar Lang di production)

### 9.3 Target 24 Bulan (Realistis)

| Metrik | Saat ini | Target 24 bulan |
|---|---|---|
| External stars | 0 (1 self) | 500+ |
| External contributors | 0 | 5+ |
| External users (projects depending) | 0 | 10+ |
| Workshop paper accepted | 0 | 1+ |
| Production users | 0 | 1-3 (early adopters) |
| Versi | v24 (inflasi) | v1.0 (jujur) |
| Real test coverage | TBD | > 80% core |

---

## 10. Standar Engineering yang Dipertahankan

### 10.1 Test-First untuk Fitur Core

Untuk fitur di domain isolation, type system, atau memory model:

1. Tulis property-based test (proptest, quickcheck-style)
2. Tulis golden test untuk error message
3. Implementasi sampai test pass
4. Tambah regression test untuk setiap bug yang ditemukan

### 10.2 Documentation-Driven Development

Untuk fitur baru:

1. RFC draft di `docs/rfc/` (template: motivation, design, alternatives, drawbacks)
2. Diskusi di GitHub Discussions atau internal review
3. Spec/dokumentasi di `book/` atau `docs/`
4. Test berdasarkan dokumentasi
5. Implementasi
6. Update CHANGELOG

### 10.3 Honest CHANGELOG

Format:

```
## [v0.x.y] - YYYY-MM-DD

### Added
- Fitur X (status: experimental | stable | research)

### Changed
- ...

### Fixed
- ...

### Known Issues
- ...

### Limitations
- Apa yang BELUM bekerja dan kapan diharapkan
```

Setiap entry harus actionable—pengguna harus tahu apa yang berubah dan bagaimana migrasi.

### 10.4 Honest README Template

```
# Fajar Lang (fj)

> Experimental systems language for embedded AI safety.
> **Status: pre-1.0, research-grade.** Not production-ready.

[Status badges yang akurat]

## What is this?

[2-3 paragraf jujur tentang apa yang Fajar Lang dan bukan apa]

## Differentiator

[Penjelasan @kernel/@device/@safe dengan contoh kode]

## What works today

[Daftar fitur yang sudah stable dengan link ke test/dokumentasi]

## What's experimental

[Daftar fitur yang masih research dengan disclaimer]

## What's NOT here

[Daftar use case yang Fajar Lang TIDAK target]

## Quick start

[Setup minimal yang benar-benar jalan]

## Roadmap

[Link ke STRATEGIC_COMPASS.md ini]
```

### 10.5 Code Style

- Cargo fmt + clippy default + zero warning policy
- Conventional Commits format: `<type>(<scope>): <description>`
- PR template dengan checklist (test, docs, CHANGELOG)
- CI yang test di Linux, macOS, dan minimal 1 cross-target embedded

---

## 11. Pola Komunikasi dan Branding

### 11.1 Posisioning Pesan

**Pesan utama (elevator pitch 1 kalimat):**

> *"Fajar Lang is an experimental systems language that makes embedded AI safe by construction, with compile-time isolation between kernel and ML domains."*

**Pesan untuk audience teknis:**

> *"Type-level domain isolation (`@kernel`/`@device`/`@safe`) eliminates a class of bugs in safety-critical embedded ML—heap allocations in kernel paths, raw pointer dereferences in inference paths—at compile time, not runtime."*

**Pesan untuk audience Indonesia:**

> *"Bahasa pemrograman pertama dari Indonesia yang menarget niche global: embedded AI safety. Dibangun dengan disiplin engineering kelas dunia, terbuka untuk kolaborasi internasional."*

### 11.2 Yang Tidak Boleh Dipakai

- ❌ "The only language where..." — superlative tanpa bukti
- ❌ "Production-ready" sebelum ada production user
- ❌ "Better than Rust/Mojo/C" — tone defensif yang mengundang serangan
- ❌ "Revolutionary" / "Game-changing" / "World-class" — buzzword tanpa substansi

### 11.3 Yang Boleh Dipakai

- ✅ "Experimental" / "Research-grade" / "Pre-1.0"
- ✅ "Inspired by Rust, Zig, Koka, Idris" — beri kredit ke pendahulu
- ✅ "Targets a specific niche: embedded AI safety" — fokus
- ✅ "Made in Indonesia, open to international collaboration" — origin tanpa nasionalisme sempit

### 11.4 Channel Komunikasi

Prioritas:

1. **GitHub Discussions** — untuk pertanyaan teknis dan engagement komunitas
2. **Blog/website** (PrimeCore.id atau dedicated) — untuk update besar dan tutorial
3. **PL Twitter/Bluesky** — untuk engage dengan komunitas PL global
4. **Papers di workshop akademis** — untuk validasi serius
5. **Lokal (Indonesia)** — talks di kampus, KOMNAS Informatika, IDStarTech

---

## 12. Untuk Sesi Claude Code: Aturan Operasional

Bagian ini adalah **instruksi langsung** untuk Claude Code yang bekerja di repo Fajar Lang.

### 12.1 Setiap Awal Sesi

1. **Baca file ini (STRATEGIC_COMPASS.md) jika belum dalam konteks.**
2. **Baca CLAUDE.md** untuk instruksi spesifik repo.
3. **Lihat HONEST_STATUS.md** untuk state aktual fitur.
4. **Tanyakan ke Fajar:** "Apa goal sesi ini?" — pastikan goal jelas dan terukur.

### 12.2 Sebelum Menambahkan Fitur

1. Konsultasi **Section 7 Decision Framework**.
2. Jika lulus, tulis RFC singkat (motivation, design sketch).
3. Konfirmasi dengan Fajar sebelum mulai implementasi.
4. Tulis test dulu jika feasible.

### 12.3 Saat Menulis Kode

- Konsisten dengan code style existing (cargo fmt + clippy)
- Setiap fungsi public punya doc comment
- Setiap fitur baru punya test
- Update CHANGELOG dan dokumen relevan

### 12.4 Saat Reporting Status

- **Jujur tentang apa yang bekerja dan tidak.** Jangan inflasi.
- Jika test pass, sebutkan test yang pass dan scope-nya.
- Jika ada known limitation, sebutkan eksplisit.
- Jangan klaim "production-ready" tanpa konteks.

### 12.5 Saat Menulis Dokumentasi

- Bahasa Inggris untuk core docs (audience global).
- Bahasa Indonesia untuk tutorial regional dan branding.
- Hindari hyperbole (lihat Section 11.2).
- Selalu tunjukkan contoh kode yang bisa dijalankan.

### 12.6 Anti-pattern Spesifik untuk AI-Assisted Development

Karena banyak kode awal Fajar Lang dibantu AI, ada risiko spesifik yang harus diawasi:

❌ **Ghost features** — kode yang ada tapi tidak benar-benar bekerja end-to-end.
✅ **Solusi:** setiap klaim fitur harus punya test integrasi yang jalan dari source code sampai output expected.

❌ **Scaffolding bloat** — generate banyak file boilerplate yang terlihat impressive tapi tidak ada substansi.
✅ **Solusi:** prefer DELETE atas ADD. Kode yang tidak dipakai harus dihapus, bukan dipertahankan "siapa tahu nanti".

❌ **Hallucinated API references** — dokumentasi yang merujuk ke fungsi/library yang tidak ada.
✅ **Solusi:** verify setiap link dan referensi di dokumentasi sebelum commit.

❌ **Inflated test count** — tes trivia atau placeholder yang menambah angka tapi tidak menguji apa-apa.
✅ **Solusi:** audit test suite, hapus yang trivia, fokus ke property-based dan integration test.

❌ **Version number inflation** — increment v22 → v23 → v24 dengan changelog yang tidak meaningful.
✅ **Solusi:** gunakan semver disiplin, increment minor untuk feature riil, increment patch untuk bug fix.

❌ **Klaim "X% improvement" tanpa benchmark reproducible** — angka di README yang tidak bisa direproduksi.
✅ **Solusi:** setiap angka di README harus link ke benchmark script yang bisa dijalankan ulang.

### 12.7 Saat Diminta Membuat Klaim Pemasaran

Jika Fajar (atau orang lain) meminta klaim pemasaran/README yang berlebihan:

✅ **Tawarkan alternatif yang jujur.** "Saya akan tulis 'experimental research-grade language' bukan 'production-ready', karena yang terakhir merusak kredibilitas dengan komunitas PL."

✅ **Tunjukkan contoh dari proyek yang dihormati.** Zig 9 tahun masih pre-1.0. Mojo eksplisit "still young". Disiplin ini dihormati di komunitas PL.

✅ **Pertahankan posisi jika perlu.** Klaim premature merugikan Fajar Lang lebih dari menolong. Ini adalah service jangka panjang, bukan ketidaktaatan.

### 12.8 Saat Ada Konflik Antara Speed dan Quality

Default: **quality menang.**

Untuk solo developer pre-1.0 dengan ambisi serius:

- Lebih baik 1 fitur solid daripada 10 fitur setengah jadi
- Lebih baik delay 3 bulan daripada ship dengan soundness bug
- Lebih baik test coverage tinggi daripada feature surface luas

Pengecualian: jika ada deadline eksternal (paper submission, demo konferensi), prioritaskan deliverable spesifik tapi catat technical debt eksplisit di issue tracker.

### 12.9 Saat Diminta Implementasi Fitur Eksotis

Jika diminta implementasi fitur PL canggih (mis. dependent types, algebraic effects, linear types, modal types, etc.):

1. Cek apakah ada di Section 5.1 (Bekukan list)
2. Jika ya, ingatkan Fajar bahwa ini sudah di-defer
3. Jika belum di list, jalankan Decision Framework
4. Jika belum di list dan lulus framework, mulai dengan **minimal core calculus** terlebih dahulu, bukan implementasi penuh
5. Tulis spec formal sebelum kode

### 12.10 Saat Bekerja dengan FajarOS

FajarOS Nova dan Surya adalah **dogfood project**, bukan inti compiler. Saat bekerja di FajarOS:

- Konsisten dengan API compiler—jika compiler berubah, FajarOS adalah validator pertama
- Bug di FajarOS yang reveal bug compiler adalah prioritas tinggi
- FajarOS sendiri bukan target distribusi—ini bukan pesaing Linux/Zephyr

---

## 13. Penutup: Mengapa Fajar Lang Worth It

Setelah semua kritik jujur di atas, **Fajar Lang tetap worth pursuing**. Inilah alasannya:

### 13.1 Ide Inti Genuinely Berharga

Domain isolation di type system adalah kontribusi PL yang menarik. Tidak ada bahasa mainstream yang melakukannya. Kalau Fajar Lang berhasil membuktikan ide ini bekerja di niche embedded AI, itu adalah kontribusi yang nyata ke komunitas PL global.

### 13.2 Niche Embedded AI Safety Punya Demand Real

Pasar embedded AI tumbuh cepat (medical, automotive, industrial IoT). Safety-critical embedded butuh tools yang lebih baik dari C. Rust hadir tapi belum punya ergonomic ML. Mojo tidak target embedded MCU. **Ada gap yang nyata.**

### 13.3 Indonesia Punya Posisi Unik

- Pasar IoT industrial yang bertumbuh
- Talenta engineering yang underutilized
- Pemerintah mendorong kedaulatan teknologi
- Belum ada bahasa pemrograman besar yang berasal dari Indonesia (atau Asia Tenggara)

Fajar Lang bisa menjadi **proof of concept** bahwa programming language design bukan monopoli Silicon Valley/Eropa/Jepang.

### 13.4 Fajar Sebagai Author

Fajar Putranto punya kombinasi yang langka untuk founder bahasa pemrograman:

- 28+ tahun pengalaman profesional di domain non-tech (tax, legal, business)
- Curiosity teknis yang serius (programming, systems, ML)
- Resource finansial untuk sustained development (TaxPrime, PrimeCore)
- Network ke pemerintah, kampus, industri (IKANAS STAN, ACEXI)
- Disiplin executive untuk eksekusi multi-tahun

Kombinasi ini bukan jaminan sukses, tapi memberi Fajar Lang **runway yang lebih panjang** dari proyek mahasiswa atau startup pre-revenue.

### 13.5 Garis Bawah

> *Fajar Lang punya peluang nyata untuk menjadi **bahasa kecil yang signifikan** di niche embedded AI safety, asal disiplin scope dan honesty dipertahankan. Yang harus dihindari adalah mengejar mimpi "menggantikan Rust/Mojo/C" yang akan menghabiskan energi tanpa hasil. Yang harus dikejar adalah mimpi yang lebih sempit tapi lebih dalam: **bahasa pertama yang membuat embedded AI aman by construction**.*

Itulah kompas yang harus dijaga. Setiap commit, setiap PR, setiap rilis, harus membawa Fajar Lang lebih dekat ke utara magnetik ini.

---

## Lampiran A: Checklist Setiap Sesi Claude Code

```
[ ] Baca STRATEGIC_COMPASS.md (file ini) jika belum di konteks
[ ] Baca CLAUDE.md untuk instruksi repo
[ ] Konfirmasi goal sesi dengan Fajar (jika unclear)
[ ] Cek apakah goal sejalan dengan niche embedded AI safety
[ ] Sebelum menambah fitur: jalankan Section 7 Decision Framework
[ ] Tulis test/spec sebelum implementasi (jika feasible)
[ ] Implementasi minimal yang lulus test
[ ] Update CHANGELOG, dokumentasi, dan README jika relevan
[ ] Honest report status: apa yang bekerja, apa yang belum
[ ] Hindari klaim hyperbolic atau version number inflation
```

## Lampiran B: Quick Reference — Yes/No Decisions

| Pertanyaan | Jawaban |
|---|---|
| Tambah fitur untuk web framework? | ❌ Tidak (di luar niche) |
| Tambah fitur untuk GUI desktop? | ❌ Tidak (di luar niche) |
| Tambah BSP untuk STM32N6? | ✅ Ya (core niche) |
| Tambah dependent types penuh? | ⏸️ Bekukan (terlalu kompleks solo) |
| Tambah algebraic effects? | ⏸️ Bekukan (research, post-v1.0) |
| Tambah error message improvement? | ✅ Ya (quality of life) |
| Tambah HTTP server? | ❌ Tidak (di luar niche) |
| Tambah NPU SDK FFI (ST Edge AI, QNN)? | ✅ Ya (core niche) |
| Tambah Linux kernel module support? | ⚠️ Mungkin, tapi prioritas rendah |
| Tambah CUDA codegen full? | ❌ Tidak (Mojo/MLIR menang) |
| Tambah compile-time tensor shape check? | ✅ Ya (core niche) |
| Tambah bahasa fitur eksotis baru (linear types, modal types, dll)? | ⏸️ Bekukan kecuali ada use case langsung di niche |
| Tambah self-hosted compiler? | ⏸️ Bekukan sampai core stabil |
| Tambah formal proof / SMT verification? | ⏸️ Bekukan kecuali untuk niche safety-critical certification |
| Tambah package registry sendiri? | ❌ Tidak (pakai existing infra atau git deps) |
| Tambah debugger/DAP improvement? | ✅ Ya jika untuk debugging embedded target |
| Tambah quantization support (INT8, INT4)? | ✅ Ya (core niche untuk embedded ML) |
| Tambah ONNX import? | ✅ Ya (core niche untuk model deployment) |

## Lampiran C: Reference Materials

**PL Theory:**
- TAPL (Pierce) — fondasi type systems
- ATTAPL (Pierce, ed.) — advanced topics, dependent types, effects
- Hylo Language Tour — mutable value semantics
- Koka Manual (Daan Leijen) — algebraic effects implementation
- Idris 2 docs — dependent types in practice

**Compiler Engineering:**
- LLVM Tutorial Kaleidoscope
- Cranelift Documentation
- Crafting Interpreters (Robert Nystrom)
- Engineering a Compiler (Cooper & Torczon)

**Embedded:**
- ARM Cortex-M Programming (Joseph Yiu)
- ST Edge AI Documentation (STM32N6)
- Qualcomm AI Engine SDK / QNN
- TensorFlow Lite for Microcontrollers
- CMSIS-NN Documentation

**Comparable Projects:**
- Rust compiler (rust-lang/rust) — gold standard untuk multi-backend systems compiler
- Zig compiler (ziglang/zig) — simplicity-first systems language
- Mojo / Modular MAX — ML systems language
- Pony Language — actor-based systems
- Hylo / Val — value-semantics systems language

## Lampiran D: Glossary

- **Niche embedded AI safety**: Niche utama Fajar Lang. Embedded ML inference di MCU/NPU dengan compile-time domain safety.
- **Domain isolation**: Pemisahan `@kernel`/`@device`/`@safe`/`@unsafe` di level type system, dienforce oleh compiler.
- **Decision Framework**: 5 pertanyaan di Section 7 yang harus dijawab sebelum menambah fitur signifikan.
- **Honest reporting**: Pelaporan status yang jujur, tanpa inflasi atau hyperbole.
- **Ghost feature**: Fitur yang ada di codebase atau dokumentasi tapi tidak bekerja end-to-end.
- **Scaffolding bloat**: File boilerplate yang terlihat impressive tapi tidak ada substansi.
- **Single bet strategy**: Strategi memilih satu pertaruhan utama (embedded AI safety) dan tidak menyebar ke domain lain.

---

**Akhir Dokumen.**

*"Disiplin scope dan honesty adalah satu-satunya hal yang membedakan bahasa pemrograman yang sukses dari yang gagal di tahap eksperimental."*

— STRATEGIC_COMPASS.md v1.0, Mei 2026
— Disusun untuk Fajar Lang oleh kolaborasi Muhamad Fajar Putranto + Claude
