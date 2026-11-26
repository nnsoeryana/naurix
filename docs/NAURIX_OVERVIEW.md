# Naurix — Dokumen Lengkap

**Dibuat:** 23 November 2026, diperbarui 23 November 2026 (sore) setelah tahap 8 (workflow) dan tahap 9 (memori) diimplementasikan, dan sekali lagi untuk perbaikan terakhir (commit sampai `36f4431`, branch `feat/memory`)
**Sumber:** dokumen di `docs/` (PRD, Product Vision, Technical Design, Implementation Plan, ADR 0001–0030), `AGENTS.md`, `README.md`, serta isi kode di `crates/core`, `src-tauri`, `src`, dan konfigurasi proyek.
**Catatan:** dokumen ini merangkum keadaan proyek pada tanggal di atas. Bila berbeda dengan ADR atau PRD, yang berlaku adalah ADR dan PRD. Hal yang belum terverifikasi ditandai jelas.

---

## 1. Apa itu Naurix

Naurix adalah aplikasi desktop open-source yang **mengutamakan terminal** (*terminal-first*) untuk mengelola sesi shell biasa dan **CLI agent coding berbasis AI** dalam satu tempat.

Aplikasi ini menjalankan shell dan agent seperti Claude Code, Codex, Gemini CLI, Antigravity CLI, OpenCode, atau CLI buatan sendiri, berdampingan, per proyek.

- **Tagline:** *One workspace. Infinite possibilities.*
- **Ringkasan produk:** *One terminal. Multiple agents. Full control.*
- **Nama lama:** Agent Terminal (diganti menjadi Naurix, ADR 0013).
- **Lisensi:** Apache License 2.0.
- **Identitas aplikasi:** `dev.naurix.app`, versi saat ini `0.0.0` (pengembangan awal).
- **Website:** https://naurix.nana-dev.web.id/

Naurix **bukan** agent AI, bukan IDE, dan bukan layanan hosted. Semua berjalan di mesin pengguna.

### Masalah yang diselesaikan

Developer yang memakai beberapa agent coding sekaligus menghadapi kerja yang terpecah di banyak jendela terminal:

- tidak ada tampilan terpadu atas agent lintas proyek,
- sulit membedakan sesi yang berjalan, menunggu, selesai, atau gagal,
- konteks hilang saat jendela atau aplikasi ditutup,
- output agent, output shell, perubahan Git, dan status tugas terpisah-pisah,
- tiap agent punya konvensi yang berbeda.

---

## 2. Prinsip produk

1. **Terminal first.** Shell asli adalah fitur utama, bukan tambahan.
2. **AI-native, bukan AI-only.** Manajemen agent terintegrasi, tetapi terminal biasa tetap sempurna tanpanya.
3. **Kontrol pengguna.** Tidak ada aksi tersembunyi. Setiap aksi agent melewati satu approval gate pusat.
4. **Local-first.** Proyek, eksekusi proses, dan metadata sesi tetap di mesin pengguna. Tidak perlu akun atau cloud.
5. **Berbasis adapter.** Banyak agent didukung tanpa mengikat aplikasi ke satu vendor.
6. **Progressive enhancement.** Kemampuan terstruktur dipakai bila ada. Peluncuran lewat PTY/CLI tetap jalur cadangan universal.
7. **Status transparan.** Menampilkan apa yang berjalan, apa yang berubah, dan apa yang butuh perhatian.
8. **Core portabel.** Logika inti tidak terikat macOS.
9. **Ringan.** Performa diukur, tidak diklaim (ADR 0006).
10. **Agent-agnostic dan jujur soal kemampuan.** Kemampuan lanjutan (status, resume, eksekusi terstruktur) dipakai hanya setelah diverifikasi per agent dan versi.
11. **Pemisahan runtime.** Terminal Runtime (PTY, proses) terpisah dari Agent Runtime (adapter, approval).
12. **Memori eksplisit dan opt-in.** Catatan proyek hanya yang ditulis atau disetujui pengguna. Prompt, transkrip, dan output terminal tidak pernah disimpan otomatis.
13. **Sesi hidup lebih lama dari tampilan.** Sesi bertahan saat UI di-reload atau jendela ditutup selama proses Naurix hidup. Tidak ada daemon latar belakang.

### Target pengguna

- **Utama:** developer individu yang memakai agent coding setiap hari, pengguna tingkat lanjut lintas repositori, dan penyuka alur kerja keyboard.
- **Sekunder:** engineering lead yang meninjau kerja agent paralel, kontributor open-source adapter, tim yang ingin control plane desktop lokal.

Persona di PRD: **P1** developer berbantuan AI, **P2** power user terminal (akan menolak produk bila perilaku terminal dasar tidak andal), **P3** kontributor adapter.

---

## 3. Scope

### 3.1 Termasuk dalam scope

| Area | Isi |
|---|---|
| Terminal | Shell asli di PTY, warna ANSI, resize, paste, scrollback 5.000 baris, tab, split pane |
| Workspace | Membuka folder sebagai proyek, ganti, ubah nama, hapus (tidak menghapus file) |
| Agent | Peluncuran preset dan agent kustom, deteksi instalasi, daftar sesi agent, stop |
| MCP | Registry pusat, secret di credential store OS, approval, uji koneksi, sinkronisasi ke config agent |
| Skills | Penyimpanan pusat, instal dari Git/folder, aktif/nonaktif lewat symlink yang dapat dibatalkan |
| Git | Inspeksi hanya-baca (status, diff, worktree) dan worktree opt-in untuk agent |
| Orkestrasi (bertahap) | Sesi di Core, event bus, adapter, approval gate, batas agent paralel, worktree, sinyal status, workflow dan run |
| Memori proyek | Catatan yang ditulis pengguna sendiri, opt-in per workspace, dicari dengan FTS5, dapat diekspor |
| Tampilan | Command palette, 9 tema, companion animasi, pengaturan |

### 3.2 Di luar scope

Tetap di luar scope kecuali ada ADR baru yang menyetujui:

- eksekusi atau sinkronisasi di cloud, mesin remote, kolaborasi tim,
- IDE atau editor kode penuh,
- daemon atau supervisor latar belakang,
- penangkapan memori otomatis (prompt, transkrip, output),
- inferensi model bawaan dan pengelolaan API key penyedia LLM,
- marketplace MCP/skills publik (hanya katalog statis kecil dan konfigurasi manual),
- agent otonom yang bertindak tanpa persetujuan,
- persetujuan otomatis aksi agent,
- penyimpanan transkrip terminal permanen,
- jaminan memulihkan proses sembarang setelah aplikasi berhenti,
- telemetri (apa pun harus opt-in dan didokumentasikan).

### 3.3 Prioritas kebutuhan

PRD memakai tiga tingkat: **P0** wajib untuk MVP, **P1** bernilai dan boleh menyusul, **P2** masa depan. ID kebutuhan antara lain `WS-*` (workspace), `TERM-*` (terminal), `AG-*` (agent), `GIT-*`, `DATA-*`, `MCP-*`, `SK-*`, `UX-*`, serta `SES-*`, `ORCH-*`, `APR-*`, `WF-*`, `MEM-*` untuk orkestrasi.

---

## 4. Fitur yang tersedia saat ini

| Fitur | Keterangan |
|---|---|
| Shell asli | `$SHELL -l` di PTY, warna, resize, paste, scrollback 5.000 baris |
| Workspace | Buka folder, ganti, ubah nama, hapus; folder hilang ditangani dengan peringatan |
| Tab dan split | Buat, ganti nama, tutup, pindah; split kanan/bawah, seret untuk mengubah ukuran |
| Command palette | Cmd+K, plus pintasan keyboard |
| Prompt Nerd Font | Dukungan ikon dan powerline, font dapat diatur |
| Pemulihan setelah restart | Daftar workspace tetap ada; sesi lama tampil sebagai *interrupted* (proses tidak dipulihkan) |
| Konfirmasi tutup | Meminta konfirmasi sebelum menutup pane/tab yang masih menjalankan perintah |
| Peluncuran agent | Claude Code, Codex, Gemini CLI, Antigravity CLI, OpenCode, dan CLI kustom, dengan tinjauan perintah dan deteksi instalasi |
| Daftar sesi agent | Di sidebar, terpisah dari shell, dengan stop dan status berbasis proses |
| Kemampuan per agent | Daftar kemampuan, versi, dan pemberitahuan deprecation, masing-masing ditandai *verified* atau *documented only* |
| MCP Integrations | Katalog atau manual, secret di credential store OS, approval, uji koneksi, pratinjau/terapkan/batalkan sinkronisasi |
| Skills | Penyimpanan pusat, instal, aktif/nonaktif per agent dengan symlink |
| Git (hanya baca) | Branch, ahead/behind, file berubah, konflik, stash, diff, "buka di terminal" di root repo |
| Pengaturan | Umum, tampilan (9 tema, terang/gelap/sistem, aksen), terminal, agent, MCP, skills, performa, keamanan |
| Sesi bertahan | Reload UI dan tutup jendela tidak mengakhiri sesi (ADR 0026) |
| Approval gate | Peluncuran agent melalui satu gerbang persetujuan bersekop (ADR 0027) |
| Batas agent paralel | Default 4, rentang 1–16, hanya menghitung agent (ADR 0027) |
| Worktree opt-in | Agent dapat berjalan di Git worktree terpisah, disetujui sekali per aksi (ADR 0028) |
| Sinyal status | Status "menunggu" dan sejenisnya hanya dari sinyal agent yang terverifikasi, Claude Code dulu, opt-in (ADR 0029) |
| Companion | Kartu animasi aurora di sidebar yang hanya informatif |
| Workflows | Alur kerja yang dapat dipakai ulang: langkah berupa referensi ke agent, dependensi antar langkah, satu persetujuan untuk seluruh run, progres langsung (ADR 0030) |
| Memori proyek | Catatan opt-in per workspace, pencarian FTS5, penolakan otomatis untuk yang tampak seperti secret, ekspor Markdown/JSON, dan "sisipkan ke terminal agent" (ADR 0031) |

Belum ada: validasi Apple Silicon di perangkat nyata (M11a) dan penandatanganan serta notarization (M11b, butuh akun Apple Developer).

**Catatan status ADR 0030 dan 0031:** keduanya sudah **diimplementasikan** di kode, tetapi di indeks ADR masih bertanda *Proposed* menunggu maintainer menandainya *Accepted*. Setelah itu PRD dan `AGENTS.md` akan mencatatnya selesai. Pemeriksaan visual dan dengan agent nyata belum dilakukan (lihat bagian 16).

---

## 5. Arsitektur

### 5.1 Gambaran umum

```text
┌──────────────────────────────────────────────────────────┐
│                  React 19 + TypeScript                    │
│  Workspace · Terminal · Agent · Git · Palette · Settings  │
└───────────────────────────┬──────────────────────────────┘
                            │ Tauri commands / events / Channel
┌───────────────────────────▼──────────────────────────────┐
│                       Tauri 2 (src-tauri)                 │
│     jembatan: validasi input, capabilities, lifecycle     │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────┐
│                  crates/core  (tanpa Tauri)               │
│  session · pty · agents · approval · events · signals     │
│  git · mcp · skills · settings · credential · persistence │
└──────────┬─────────────────────────────────┬─────────────┘
           │                                 │
   PTY / shell / proses agent          SQLite (metadata)
                                       OS credential store
```

### 5.2 Aturan arsitektur

- Eksekusi proses dan pengelolaan PTY ada di **Rust**, bukan di webview.
- **Logika domain independen** dari React dan handler Tauri.
- `src-tauri` hanya **jembatan**. Siklus hidup sesi dan logika bisnis ada di `crates/core`. `src-tauri/src/terminal.rs` hanya menyesuaikan `SessionManager` ke Channel Tauri.
- Perintah dibangun dari **executable + array argumen**, tidak pernah string shell dari input pengguna.
- **Output terminal tidak masuk state global React.** Ditulis ke renderer lewat ref dan aliran event berbatas.
- Perbedaan OS ditempatkan di `crates/core/src/platform.rs` atau di balik `cfg`.
- Skema SQLite berubah lewat **migrasi**.
- Capability Tauri memakai prinsip hak akses minimum.

### 5.3 Pemisahan runtime (ADR 0022)

- **Terminal Runtime:** PTY, siklus hidup proses, flow control, dan jalur byte ke tampilan. Tidak tahu apa pun soal agent.
- **Agent Runtime:** adapter, sesi agent, approval, dan sinyal status terverifikasi. Berada di atas Terminal Runtime dan memintanya memulai atau menghentikan proses.
- Terminal harus tetap berfungsi penuh saat Agent Runtime absen.

### 5.4 Jalur data terminal

Rust menggabungkan output PTY (sekitar 16 ms atau hingga 64 KB per frame) dan mengirim **byte mentah** lewat `Channel` Tauri. Flow control berbasis ack dengan antrean berbatas sehingga banjir output (`yes`) tidak membuat memori tumbuh. Satu instance xterm.js per pane yang terlihat. Tab tersembunyi mempertahankan PTY dan melepas tampilannya. Tidak ada polling: refresh Git terjadi saat jendela fokus, sesi keluar, atau refresh eksplisit.

### 5.5 Sesi dan siklus hidup (ADR 0023, 0026)

- `SessionManager` di Core memiliki sesi. UI hanya *attach* dan *detach*.
- Output selalu dikuras ke **ring buffer di memori** (mulai 1 MiB per sesi, hipotesis yang perlu benchmark) agar proses tidak terblokir saat tidak ada tampilan. Buffer tidak pernah ditulis ke disk.
- Setelah reload, tiap sesi kembali sebagai satu tab (layout split tidak dipulihkan) dan di-*replay*.
- macOS: tombol tutup jendela **menyembunyikan** jendela, klik Dock membuka kembali, Cmd+Q keluar. Linux dan Windows: perilaku lama (menutup jendela terakhir berarti keluar) sampai divalidasi.
- Saat keluar, semua proses dihentikan bertahap (SIGHUP, SIGTERM, lalu SIGKILL setelah masa tenggang; di Windows lewat Job Object). Sesi yang pernah berjalan ditandai `interrupted` setelah restart.
- Sesi yang sudah keluar disimpan hingga batas tertentu (16) bila belum ditutup.

### 5.6 Approval gate (ADR 0027)

Satu gerbang di `crates/core/src/approval` menggantikan parameter `reviewed: bool` lama.

1. Pemanggil meminta otorisasi atas sebuah `Action` yang **dibangun backend** dari data yang sudah di-resolve.
2. Bila ada izin aktif yang cocok dengan **fingerprint** (SHA-256 atas kelas aksi, agent, path program absolut, argumen, dan environment tambahan yang terlihat), aksi diizinkan.
3. Bila tidak, gerbang mengembalikan permintaan berisi ringkasan buatan server. Permintaan sekali pakai dan kedaluwarsa dalam 5 menit.
4. UI hanya menjawab (tolak atau izinkan dengan sekop). UI tidak pernah menyuplai fingerprint atau ringkasan.
5. Fingerprint dihitung ulang saat dipakai, jadi target yang berubah tidak bisa menumpang izin lama.

**Sekop:** `once`, `workspace`, `app` (khusus peluncuran preset, terikat path executable persis), dan `run` (izin untuk satu eksekusi workflow, ADR 0030). Sekop `session` tidak ditambahkan karena belum ada yang memerlukannya. Tidak ada sekop "setujui semuanya".

| Kelas aksi | Sekop maksimum |
|---|---|
| `LaunchAgent` (preset) | app, untuk path executable persis |
| `LaunchAgent` (kustom) | workspace, terikat fingerprint penuh |
| `CreateWorktree`, `RemoveWorktree` | once |
| `RunWorkflow` | once (menghasilkan izin `run` untuk tiap peluncuran agent dalam run itu) |
| Tulis config agent | once (direncanakan, belum dipindah ke gerbang) |

Izin disimpan sebagai metadata (tabel `approvals`), dapat dicabut di Settings → Security, dibersihkan saat workspace dihapus, dan riwayat dibatasi 200 baris. `sessions.approval_id` mencatat izin mana yang mengizinkan peluncuran.

### 5.7 Adapter agent (ADR 0011, 0014, 0025)

- Peluncuran lewat **PTY generik** adalah jalur dasar universal untuk semua agent, termasuk CLI kustom.
- Setiap kemampuan (`PtyLaunch`, `McpConfig`, `Skills`, `StatusSignals`, `SessionResume`) punya **tingkat verifikasi**:
  - `Verified`: dikonfirmasi dengan menjalankan CLI asli (fixture) atau tes Naurix sendiri.
  - `DocumentedOnly`: ada di dokumentasi resmi, belum dikonfirmasi. Naurix tidak bertindak atas dasar ini.
  - `DocumentedAccepted`: belum dikonfirmasi, tetapi maintainer mengizinkan aksi yang dapat dibatalkan (misalnya symlink skill).
  - `Unsupported`: tidak ada dasar.
- Kemampuan tidak diturunkan dari kemampuan lain. MCP terverifikasi tidak berarti status, resume, atau eksekusi terstruktur terverifikasi.
- Perilaku per agent hidup di trait `AgentAdapter` (`crates/core/src/agents`).

**Dukungan agent saat ini** (`docs/AGENT_ADAPTERS.md`, dicatat 2026-11-19 di macOS 14.6 x86_64):

| Agent | Peluncuran PTY | MCP | Catatan |
|---|---|---|---|
| Claude Code | Ya | Verified (`.mcp.json` proyek) | Satu-satunya dengan sinyal status terverifikasi (ADR 0029) |
| OpenCode | Ya | Verified (bentuk datar `mcp.<name>`) | Dokumentasi dan biner terpasang berbeda, Naurix hanya bertindak bila file sudah memakai bentuk terverifikasi |
| Codex CLI | Ya | Documented only | CLI tidak terpasang di mesin pengembangan |
| Gemini CLI | Ya | Documented only | Dihentikan untuk sebagian besar akun individu mulai 18 Juni 2026 |
| Antigravity CLI (`agy`) | Ya | Documented only | Penerus Gemini CLI, ditambahkan sebagai preset |
| Kustom | Ya | — | Executable, argumen, environment tambahan, ditinjau sebelum diluncurkan |

### 5.8 Sinyal status (ADR 0029)

Status seperti "menunggu persetujuan" **hanya** ditampilkan dari sinyal yang dilaporkan agent sendiri lewat kemampuan adapter terverifikasi. Tidak pernah dari heuristik output (misalnya timer diam). Untuk Claude Code, hook disuntikkan **per peluncuran** lewat `--settings`, tanpa menulis ke konfigurasi pengguna. Hook tidak membaca prompt, jawaban, atau input tool. Kemampuan ini opt-in per agent dan mati secara default.

### 5.9 Worktree (ADR 0028)

- Opt-in per peluncuran, **mati secara default**.
- Perintah: `git worktree add -b <branch> <path> <base>` lewat runner Git yang diperkeras. Branch bernama `naurix/<agent-id>-<yyyymmdd-hhmm>`.
- Lokasi di folder data Naurix: `<app data>/worktrees/<workspace id>/<slug branch>`. Path tidak pernah datang dari webview.
- Hook repositori tidak dijalankan (`core.hooksPath=/dev/null`, `core.fsmonitor=false`). Repositori dengan filter lokal diblokir sampai dipercaya.
- File untracked atau di-ignore (`node_modules`, `.env`) **tidak disalin**, begitu juga submodule.
- Hapus tanpa `--force`, hanya worktree buatan Naurix, ditolak bila kotor, terkunci, atau sedang dipakai. **Branch selalu dipertahankan.**
- Worktree bukan workspace.
- Naurix tidak melakukan merge, rebase, push, atau hapus branch.

### 5.10 Event bus (ADR 0024)

Bus event domain di Core hanya membawa **metadata** (`SessionStarted`, `SessionExited`, `SessionStatus`, dan `RunChanged` dari workflow), terpisah dari jalur byte terminal. Pengiriman event sengaja boleh hilang, sehingga tampilan membaca ulang status saat dibuka dan saat jendela kembali fokus.

### 5.11 Workflow dan run (ADR 0030)

Alur kerja yang dapat dijalankan berkali-kali, tanpa mengubah prinsip kontrol pengguna.

- **Model:** *workflow* adalah templat bernama dalam satu workspace. *Langkah* (step) adalah "jalankan agent X" (preset atau agent kustom, termasuk agent kustom yang hanya berupa perintah seperti `npm test`). *Dependensi*: langkah B menunggu langkah A. **Tidak ada entitas Task terpisah**: langkah adalah tugasnya. *Run* adalah satu eksekusi workflow, dan menyalin langkah serta dependensi saat dimulai (snapshot), jadi mengubah workflow tidak memengaruhi run yang berjalan.
- **Batas validasi:** maksimal 24 langkah, 64 dependensi, nama 1–60 karakter, tanpa siklus (dicek di Core dengan topological sort). Workflow tidak dapat diubah atau dihapus saat ada run aktif.
- **Status langkah:** `pending`, `running`, `succeeded`, `failed`, `skipped`, `cancelled`. Status run: `running`, `succeeded`, `failed`, `cancelled`, `interrupted`.
- **Kapan langkah selesai:** saat prosesnya keluar (kode 0 berarti sukses, selain itu gagal), atau saat pengguna menekan **Mark done** atau **Skip**. Tidak ada heuristik output dan tidak ada sinyal status yang menyelesaikan langkah. Mark done tidak menghentikan agent.
- **Dependen** mulai bila semua dependensinya `succeeded` atau `skipped`. Langkah yang dependensinya gagal atau dibatalkan tetap `pending` dan ditampilkan sebagai *blocked*.
- **Persetujuan:** satu persetujuan `RunWorkflow` (sekali pakai) yang meninjau **perintah persis tiap langkah** dan dependensinya. Setelah disetujui, Core membuat izin ber-sekop `run` untuk tiap peluncuran agent berbeda dalam run itu. Izin ini hanya cocok untuk peluncuran oleh run tersebut, dicabut saat run berakhir, dan bila ada peluncuran yang belum tercakup (misalnya path program berubah), langkah itu gagal dengan catatan `approval missing`. Tidak pernah bertanya di tengah run.
- **Langkah hanya referensi ke agent.** Tidak ada prompt tersimpan dan tidak ada input otomatis. Langkah yang butuh argumen tetap memakai agent kustom.
- **Eksekusi:** engine di Core tidak memiliki proses. Ia meminta `SessionManager` memulai sesi lewat launcher dari shell (jalur peluncuran agent yang sama, jadi Terminal Runtime tetap tidak tahu soal workflow). Sesi langkah berjalan *detached*, dan tombol **Open terminal** memasangnya ke pane. Beberapa langkah independen dapat berjalan bersamaan, tunduk pada batas agent. Langkah yang kehabisan slot tetap `pending` dan dicoba lagi saat sesi lain berakhir.
- **Restart:** run yang berjalan ditandai `interrupted` dan langkahnya `failed` dengan catatan `interrupted`. Tidak ada yang dilanjutkan otomatis.
- **Batas yang sengaja ada:** tanpa worktree dalam workflow (Git mutation tidak dapat disetujui di muka), tanpa kondisi, loop, retry, jadwal, pemicu, atau variabel, dan tanpa tipe langkah perintah shell.

### 5.12 Memori proyek (ADR 0031)

Catatan yang ditulis pengguna sendiri, bukan hasil tangkapan otomatis.

- **Catatan:** judul 1–100 karakter dan isi hingga 8.000 karakter, maksimal 200 catatan per workspace. Tanpa tag, lampiran, atau tautan ke file atau sesi.
- **Opt-in per workspace.** Memori mati untuk setiap workspace sampai pengguna menyalakannya. Saat mati, catatan lama disimpan tetapi disembunyikan, dengan tombol **Delete all notes** agar tidak ada yang tersisa tanpa jalan untuk menghapus. Menghapus workspace menghapus catatannya.
- **Penyimpanan:** SQLite dengan tabel FTS5 (`memory_fts`, tokenizer `unicode61` dengan penghilangan diakritik) dan trigger yang menjaga indeks sinkron. FTS5 sudah terkompilasi di SQLite bawaan (3.54.1), sehingga tanpa dependensi baru. Menghapus catatan, semua catatan, atau workspace juga menghapus baris indeksnya.
- **Secret ditolak tanpa opsi override.** Pemindai mencari header private key, AWS key id, token GitHub (`ghp_`, `github_pat_`), key bergaya `sk-`, token Slack, key Google API, JWT, serta penetapan `nama = nilai` untuk nama yang menyiratkan rahasia dengan nilai panjang yang tampak acak. Penolakan menyebut **baris dan jenisnya** tetapi tidak pernah mengulang nilainya. Ini jaring pengaman, bukan jaminan, dan UI menyatakannya.
- **Pencarian:** kueri pengguna tidak pernah diteruskan ke FTS5 apa adanya. Ia dipecah menjadi kata dan tiap kata dikutip sebagai awalan, sehingga operator FTS5 tidak berlaku. Hasil diurutkan relevansi dan dibatasi 50.
- **Pengiriman ke agent:** tombol **Insert into agent terminal** mengetik teks catatan ke pane aktif **bila pane itu sesi agent**, lewat paste terminal, **tanpa menekan Enter**. Ditolak untuk pane shell biasa. Catatan multibaris hanya disisipkan bila agent mengaktifkan bracketed paste. Tidak ada file agent yang ditulis. Menulis ke file memori milik agent (CLAUDE.md dan sejenisnya) belum dibuat dan butuh ADR sendiri.
- **Ekspor:** Markdown dan JSON. Backend membuka dialog simpan sendiri, jadi tujuan tidak pernah dari webview. Bila tujuan berada di dalam repositori Git, diminta konfirmasi kedua yang menyebut risikonya. Naurix tidak pernah menjalankan `git add` atau `git commit`.
- **Hanya pengguna yang membuat catatan.** Tidak ada jalur bagi agent untuk menulis catatan (tanpa tool MCP, hook, atau tangkapan output).
- **Privasi di kode:** isi catatan tidak dicatat di log, tidak masuk pesan error, dan tidak disertakan dalam event.

---

## 6. Teknologi

### 6.1 Ringkasan stack

| Lapisan | Teknologi |
|---|---|
| Shell desktop | Tauri 2 |
| Backend | Rust (edisi 2021, toolchain stable, target `x86_64-apple-darwin` dan `aarch64-apple-darwin`) |
| Frontend | React 19, TypeScript 7 |
| Renderer terminal | xterm.js 6 (dengan addon fit dan WebGL) |
| PTY | `portable-pty` 0.9 |
| Database | SQLite lewat `rusqlite` 0.43 (fitur `bundled`) |
| Secret | macOS Keychain (`security-framework` 3), Secret Service di Linux, Credential Manager di Windows (`keyring` 3) |
| Git | CLI `git` (ADR 0005) |
| State frontend | Zustand 5 |
| Styling | Tailwind CSS 4 |
| Bundler | Vite 8 |
| Tes | Tes Rust (`cargo test`), Vitest 5 |
| Paket | pnpm, Node ≥ 22 |
| Lisensi | Apache-2.0 |

### 6.2 Dependensi Rust (`crates/core`)

`portable-pty`, `thiserror`, `rusqlite` (bundled), `serde`, `serde_json` (preserve_order), `sha2`, `libc` (Unix), `security-framework` (macOS), `keyring` (Linux dan Windows). Aplikasi Tauri menambah `tauri`, `tauri-plugin-dialog`, dan `arboard` (clipboard Linux dan Windows, macOS memakai `pbcopy`/`pbpaste`).

### 6.3 Dependensi frontend

Runtime: `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl`, `react`, `react-dom`, `zustand`.

### 6.4 Profil rilis

`opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.

### 6.5 Website

Folder `website/` memakai Next.js 16 dan React 19 (terpisah dari aplikasi desktop).

### 6.6 Keputusan strategi renderer

Hanya xterm.js yang diimplementasikan. Antarmuka `TerminalView` dibuat agar renderer dapat diganti. libghostty hanya akan dipertimbangkan setelah spike membuktikan kelayakan integrasi, kompatibilitas lisensi, kemudahan pemeliharaan, dan manfaat yang terukur (TERM-014, P2, Spike B).

---

## 7. Struktur repositori

```text
naurix/
├── AGENTS.md            instruksi untuk coding agent (invarian, aturan teknik)
├── README.md, CONTRIBUTING.md, SECURITY.md, LICENSE
├── Cargo.toml           workspace Rust: crates/core + src-tauri
├── package.json         frontend, pnpm
├── run.sh               pembantu pengembangan (doctor, app, dmg, sandbox, test)
├── crates/core/         logika domain, tanpa dependensi Tauri
├── src-tauri/           jembatan Tauri (command handler, events, capability)
├── src/                 UI React
├── bench/               harness benchmark RSS/CPU dan hasil rekaman
├── website/             situs Next.js
├── docs/                PRD, visi, desain teknis, rencana, ADR
├── assets/, scripts/
└── .github/workflows/ci.yml
```

### Modul inti (`crates/core/src`)

| Modul | Tanggung jawab |
|---|---|
| `pty/` | Terminal Runtime: PTY, forwarder, flow control |
| `session/` | `SessionManager`: memulai, attach/detach, close, list, batas agent |
| `agents/` | Definisi agent (`def.rs`), adapter, perilaku, penyimpanan, pencarian executable, versi |
| `approval/` | Approval gate, fingerprint, sekop, riwayat |
| `events.rs` | Event bus domain |
| `signals.rs` | Sinyal status agent terverifikasi |
| `workflow/` | Model dan validasi workflow, penyimpanan, dan engine yang memulai langkah dari status proses |
| `memory/` | Catatan memori, pencarian FTS5, ekspor, pengecekan repositori, dan pemindai secret (`secrets.rs`) |
| `git/` | Runner Git yang diperkeras, status, diff, worktree |
| `mcp/`, `skills/` | Registry, approval, sinkronisasi ke config agent |
| `persistence/` | SQLite, migrasi, workspace |
| `credential.rs` | Akses credential store OS |
| `settings.rs` | Pengaturan aplikasi |
| `platform.rs` | Perbedaan OS |

### Modul jembatan (`src-tauri/src`)

`agent`, `approval`, `clipboard`, `events`, `git`, `lifecycle`, `mcp`, `memory`, `settings`, `skills`, `terminal`, `workflow`, `workspace`, `worktree`.

### Fitur UI (`src/features`)

`agents`, `appearance`, `approvals`, `git`, `home`, `layout`, `mcp`, `memory`, `naurix` (companion), `palette`, `settings`, `skills`, `tabs`, `terminal`, `workflows`, `workspaces`, `worktrees`. State global di `src/state/store.ts`, tema di `src/themes/`, harness smoke di `src/dev/smoke.ts`.

---

## 8. Model data

SQLite menyimpan **metadata saja**. Tabel pada migrasi saat ini:

| Tabel | Isi |
|---|---|
| `workspaces` | Id, nama, path, waktu |
| `sessions` | Identitas, jenis, judul, workspace, waktu, status terakhir, `approval_id`, `worktree_id` |
| `settings` | Pasangan kunci-nilai (termasuk batas agent, kepercayaan repo Git) |
| `mcp_servers` | Definisi server dan fingerprint persetujuan |
| `mcp_sync_entries`, `mcp_sync_runs` | Catatan sinkronisasi agar dapat dibatalkan |
| `skills`, `skill_links` | Skill terpasang dan symlink yang dibuat |
| `approvals` | Kelas, sekop, fingerprint, ringkasan (tanpa argumen atau nilai environment), status |
| `worktrees` | Worktree yang dibuat Naurix (path, branch, commit dasar) |
| `workflows`, `workflow_steps`, `workflow_deps` | Templat workflow, langkah (hanya referensi `agent_id`), dan dependensi |
| `runs`, `run_steps`, `run_deps` | Eksekusi workflow sebagai snapshot, status tiap langkah, kode keluar, dan catatan berkosakata tetap (misalnya `agent not found`, `approval missing`), tidak pernah output proses |
| `memory_notes` | Catatan memori opt-in (judul, isi, waktu) per workspace |
| `memory_fts` | Tabel virtual FTS5 untuk pencarian catatan |

Kolom `workspaces.memory_enabled` (bawaan 0) menyimpan opt-in memori. Migrasi bersifat *append-only*: migrasi 5 menambah worktree, 6 workflow dan run, 7 memori.

**Tidak disimpan:** input atau output terminal, prompt, nilai environment, isi source. Satu-satunya teks pengguna di luar metadata adalah **catatan memori** yang diketik sendiri oleh pengguna dan di-opt-in per workspace (ADR 0022, 0031). Nama workflow dan langkah juga teks yang diketik pengguna, dibatasi panjangnya dan dibersihkan dari karakter kontrol.

---

## 9. Keamanan dan privasi

- **Secret hanya di credential store OS.** Tidak pernah di SQLite, log, atau webview setelah disimpan. Naurix tidak menyimpan kredensial penyedia LLM (ADR 0019).
- **Server MCP tidak dijalankan sebelum disetujui**, termasuk saat uji koneksi.
- **Skill tidak pernah dieksekusi.** Isinya dianggap tidak tepercaya.
- **Tidak menimpa file milik pengguna.** Sinkronisasi MCP/skill melalui dry-run, persetujuan, cadangan, dan dapat dibatalkan.
- **Git diperkeras terhadap repositori berbahaya** (ADR 0017): hook dinonaktifkan, `fsmonitor` mati, `diff.external` dan `credential.helper` dikosongkan, lingkungan dibersihkan, tanpa prompt terminal, output dan waktu dibatasi.
- **Konten proyek dan output agent dianggap tidak tepercaya.**
- **Catatan memori ditolak bila tampak seperti secret**, tanpa opsi "simpan saja". Pemindai bersifat heuristik dan hanya jaring pengaman.
- **Memori tidak pernah dikirim ke agent secara diam-diam** dan tidak pernah ditulis ke repositori atau di-commit otomatis.
- **Workflow tidak mengotomatiskan input.** Langkah tidak menyimpan prompt, dan izin run dicabut saat run berakhir.
- **Tidak ada telemetri.**
- **Tidak ada klaim platform atau performa tanpa rekaman nyata.** CI saja bukan bukti.
- Pelaporan kerentanan lewat GitHub Security Advisories secara privat (`SECURITY.md`).

---

## 10. Performa dan kompatibilitas

### Target (hipotesis, ADR 0006)

| Skenario | Target sementara |
|---|---|
| Idle, tanpa terminal | ≤ 120 MB |
| Satu sesi shell | ≤ 180 MB |
| 4 / 8 terminal | Belum ada target, pertumbuhan per terminal dicatat |
| Scrollback bawaan | 5.000 baris |

"RSS aplikasi" berarti proses utama ditambah proses helper WebKit milik aplikasi, tanpa shell atau agent yang diluncurkan pengguna.

### Bukti yang ada

Hasil benchmark terekam di `bench/results/` untuk **x86_64** pada 2026-11-19: idle (blink on/off), 1/4/8 terminal, banjir output (DOM dan WebGL), 4 agent mock, dan campuran 30 menit. **Belum ada** hasil arm64, Linux, atau Windows. Angka tidak boleh diklaim untuk platform tersebut sebelum ada rekaman.

### Matriks platform

| Platform | Status |
|---|---|
| macOS (Intel dan Apple Silicon) | Didukung. Validasi Apple Silicon di perangkat nyata masih menunggu (M11a) |
| Linux | Kode selesai, **belum divalidasi** (ADR 0021) |
| Windows | Kode selesai, **belum divalidasi** (ADR 0021) |

CI membangun dan menjalankan tes otomatis di macOS arm64, Ubuntu 24.04, dan Windows. Itu bukan pengganti checklist manual dan benchmark.

Catatan lintas platform: pintasan memakai Cmd di macOS dan Ctrl+Shift di Linux dan Windows agar Ctrl+C/W/D tetap untuk shell. Symlink skill di Windows butuh Developer Mode. Lokasi config agent baru diverifikasi di macOS.

---

## 11. Tema dan companion

### Sistem tema (ADR 0020)

Satu set token semantik dipakai untuk seluruh aplikasi dan terminal xterm.js. Sembilan tema bawaan, mode terang/gelap/sistem, dan aksen kustom. Tema dicache agar tidak berkedip saat dimuat. Komponen tidak menerima tema lewat props.

### Companion

Kartu animasi aurora di dasar sidebar, hanya informatif: tanpa kontrol, tidak mengambil fokus, tidak pernah memulai, menghentikan, atau menyetujui apa pun.

| Status | Tampilan |
|---|---|
| Idle | Satu pita lembut |
| Ready | Pita sama, lebih lambat (agent hidup tetapi diam) |
| Working | Lima pita mengalir |
| Waiting | Pita tipis (hanya bila ada sinyal adapter yang andal) |
| Success | Satu ledakan sekitar 2,4 detik |
| Error | Pita redup dengan garis peringatan |

Status diturunkan dari fakta **proses**, bukan tebakan. Teks "Waiting for your agent's response..." adalah heuristik yang ditandai demikian. Hanya `transform` dan `opacity` yang dianimasikan, dan animasi berhenti saat jendela tersembunyi.

---

## 12. Peta jalan orkestrasi (ADR 0022)

Urutan tahap tetap, masing-masing meninggalkan aplikasi tetap bisa dijalankan dan tes terminal tetap hijau:

| Tahap | Isi | ADR | Status |
|---|---|---|---|
| 1 | Kepemilikan sesi pindah ke Core, satu id sesi stabil | 0023 | Selesai |
| 2 | Event bus | 0024 | Selesai |
| 3 | Trait `AgentAdapter`, tanpa perubahan perilaku | 0025 | Selesai |
| 4 | Sesi independen dari webview dan jendela | 0026 | Selesai (cek manual macOS menunggu) |
| 5 | Approval gate dan batas agent (Task/Run dipindah ke tahap 8) | 0027 | Selesai (cek visual dan agent nyata menunggu) |
| 6 | Isolasi worktree opt-in | 0028 | Selesai (cek visual dan agent nyata menunggu) |
| 7 | Spike sinyal terstruktur per agent | 0029 | Selesai untuk Claude Code (jalur penolakan interaktif dan platform lain belum terverifikasi) |
| 8 | Workflow, termasuk Task/Run | 0030 | Diimplementasikan (cek visual dan agent nyata menunggu; ADR masih *Proposed* menunggu maintainer) |
| 9 | Memori | 0031 | Diimplementasikan (cek dialog ekspor dan agent nyata menunggu; ADR masih *Proposed* menunggu maintainer) |

Dengan ini seluruh sembilan tahap di ADR 0022 sudah memiliki implementasi. Tahap hanya dikerjakan bila maintainer memintanya dan ADR lanjutannya ditulis lebih dulu.

### Keputusan maintainer untuk tahap 8 (ADR 0030)

Satu persetujuan untuk seluruh run (izin terikat fingerprint workflow dan berakhir bersama run). Sebuah langkah selesai saat prosesnya keluar (0 sukses, selain itu gagal) atau saat pengguna menekan *Mark done* atau *Skip*. Langkah hanya berupa **referensi ke agent**, tanpa menyimpan prompt dan tanpa input otomatis.

Perbedaan implementasi dari desain: *Cancel* juga membuat langkah yang sedang berjalan menjadi `cancelled` (sesinya dibiarkan hidup); batas agent yang penuh membuat langkah tetap `pending` dan dicoba lagi, bukan gagal; exit yang terlewat oleh bus event dikejar saat run dibaca (`reconcile`); editor hanya menawarkan langkah sebelumnya sebagai dependensi sehingga siklus tidak dapat digambar.

### Keputusan maintainer untuk tahap 9 (ADR 0031)

1. Pengiriman ke agent berupa **sisipkan ke terminal agent tanpa Enter**, tidak ada file agent yang ditulis.
2. Dugaan secret **ditolak tanpa override**.
3. **Hanya pengguna** yang membuat catatan, lewat UI Naurix.

Temuan saat dijalankan: program tanpa bracketed paste (misalnya `cat`) menganggap tiap baris tempel sebagai Enter, sehingga sebagian catatan terkirim sebelum pengguna melihatnya. Karena itu catatan multibaris sekarang ditolak untuk kasus itu dan hanya catatan satu baris yang selalu boleh.

---

### Perbaikan terbaru setelah tahap 8 dan 9

| Commit | Perbaikan |
|---|---|
| `2627e49` | Langkah workflow yang tertahan batas agent kini dimulai saat sesi apa pun berakhir |
| `9474b62` | UI: balasan pencarian memori yang sudah usang dibuang, draf catatan dan workflow disimpan per proyek, dan status proses yang sudah berakhir tidak lagi ditampilkan |
| `8ed6513` | Tanda "disetujui" dan pengecekan versi agent kini memperhitungkan peluncuran sebenarnya, termasuk argumen hook status (ADR 0029). Pengecekan itu kini menyertakan argumen tambahan tersebut dalam fingerprint. README mendokumentasikan mode smoke test (`reload`, `limit`, `worktree`, `status`, `workflow`, `memory`) |
| `36f4431` | PTY: anak proses yang langsung keluar tidak lagi tertinggal terdaftar selamanya. Ada balapan antara `spawn` dan callback exit, kini registry dikunci selama spawn. Tes: `supervisor_race.rs` |

Dua perbaikan terakhir menyentuh jalur yang dipakai setiap peluncuran (approval dan PTY), jadi layak dijalankan ulang di mesin sendiri sebelum rilis (`./run.sh test`).

## 13. Milestone implementasi MVP

| Milestone | Cakupan | Status |
|---|---|---|
| M0 | Kebersihan repo dan dokumen | Selesai |
| M1 | Scaffold Tauri 2 + React/TS, CI, harness bench | Selesai |
| M2 | PTY dan supervisor | Selesai |
| M3 | Tampilan xterm.js, batching, flow control | Selesai |
| M4 | SQLite, workspace, tab, split, palette, semantik restart | Selesai |
| M5 | Peluncuran agent, preset, agent kustom | Selesai |
| M6 | Lapisan adapter dan matriks kemampuan | Selesai |
| M7 | MCP manager | Selesai |
| M8 | Skills manager | Selesai |
| M9 | Status Git | Selesai |
| M10 | UI Settings dan penyetelan performa | Selesai |
| M11a | Validasi rilis di Intel dan Apple Silicon | Menunggu perangkat Apple Silicon |
| M11b | Penandatanganan dan notarization | Tertahan, butuh akun Apple Developer |
| M12 | Kesiapan Linux | Kode selesai, belum divalidasi |
| M13 | Kesiapan Windows | Kode selesai, belum divalidasi |

Spike yang direncanakan: A (xterm.js + PTY), D (pembersihan proses), C (perilaku CLI agent nyata), B (libghostty, sebagai catatan keputusan setelah M10).

---

## 14. Daftar ADR

Sebuah ADR tidak diubah setelah *Accepted*. Perubahan keputusan dibuat lewat ADR baru yang menggantikannya.

| ADR | Judul | Status |
|---|---|---|
| 0001 | Scope platform: macOS x86_64 + arm64 | Accepted (klausul Windows/Linux digantikan 0021) |
| 0002 | Crate `core` independen + shell Tauri tipis | Accepted |
| 0003 | Registry pusat + adapter sinkronisasi per agent | Accepted |
| 0004 | Secret hanya di Keychain | Accepted |
| 0005 | Integrasi Git lewat CLI | Accepted |
| 0006 | Anggaran performa adalah hipotesis sampai diukur | Accepted |
| 0007 | Klarifikasi scope | Accepted |
| 0008 | Crate PTY dan siklus hidup proses | Accepted |
| 0009 | Jalur data terminal dan flow control | Accepted |
| 0010 | Workspace, tab, pane, dan persistensi | Accepted |
| 0011 | Peluncuran agent lewat PTY generik | Accepted |
| 0012 | Font dan tema terminal | Accepted |
| 0013 | Ganti nama menjadi Naurix dan merek Aurora | Accepted |
| 0014 | Fakta adapter membawa tingkat verifikasi | Accepted |
| 0015 | MCP manager | Accepted |
| 0016 | Skills manager | Accepted |
| 0017 | Status Git hanya-baca, diperkeras | Accepted |
| 0018 | Model pengaturan dan anggaran performa | Accepted |
| 0019 | Naurix tidak menyimpan kredensial | Accepted (sebagian diimplementasikan) |
| 0020 | Sistem tema | Accepted |
| 0021 | Kesiapan Windows dan Linux | Accepted (diimplementasikan, belum divalidasi) |
| 0022 | Scope workspace orkestrasi, siklus hidup sesi, memori proyek | Accepted |
| 0023 | Kepemilikan sesi di Core | Accepted |
| 0024 | Event bus domain | Accepted |
| 0025 | Trait `AgentAdapter` | Accepted |
| 0026 | Sesi hidup lebih lama dari tampilan | Accepted (diimplementasikan; cek manual macOS menunggu) |
| 0027 | Approval gate pusat | Accepted (diimplementasikan; cek visual dan agent nyata menunggu) |
| 0028 | Isolasi Git worktree opt-in | Accepted (diimplementasikan; cek visual dan agent nyata menunggu) |
| 0029 | Sinyal status terverifikasi, Claude Code dulu | Accepted (diimplementasikan untuk Claude Code) |
| 0030 | Workflow dan run yang dapat dipakai ulang | Proposed (diimplementasikan; cek visual dan agent nyata menunggu) |
| 0031 | Memori proyek opt-in | Proposed (diimplementasikan; dialog ekspor dan cek agent nyata menunggu) |

---

## 15. Pengembangan

### Prasyarat

Xcode command line tools (macOS), Rust (stable), Node 22+, pnpm. Jalankan `./run.sh doctor` untuk memeriksa dan mendapatkan perintah instalasi yang kurang.

### Perintah

| Perintah | Fungsi |
|---|---|
| `./run.sh` | Mode pengembangan (hot reload) |
| `./run.sh app` | Bangun `Naurix.app` dan buka |
| `./run.sh install` | Bangun dan salin ke `~/Applications` |
| `./run.sh dmg` | Bangun installer `.dmg` |
| `./run.sh sandbox` | Salinan kedua dengan data sekali pakai (folder data dan port terpisah) |
| `./run.sh test` | Semua tes dan lint |
| `./run.sh doctor` | Periksa lingkungan |
| `pnpm dev` / `build` / `typecheck` / `test` | Skrip frontend |

Linux membutuhkan prasyarat Tauri (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, dan lainnya) dan bundle `.deb`, `.rpm`, `.AppImage`. Windows membutuhkan toolchain MSVC, WebView2, dan menghasilkan `.msi` atau NSIS `.exe`.

### Alur kerja kontribusi (`AGENTS.md`)

1. Tentukan ID kebutuhan PRD yang relevan.
2. Periksa kode yang ada dan ikuti pola yang berlaku.
3. Usulkan rencana implementasi kecil.
4. Implementasikan irisan vertikal lengkap terkecil.
5. Tambah atau perbarui tes.
6. Jalankan format, lint, type check, dan tes.
7. Laporkan file yang berubah, perilaku, tes yang dijalankan, batasan, dan risiko lanjutan.

Penyelesaian (*definition of done*): kriteria penerimaan terpenuhi, tes mencakup perilaku normal dan jalur gagal penting, tidak ada secret atau isi terminal yang dicatat atau disimpan, error dapat ditindaklanjuti dan tidak menjatuhkan aplikasi, dokumentasi diperbarui.

### Tes

Tes Rust di `crates/core/tests` mencakup adapter, agent, approval, event, Git, MCP, PTY (termasuk Windows), sesi (reattach dan batas), sinyal status, skill, workflow, memori (FTS5, pemindai secret, kueri berbahaya, dan penghapusan indeks), kondisi balapan supervisor, dan worktree (termasuk repositori berbahaya dengan hook `post-checkout` dan `fsmonitor`). Tes frontend memakai Vitest, dan `src/dev/smoke.ts` adalah harness smoke untuk skenario seperti reload dan approval.

---

## 16. Batasan dan hal yang belum terverifikasi

- **Apple Silicon** belum divalidasi di perangkat nyata. **Linux dan Windows** belum dijalankan di perangkat nyata.
- **Penandatanganan dan notarization** belum ada (butuh akun Apple Developer).
- **Replay setelah reload** bisa salah merender TUI penuh layar bila potongan jatuh di tengah escape sequence. Mitigasinya hanya resize ukuran PTY agar TUI menggambar ulang. Perlu dicoba dengan tiap CLI agent nyata.
- **Ukuran ring buffer 1 MiB** masih hipotesis tanpa benchmark.
- **Klaim "tidak ada proses yatim setelah aplikasi di-kill"** belum boleh ditulis sebagai jaminan sebelum ada tes setara Spike D.
- **Hanya Claude Code dan OpenCode** yang MCP-nya terverifikasi. Hanya Claude Code yang sinyal statusnya terverifikasi.
- **Workflow (ADR 0030):** tampilan Workflows hanya diuji lewat skrip yang menekan tombol dan membaca teks, belum dilihat manusia. Belum ada agent CLI nyata yang dijalankan sebagai langkah (hanya perintah `sh`), langkah dengan status hook aktif belum dicoba, restart saat run aktif baru diuji di Core, dan beberapa run bersamaan atau folder workspace yang hilang belum dicoba di aplikasi. "Sukses" untuk agent interaktif adalah kata pengguna (*Mark done*). Kode 0 hanya berarti proses selesai bersih, bukan hasil kerjanya benar.
- **Memori (ADR 0031):** ekspor lewat dialog simpan native (dan konfirmasi kedua di dalam repositori Git) belum diuji di aplikasi, hanya isinya di Core. Tampilan Memory belum dilihat manusia. Belum dicoba dengan agent CLI nyata, jadi belum diketahui apakah masing-masing mengaktifkan bracketed paste. Pemindai secret heuristik dan hanya diuji pada bentuk token yang dikenal, sehingga penolakan keliru maupun yang terlewat mungkin terjadi. Impor catatan belum ada.
- **Approval MCP, sinkronisasi, dan kepercayaan repo Git** belum dipindah ke gerbang pusat. Masing-masing masih mekanisme sendiri.
- **Fingerprint tidak menghash isi biner executable.** Biner yang diperbarui di path yang sama tetap dianggap disetujui. Ini dinyatakan di teks tinjauan.
- **Worktree** tidak menyalin file untracked atau ter-ignore dan tidak menginisialisasi submodule.
- Tidak ada pembacaan memori Windows di Settings.
- Beberapa pemeriksaan visual dan dengan agent nyata di ADR 0026–0028 tercatat sebagai belum terverifikasi.

---

## 17. Ringkasan satu halaman

| Pertanyaan | Jawaban |
|---|---|
| Apa itu? | Workspace desktop terminal-first untuk shell dan CLI agent coding AI |
| Untuk siapa? | Developer yang memakai beberapa agent coding setiap hari |
| Apa yang membedakan? | Terminal kelas satu, kontrol penuh lewat approval gate, privasi metadata-first, ringan |
| Teknologi? | Tauri 2, Rust, React 19, TypeScript, xterm.js, SQLite |
| Data pengguna? | Hanya metadata di mesin lokal. Secret di credential store OS |
| Agent didukung? | Claude Code, Codex, Gemini CLI, Antigravity CLI, OpenCode, dan CLI kustom lewat PTY |
| Platform? | macOS didukung. Linux dan Windows kode selesai tetapi belum divalidasi |
| Posisi sekarang? | MVP M0–M10 selesai, orkestrasi tahap 1–9 sudah diimplementasikan (workflow dan memori menunggu penandaan *Accepted* dan pemeriksaan visual) |
| Bukan apa? | Bukan agent AI, bukan IDE, bukan layanan cloud, tanpa telemetri dan marketplace publik |
| Lisensi? | Apache-2.0 |
