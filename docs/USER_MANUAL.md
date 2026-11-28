# Naurix — Panduan Pengguna

**Versi dokumen:** 23 November 2026 (sesuai kode sampai commit `36f4431`)
**Untuk:** developer yang memakai terminal dan CLI agent coding AI (Claude Code, Codex, Gemini CLI, Antigravity CLI, OpenCode, atau CLI buatan sendiri).

Panduan ini menjelaskan cara memakai Naurix dari awal. Teks di antara tanda kutip seperti **Open folder…** adalah tulisan asli pada tombol atau menu di aplikasi, yang berbahasa Inggris.

> **Penting tentang status fitur.** Fitur Workflows dan Memory baru diuji lewat skrip, belum pernah dilihat atau dicoba manusia di layar. Peluncuran agent dengan agent CLI nyata juga baru dicoba terbatas. Bagian yang terkait ditandai ⚠️. Linux dan Windows: kodenya ada tetapi belum divalidasi di perangkat nyata.

---

## Daftar isi

1. [Apa itu Naurix](#1-apa-itu-naurix)
2. [Memasang dan menjalankan](#2-memasang-dan-menjalankan)
3. [Langkah pertama](#3-langkah-pertama)
4. [Tampilan utama](#4-tampilan-utama)
5. [Terminal, tab, dan split](#5-terminal-tab-dan-split)
6. [Menjalankan agent AI](#6-menjalankan-agent-ai)
7. [Menjalankan agent di worktree terpisah](#7-menjalankan-agent-di-worktree-terpisah)
8. [Melihat apa yang sedang dikerjakan agent](#8-melihat-apa-yang-sedang-dikerjakan-agent)
9. [Approval dan keamanan](#9-approval-dan-keamanan)
10. [Workflows](#10-workflows)
11. [Memory (catatan proyek)](#11-memory-catatan-proyek)
12. [Security (laporan keamanan)](#12-security-laporan-keamanan)
13. [MCP Integrations](#13-mcp-integrations)
14. [Agent Skills](#14-agent-skills)
15. [Git](#15-git)
16. [Pengaturan](#16-pengaturan)
17. [Pintasan keyboard](#17-pintasan-keyboard)
18. [Data, privasi, dan reset](#18-data-privasi-dan-reset)
19. [Pemecahan masalah](#19-pemecahan-masalah)
20. [Pertanyaan umum](#20-pertanyaan-umum)

---

## 1. Apa itu Naurix

Naurix adalah aplikasi desktop untuk menjalankan **shell biasa** dan **CLI agent AI** berdampingan, per proyek, dalam satu jendela.

Yang perlu kamu ketahui sejak awal:

- Naurix **bukan** agent AI, bukan editor kode, dan bukan layanan cloud. Ia menjalankan CLI yang sudah kamu pasang.
- Semua berjalan di komputermu. Tidak perlu akun. **Tidak ada telemetri.**
- Terminal adalah fitur utama. Workflows, Memory, dan lainnya bersifat tambahan. Kamu bisa memakai Naurix sebagai terminal biasa tanpa menyentuhnya.
- Naurix **tidak menjalankan apa pun diam-diam**. Peluncuran agent, perubahan Git, dan penulisan konfigurasi selalu lewat persetujuanmu.
- Naurix **tidak menyimpan** isi terminal, prompt, atau nilai environment. Yang tersimpan hanya metadata (nama proyek, judul sesi, pengaturan).

---

## 2. Memasang dan menjalankan

Naurix saat ini dipakai dengan membangunnya dari sumber. Belum ada installer yang ditandatangani.

### 2.1 Prasyarat (sekali saja)

| Kebutuhan | Perintah (macOS) |
|---|---|
| Alat compiler | `xcode-select --install` |
| Rust | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node 22 atau lebih baru | `brew install node` |
| pnpm | `npm install -g pnpm` (atau `corepack enable`) |

Buka terminal baru setelahnya agar `cargo` ada di `PATH`.

### 2.2 Menjalankan

Dari folder proyek Naurix:

```sh
./run.sh doctor     # memeriksa semua prasyarat dan memberi tahu cara memperbaikinya
./run.sh            # mode pengembangan (muat ulang otomatis)
```

Kompilasi pertama memakan beberapa menit. Setelah itu cepat.

### 2.3 Membuat aplikasi yang bisa diklik

| Perintah | Hasil |
|---|---|
| `./run.sh app` | Membangun `Naurix.app` lalu membukanya |
| `./run.sh install` | Membangun dan menyalin ke `~/Applications` (bisa dibuka lewat Spotlight) |
| `./run.sh dmg` | Membuat installer `Naurix_<versi>_<arsitektur>.dmg` |

Aplikasi ini **belum ditandatangani**. Di Mac lain, macOS bisa bilang "rusak" atau "developer tidak dikenal". Klik kanan lalu **Open**, atau jalankan:

```sh
xattr -dr com.apple.quarantine /Applications/Naurix.app
```

### 2.4 Mencoba tanpa mengganggu data asli

```sh
./run.sh sandbox
```

Membuka salinan kedua dengan **data sekali pakai** (folder data dan port terpisah). Cocok untuk bereksperimen.

### 2.5 Linux dan Windows

Kodenya ada tetapi **belum divalidasi di perangkat nyata**, jadi anggap eksperimental.

- **Linux:** pasang prasyarat Tauri (`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev pkg-config`), lalu `./run.sh doctor` dan `./run.sh`. Penyimpanan secret butuh Secret Service (GNOME Keyring atau KWallet) yang berjalan.
- **Windows:** pasang Rust (MSVC), Node 22+, pnpm, dan WebView2. Di PowerShell: `pnpm install` lalu `pnpm tauri dev`. Menautkan skill ke agent memakai symlink, yang butuh **Developer Mode**.

---

## 3. Langkah pertama

1. Klik **Open folder…** (atau tekan **⌘O**) lalu pilih folder proyekmu. Folder itu menjadi satu *workspace*.
2. Tekan **⌘T** untuk membuka tab terminal. Shell login-mu (`$SHELL -l`) mulai di folder itu.
3. Tekan **⌘D** untuk membagi pane ke kanan, **⇧⌘D** ke bawah. Seret garis pemisah tipis untuk mengubah ukuran.
4. Tekan **⌘K** untuk command palette. Coba *Rename tab…* atau *Switch workspace*.
5. Tekan **⇧⌘A** untuk meluncurkan agent AI (bagian 6).
6. Tutup lalu buka lagi aplikasinya. Workspace-mu masih ada. Terminal yang tadi berjalan tampil sebagai "interrupted when the app closed" (prosesnya tidak dipulihkan).

---

## 4. Tampilan utama

- **Sidebar (kiri):** daftar workspace, daftar agent yang sedang berjalan, dan menu ke tampilan lain: **Workflows**, **Memory**, **Git**, **MCP**, **Skills**, **Settings**. Di dasarnya ada kartu animasi Naurix (companion).
- **Area utama:** deretan tab dan pane terminal. Beberapa tampilan (Git, MCP, Skills, Settings, Workflows, Memory) menggantikan area ini sementara.
- **Status bar:** workspace aktif, status sesi, dan branch Git. Klik branch untuk membuka tampilan Git.
- **Command palette (⌘K):** cari dan jalankan hampir semua aksi dengan keyboard.

**Companion (kartu animasi di sidebar)** hanya informatif. Ia tidak punya kontrol dan tidak pernah memulai, menghentikan, atau menyetujui apa pun. Statusnya (Idle, Ready, Working, Waiting, Success, Error) diturunkan dari **fakta proses**, bukan tebakan. "Waiting" hanya muncul bila agent melaporkannya sendiri (bagian 8). Matikan di **Settings → General → Show Naurix**.

---

## 5. Terminal, tab, dan split

### Workspace
- **Open folder…** menambah proyek. **Rename workspace…** dan **Remove workspace…** ada di command palette.
- **Menghapus workspace tidak menghapus file proyekmu.**
- Bila folder proyek hilang, Naurix menampilkan peringatan "Folder not found" dan menawarkan untuk menghapus workspace-nya.

### Tab dan pane
- **⌘T** tab baru. **⌘W** menutup pane (menutup tab bila itu pane terakhir).
- **⇧⌘R** atau klik dua kali pada tab untuk mengganti nama.
- **⌘1 … ⌘9** pindah ke tab ke-N. **⇧⌘]** dan **⇧⌘[** pindah tab. **⌘]** dan **⌘[** pindah pane.
- Tab yang tersembunyi **tetap berjalan** dan menyimpan scrollback-nya (5.000 baris bawaan).

### Menutup dan keluar
- Mengetik `exit` (kode 0) menutup pane shell. Bila keluar dengan kode bukan nol atau dihentikan paksa, pane tetap terbuka dengan tombol **Restart** dan **Close** agar kamu bisa membaca apa yang terjadi. Pane agent selalu tetap terbuka.
- Bila pane masih menjalankan perintah, Naurix meminta konfirmasi sebelum menutup (bisa dimatikan, lihat bagian 16).
- **Menutup jendela (macOS)** hanya menyembunyikan jendela, sesi tetap berjalan, dan klik ikon Dock membukanya lagi. **⌘Q** benar-benar keluar dan menghentikan semua proses dengan sopan (SIGHUP, SIGTERM, lalu SIGKILL bila perlu). Bila masih ada pekerjaan berjalan, Naurix bertanya dulu.
- **Memuat ulang UI** tidak menghentikan sesi. Setelah reload, tiap sesi kembali sebagai satu tab (susunan split tidak dipulihkan) dan isinya diputar ulang dari buffer di memori. ⚠️ Pemeriksaan manual di macOS untuk ini masih menunggu, dan agent berbasis layar penuh (TUI) bisa tergambar kurang sempurna setelah replay. Naurix mengubah ukuran terminal sedikit agar TUI menggambar ulang.

### Menyalin, menempel, dan melampirkan
- Klik kanan pada terminal: **Copy**, **Paste**, **Split Right/Left/Up/Down**, **Close Pane**.
- **Seret file** dari Finder ke pane: Naurix mengetik path-nya (dikutip) di kursor, **tanpa menekan Enter**. Tidak ada yang dibaca atau dijalankan. Agent seperti Claude Code mengubah path gambar menjadi lampiran.
- **Tempel gambar dari clipboard** (⌘V): Naurix mengirim Ctrl+V ke program, itulah cara CLI agent membaca gambar dari clipboard.

### Font dan prompt
- Prompt bergaya Powerlevel10k butuh **Nerd Font**. Bila ikon tampil sebagai kotak kosong, pasang satu: `brew install --cask font-meslo-lg-nerd-font`, lalu mulai ulang Naurix.
- **⌘K → "Terminal font…"** mengganti font (ketik namanya, misalnya `Dank Mono`). **"Terminal font size…"** mengganti ukuran. Ketik `default` untuk mengembalikan.
- Catatan: ⌘K membuka palette, jadi tidak membersihkan layar. Pakai `clear` atau Ctrl-L.

---

## 6. Menjalankan agent AI

### Meluncurkan
1. Tekan **⇧⌘A**, atau klik **＋** di samping *Agents* di sidebar, atau **⌘K → Launch AI agent…**.
2. Tiap agent menampilkan apakah ditemukan (`✓ /path`) atau **Not found**. **Naurix tidak pernah memasang apa pun.** Bila agent terpasang di tempat tidak biasa, pilih **Set up…** lalu tunjuk executable-nya.
3. Klik **Launch**. Bila ini pertama kali, muncul **tinjauan** berisi perintah persis, folder kerja, dan environment tambahan. Konfirmasi untuk memulai.
4. Agent terbuka di tab sendiri dan muncul di bawah **Agents** di sidebar. Tombol **■** (*Stop and close*) menghentikannya, dengan konfirmasi saat masih berjalan.

Agent yang didukung sebagai preset: Claude Code, Codex CLI, Gemini CLI, Antigravity CLI (`agy`), dan OpenCode. Catatan: Google menghentikan Gemini CLI untuk sebagian besar akun individu mulai 18 Juni 2026. Peluncur memberi peringatan dan menawarkan penggantinya, Antigravity CLI.

### Di mana Naurix mencari agent
`PATH`-mu, ditambah `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `~/.claude/local`, `~/.opencode/bin`, `~/.bun/bin`, `~/.npm-global/bin`, `~/.cargo/bin`, `~/.volta/bin`, dan `~/.nvm/versions/node/*/bin`. Ini penting karena aplikasi yang dibuka dari Finder tidak melihat `PATH` shell-mu.

### Agent kustom
Klik **Add custom agent…** lalu isi:
- **Nama**
- **Executable**: nama perintah (misalnya `aider`) atau path absolut
- **Argumen**: satu per baris, tanpa kutipan shell
- **Environment** (opsional): baris `NAMA=nilai`
- **Security role** dan **Success exit codes in workflows** (opsional): lihat bagian 12

Nilai environment disimpan tanpa enkripsi, jadi nama yang tampak seperti rahasia (`…KEY`, `…TOKEN`, `…SECRET`, `…PASSWORD`) **ditolak**. Simpan API key di profil shell-mu. Agent kustom **selalu ditinjau** setiap peluncuran, kecuali kamu menyetujuinya untuk workspace (bagian 9).

Agent kustom juga bisa berupa perintah biasa seperti `npm test`, yang berguna untuk langkah workflow.

### Kemampuan ditampilkan apa adanya
Di langkah tinjauan, **What Naurix can do with this agent** membagi kemampuan menjadi:
- **Verified**: sudah dikonfirmasi dengan CLI asli.
- **Documented but not enabled**: ada di dokumentasi resmi tetapi belum dikonfirmasi. Naurix tidak bertindak atas dasar ini.
- **Not supported yet**.

Naurix hanya bertindak atas yang *verified*.

### Batas agent paralel
Naurix membatasi berapa agent yang berjalan bersamaan: **bawaan 4, rentang 1–16**. Hanya agent yang dihitung, terminal biasa tidak dibatasi. Bila batas penuh, peluncuran ditolak dengan pesan jelas dan **tidak ada izin yang terpakai**. Ubah di **Settings → AI Agents → Agents running at once**.

---

## 7. Menjalankan agent di worktree terpisah

Bila dua agent bekerja di folder yang sama, keduanya bisa saling menimpa file. **Git worktree** memberi tiap agent salinan kerja sendiri, di branch sendiri.

1. Buka peluncur agent (⇧⌘A).
2. Centang **Run in a separate worktree**. Pilihan ini **mati secara default** dan hanya tampil bila workspace-mu repositori Git yang tidak diblokir.
3. Bila ada agent lain yang sudah berjalan di folder yang sama, muncul **peringatan** yang menyarankan worktree (hanya saran, tidak memaksa).
4. Tinjauan menampilkan perintah Git persis dan perintah agent. Persetujuan worktree selalu **sekali saja**.

Yang terjadi:
- Branch baru bernama `naurix/<nama-agent>-<tanggal-jam>`.
- Folder worktree dibuat di folder data Naurix (`<app data>/worktrees/<id workspace>/<nama branch>`), bukan di samping repositorimu.
- Hook milik repositori **tidak dijalankan** saat pembuatan.
- **File yang tidak dilacak atau di-ignore (misalnya `node_modules`, `.env`) tidak disalin**, begitu juga submodule. Siapkan sendiri atau minta agent menyiapkannya.
- Sidebar menampilkan nama branch untuk agent yang berjalan di worktree.

Menghapus worktree:
- Hanya worktree **buatan Naurix** yang bisa dihapus. Naurix tidak memakai `--force`: Git menolak bila ada perubahan atau berkas baru, atau worktree terkunci atau sedang dipakai sesi.
- **Branch selalu dipertahankan**, supaya commit agent tidak hilang. Menghapus branch kamu lakukan sendiri.
- Worktree yang tertinggal (misalnya workspace-nya sudah dihapus) tercantum di **Settings → Security → Worktrees left behind** dengan tombol **Remove…**.

Naurix tidak melakukan merge, rebase, push, atau menghapus branch.

---

## 8. Melihat apa yang sedang dikerjakan agent

Secara bawaan status hanya menggambarkan **proses**: *Starting, Running, Exited, Failed*. Naurix tidak mengklaim agent "menunggu persetujuan" atau "selesai", karena tidak bisa tahu tanpa sinyal yang andal dari agent itu sendiri.

Untuk **Claude Code** ada opsi sinyal yang andal, **mati secara default**:

1. Buka **Settings → AI Agents**.
2. Nyalakan **Show what Claude Code is doing**.

Bila menyala, Naurix meluncurkan Claude Code dengan hook yang hanya melaporkan status seperti "working" dan "waiting for approval". Hook **tidak membaca** prompt, jawaban, atau input tool, dan **tidak menulis** ke konfigurasi Claude Code-mu (hook diberikan per peluncuran lewat `--settings`). Hanya status terverifikasi yang ditampilkan, tanpa teks pesan.

Agent lain belum punya sinyal terverifikasi, jadi tetap menampilkan status proses saja.

---

## 9. Approval dan keamanan

Naurix memakai **satu pintu persetujuan** untuk semua aksi agent.

### Cara kerjanya
Sebelum agent dimulai (atau worktree dibuat, atau workflow dijalankan), Naurix membangun **ringkasan persis** dari apa yang akan terjadi dan menunjukkannya. Kamu memilih jawaban:

| Pilihan | Artinya |
|---|---|
| **Sekali (once)** | Dipakai untuk satu aksi ini saja |
| **Workspace** | Berlaku di workspace ini untuk aksi yang identik |
| **App** | Khusus preset agent: berlaku di semua workspace, terikat ke path executable persis |
| **Run** | Khusus workflow: berlaku untuk satu eksekusi workflow |

- Persetujuan terikat ke **fingerprint** apa yang kamu lihat (program, argumen, environment tambahan). Bila ada yang berubah, kamu ditanya lagi.
- **Tidak ada pilihan "setujui semuanya".** Pembuatan worktree dan penulisan konfigurasi agent selalu hanya **sekali**.
- Permintaan persetujuan kedaluwarsa dalam 5 menit dan hanya bisa dipakai sekali.
- Catatan jujur: fingerprint **tidak** mencakup isi biner executable. Bila CLI diperbarui di path yang sama, ia tetap dianggap disetujui.

### Melihat dan mencabut
- **Settings → Security → Agent launch approvals** menampilkan izin aktif. Setiap izin punya **Revoke**, dan ada **Revoke all**.
- **MCP approvals → Withdraw all…** menarik persetujuan semua server MCP (tidak ada yang dihapus).
- **Trusted Git repositories** menampilkan repositori yang kamu percayai. Cabut untuk berhenti menjalankan Git di sana.
- Menghapus workspace menghapus izin miliknya.

---

## 10. Workflows

⚠️ *Fitur ini baru diuji lewat skrip dan dengan perintah `sh`, belum dengan agent CLI nyata dan belum pernah dilihat manusia.*

Workflow adalah **kumpulan langkah yang bisa dijalankan berulang**. Setiap langkah adalah "jalankan agent X". Langkah bisa bergantung pada langkah lain.

### Membuat workflow
1. Buka proyek, lalu pilih **Workflows** di sidebar.
2. Klik **New workflow** dan beri nama (misalnya "Build, then review").
3. Klik **Add step** untuk tiap langkah. Pilih **agent**-nya dan, bila perlu, langkah sebelumnya yang harus selesai dulu. Editor hanya menawarkan langkah **sebelumnya** sebagai dependensi, jadi siklus tidak bisa dibuat.
4. Simpan.

Batas: maksimal 24 langkah dan 64 dependensi, nama 1–60 karakter.

**Langkah hanya referensi ke agent.** Tidak ada prompt tersimpan dan tidak ada input otomatis ke agent. Bila langkah butuh argumen tetap (misalnya `npm test`), buat **agent kustom** untuknya (bagian 6).

### Menjalankan
1. Klik **Run**. Naurix menampilkan **tinjauan**: perintah persis tiap langkah, dependensi, dan apakah tiap agent sudah disetujui. Belum ada yang dimulai.
2. Klik **Approve this run once and start**. Ini satu persetujuan untuk seluruh run.
3. Langkah mulai berjalan saat dependensinya selesai. Langkah yang tidak saling bergantung bisa berjalan bersamaan, selama batas agent belum penuh. Langkah yang menunggu slot tetap `pending` dan dimulai otomatis saat ada sesi yang berakhir.

### Status langkah
| Status | Arti |
|---|---|
| `pending` | Menunggu dependensi atau slot |
| `running` | Sesinya sedang berjalan |
| `succeeded` | Proses keluar dengan kode 0, atau kamu menekan **Mark done**, atau dilewati lewat Skip |
| `failed` | Keluar dengan kode bukan nol, gagal diluncurkan, atau dihentikan sinyal |
| `skipped` | Kamu memilih **Skip** |
| `cancelled` | Run dibatalkan |

Langkah yang dependensinya gagal atau dibatalkan tetap `pending` dan ditampilkan sebagai **blocked**. Ia tidak pernah mulai sendiri.

### Kontrol selama run
- **Open terminal**: memasang pane ke sesi langkah itu agar kamu bisa melihat dan mengetik. Secara bawaan langkah berjalan tanpa tampil di tab.
- **Mark done**: menandai langkah yang sedang berjalan sebagai selesai. **Agent tidak dihentikan.** Pakai ini untuk agent interaktif yang tidak keluar sendiri.
- **Skip**: melewati langkah yang belum mulai agar langkah berikutnya boleh jalan.
- **Cancel run**: tidak memulai langkah baru dan membiarkan agent yang sedang berjalan tetap terbuka.

### Hal yang perlu diketahui
- Kode 0 hanya berarti proses selesai bersih, **bukan** bahwa hasil kerjanya benar. Untuk agent interaktif, "berhasil" adalah kata kamu (Mark done).
- Tidak ada sinyal status atau pola output yang menyelesaikan langkah.
- Izin run **dicabut saat run berakhir** dan hanya cocok untuk peluncuran oleh run itu. Bila ada peluncuran yang tidak tercakup tinjauan (misalnya path program berubah), langkahnya gagal dengan catatan `approval missing`, bukan bertanya di tengah jalan.
- Worktree **tidak** menjadi bagian workflow. Langkah berjalan di folder workspace.
- Tidak ada kondisi, loop, retry, jadwal, atau variabel.
- Workflow tidak bisa diubah atau dihapus saat ada run aktif. Menghapus workflow menyisakan riwayat run.
- Bila Naurix dimulai ulang saat run berjalan, run ditandai `interrupted` dan tidak dilanjutkan otomatis.

---

## 11. Memory (catatan proyek)

⚠️ *Tampilan ini baru diuji lewat skrip. Ekspor lewat dialog simpan belum diuji di aplikasi, dan penyisipan belum dicoba dengan agent CLI nyata.*

Memory adalah tempat **catatan yang kamu tulis sendiri** tentang sebuah proyek. Naurix **tidak pernah** menyimpan prompt, transkrip, atau output terminal sebagai memori, dan tidak ada jalur bagi agent untuk menulis catatan.

### Menyalakan
Memory **mati** untuk setiap workspace sampai kamu menyalakannya.
1. Pilih **Memory** di sidebar.
2. Nyalakan sakelarnya. Selama mati, hanya sakelar yang tampil.

Bila kamu mematikannya kembali, catatan **tetap disimpan tetapi disembunyikan**. Tampilan menyebut berapa catatan yang tersimpan, dengan tombol **Delete all notes** di sebelahnya.

### Menulis catatan
- Klik **New note**. Isi **Title** (1–100 karakter) dan teks (hingga 8.000 karakter). Maksimal 200 catatan per workspace.
- **Dugaan rahasia ditolak, tanpa opsi "simpan saja".** Naurix mengenali bentuk seperti private key, kunci AWS, token GitHub, kunci `sk-…`, token Slack, kunci Google, JWT, dan penetapan seperti `password = …`. Pesan penolakan menyebut **baris dan jenisnya**, tidak pernah nilainya. Pemindai ini jaring pengaman dan bisa keliru. Bila catatanmu ditolak secara keliru, ubah teksnya. Simpan rahasia di credential store sistem, bukan di catatan.

### Mencari, mengubah, menghapus
- **Search notes** mencari lewat FTS5. Kata dicocokkan sebagai awalan (mengetik `depl` menemukan "deploy"). Hasil dibatasi 50. Tanpa kata kunci, yang terbaru ditampilkan.
- Tiap catatan bisa diubah atau dihapus (**Delete**). **Delete all** menghapus semua catatan workspace setelah konfirmasi. Menghapus catatan juga menghapusnya dari indeks pencarian.
- Menghapus workspace menghapus catatannya.

### Menyisipkan ke agent
**Insert into agent terminal** mengetik teks catatan ke **pane aktif, bila pane itu sesi agent**. **Enter tidak ditekan.** Kamu melihat teksnya di kolom input agent, bisa mengubah atau membuangnya, lalu mengirimnya dengan Enter-mu sendiri.
- **Ditolak untuk pane shell biasa**, karena baris baru bisa menjalankan perintah.
- Catatan **satu baris** selalu bisa disisipkan. Catatan **beberapa baris** hanya bila agent mengaktifkan *bracketed paste*. Bila tidak, Naurix menolak dan menjelaskan alasannya, karena program seperti `cat` menganggap tiap baris tempel sebagai Enter.
- Tidak ada file agent (CLAUDE.md dan sejenisnya) yang ditulis.

### Mengekspor
**Export Markdown** atau **Export JSON**. Naurix membuka dialog simpan sendiri, jadi lokasi selalu pilihanmu. Bila lokasi itu **di dalam repositori Git**, kamu diminta konfirmasi kedua yang menyebut risikonya. Naurix tidak pernah menjalankan `git add` atau `git commit` untuk memori.

---

## 12. Security (laporan keamanan)

Buka **Security** di sidebar (atau **⌘K → Security reports**). Di sini kamu membaca laporan **SARIF** yang ditulis scanner atau agent, dan membandingkan dua laporan untuk melihat temuan yang baru. Red, Blue, dan Purple team bukan program tersendiri. Ketiganya adalah **peran** yang kamu berikan pada agent kustom, sedangkan instruksinya dipasang sebagai skill (bagian 14).

### Menyiapkan tim
- Di **Add custom agent…** (bagian 6), pilih **Security role**: Red, Blue, atau Purple. Peran hanya label dan tidak mengubah apa yang dijalankan.
- Banyak scanner keluar dengan kode **1** bila menemukan masalah. Isi **Success exit codes in workflows** dengan `0, 1` supaya langkah workflow berikutnya tetap berjalan. Mengubah kode ini meminta approval ulang.
- Jalankan scanner sebagai agent kustom atau langkah workflow, dan arahkan output SARIF-nya ke file di proyek.

### Membuka laporan
**Open report…** membuka dialog pilih file di folder proyek. Laporan dianggap **tidak tepercaya**:
- Maksimal 20 MiB dan 5.000 temuan. Sisanya dihitung, tidak ditampilkan.
- Path di luar proyek tidak ditampilkan. Temuannya tetap muncul, tanpa lokasi.
- Pesan yang tampak berisi secret disembunyikan.
- Potongan kode (snippet) tidak pernah dibaca.

Saring temuan menurut tingkat keparahan (Critical, High, Medium, Low, Info) atau menurut teks.

### Membandingkan (regresi)
**Compare two reports…** meminta dua file: **baseline** (yang lebih lama), lalu laporan **terbaru**. Hasilnya dibagi tiga: **New**, **Still present**, dan **No longer reported**. "No longer reported" hanya berarti scanner berhenti melaporkannya; itu **bukan bukti** masalahnya sudah diperbaiki.

### Yang perlu diketahui
- **Tidak ada yang disimpan.** Laporan hilang saat kamu meninggalkan tampilan ini atau berganti proyek. File SARIF-nya sendiri tetap di proyekmu.
- Naurix tidak menjalankan scanner dari tampilan ini dan tidak mengirim apa pun ke agent.

---

## 13. MCP Integrations

Buka **MCP** di sidebar (atau **⌘K → MCP integrations**). MCP adalah cara agent memakai tool eksternal. Naurix menyimpan definisi server sekali di satu tempat lalu menyinkronkannya ke konfigurasi agent.

1. **Tambah server:** **Add from catalog…** (Filesystem, GitHub, PostgreSQL, Playwright, Context7, Sentry) atau **Add manually…**. Belum ada yang berjalan.
2. **Secret** (token, password, connection string): **jangan** ketik di kolom perintah, argumen, environment, atau header. Deklarasikan nama secret, tulis `${secret:NAMA}` di tempat nilainya (misalnya `Authorization: Bearer ${secret:GITHUB_PAT}`), lalu klik **Set value…** pada kartu server. Nilainya disimpan di **credential store sistem** (macOS Keychain, Linux Secret Service, Windows Credential Manager), tidak pernah di database Naurix atau file konfigurasi agent, dan tidak ditampilkan lagi. Kredensial teks biasa ditolak.
3. **Review & approve:** menampilkan perintah atau URL persis. Persetujuan terikat ke definisi itu, jadi mengubah perintah, argumen, URL, environment, atau header meminta persetujuan ulang. Perintah seperti `npx` memperingatkan bahwa ia mengunduh dan menjalankan paket.
4. **Test connection** (server lokal): menjalankan perintah yang disetujui, meminta daftar tool, lalu menghentikannya. Server jarak jauh belum bisa diuji dari Naurix.
5. **Sync to agents:** pilih agent → **Preview changes…** → **Apply…**. Naurix menampilkan entri yang akan ditambah, diubah, atau dihapus, **mencadangkan file lebih dulu**, dan menyediakan **Undo**. Ia hanya menyentuh entri yang ia tulis sendiri. Entri dengan nama sama atau yang kamu ubah dibiarkan dan dilaporkan sebagai konflik.

Saat ini sinkronisasi bekerja untuk **Claude Code** (`.mcp.json` proyek) dan **OpenCode** (`~/.config/opencode/opencode.json[c]` bentuk datar `mcp.<nama>`). Bila file punya komentar, Naurix tidak menulis ulang dan menampilkan entri untuk kamu tempel sendiri.

**Penting:** file konfigurasi agent hanya berisi *referensi* ke secret. Naurix memasok nilai aslinya saat meluncurkan agent, jadi **mulai Claude Code atau OpenCode dari panel Agents** untuk server yang memakai secret. Agent yang dimulai di luar Naurix mendapat teks referensi, bukan nilainya.

---

## 14. Agent Skills

Buka **Skills** di sidebar. Skill adalah folder berisi file `SKILL.md`.

1. **Install skills…** dari **URL Git HTTPS** atau **folder di komputermu**. Naurix menampilkan **tinjauan**: asalnya, lokasi penyimpanan, jumlah file, setiap file mirip skrip, peringatan risiko (perintah shell, tool shell yang disetujui di muka, hook, biner), dan agent yang kompatibel. **Tidak ada yang terpasang sebelum kamu memilih dan mengonfirmasi.**
2. **Skill tidak dipercaya.** Naurix tidak pernah menjalankan apa pun di dalam skill. Agent mungkin menjalankan skrip atau perintahnya nanti, makanya tinjauan mendaftarnya.
3. **Use with agents…**: nyalakan skill untuk agent, untuk **semua proyek** atau **proyek ini**. Naurix membuat symlink di folder yang disebut dokumentasi agent itu, hanya bila belum ada isinya, dan menghapusnya saat dimatikan.
4. **Check for updates** menampilkan file yang ditambah, diubah, atau dihapus, plus peringatan baru. Versi lama disimpan, dan **Switch to previous version** mengembalikannya.
5. **Uninstall** menghapus tautan, salinan tersimpan, dan catatannya. Folder atau repositori aslimu tidak disentuh.

**Batas jujur:** Naurix menaruh tautan di tempat yang disebut dokumentasi, tetapi belum memverifikasi bahwa tiap agent benar-benar memuat skill lewat symlink (ditandai *Link only, not verified*).

---

## 15. Git

Buka **Git** di sidebar, atau klik branch di status bar. Tampilan ini **hanya-baca**.

- Menampilkan branch (atau detached HEAD), ahead/behind terhadap upstream, stash, merge atau rebase yang sedang berlangsung, dan file yang berubah dalam kelompok **Conflicts / Staged / Changes / Untracked**.
- Klik file untuk melihat diff-nya.
- **Open in terminal** membuka shell di root repositori, walau workspace-mu subfolder.
- Naurix tidak pernah menjalankan stage, commit, checkout, atau mengubah apa pun, kecuali pembuatan dan penghapusan worktree yang kamu setujui (bagian 7).
- Tidak ada polling. Status diperbarui saat kamu memilih workspace, membuka tampilan, mengembalikan jendela ke depan, menyelesaikan sesi, atau menekan **Refresh**. Naurix tidak mengambil *index lock* Git, jadi tidak membuat perintah `git`-mu gagal.
- **Aman pada repositori asing:** Git bisa menjalankan program yang disebut konfigurasi repositori. Naurix mematikannya. Bila konfigurasi lokal repositori mendefinisikan filter atau include (yang tidak bisa dimatikan), Naurix menolak menjalankan Git di sana sampai kamu menekan **Trust this repository…**. Batalkan nanti di **Settings → Security → Trusted Git repositories**.
- Perubahan besar dipotong (2.000 file pertama, diff sekitar 512 KB) dan tampilan menyebutnya.

---

## 16. Pengaturan

Tekan **⌘,** atau pilih **Settings**. Perubahan divalidasi backend, berlaku seketika, dan tersimpan.

| Bagian | Isi |
|---|---|
| **General** | **Close pane on clean exit** (tutup pane shell yang keluar dengan kode 0), **Confirm closing running work**, **Show Naurix** (companion), **Animate Naurix**, **Reduce Naurix animation**, dan **Reset all settings…** (proyek, agent, MCP, dan skill tidak terpengaruh) |
| **Appearance** | 9 tema (Naurix Dark/Light, Tokyo Night, Dracula, Nord, Catppuccin Mocha/Latte, GitHub Light, Solarized Light), mode Dark / Light / Follow system, dan warna aksen kustom. Warna terminal ikut tema tanpa memulai ulang shell |
| **Terminal** | Font dan ukuran, scrollback (baris), gaya kursor, **Start shells as login shells**, dan path shell (kosongkan untuk memakai `$SHELL`) |
| **AI Agents** | **Agents running at once** (1–16), **Show what … is doing** per agent (bagian 8), dan **Add or edit custom agents…** |
| **MCP Integrations** | Ringkasan server dan secret, tombol **Open MCP integrations** |
| **Agent Skills** | Ringkasan skill terpasang, tombol **Open agent skills** |
| **Performance** | **Terminal renderer** (DOM bawaan, WebGL eksperimental), scrollback, **Blink the cursor** (mematikannya menurunkan CPU saat idle), dan **Memory now** |
| **Security** | Telemetry (tidak ada), Network (hanya atas permintaan), lokasi data, credential store, **MCP approvals**, **Agent launch approvals**, **Worktrees left behind**, **Trusted Git repositories** |

Tips performa: bila menjalankan banyak terminal, turunkan scrollback.

---

## 17. Pintasan keyboard

| Aksi | macOS | Linux dan Windows |
|---|---|---|
| Command palette | ⌘K | Ctrl+Shift+K |
| Buka folder proyek | ⌘O | Ctrl+Shift+O |
| Tab terminal baru | ⌘T | Ctrl+Shift+T |
| Tutup pane | ⌘W | Ctrl+Shift+W |
| Split kanan | ⌘D | Ctrl+Shift+D |
| Split bawah | ⇧⌘D | Ctrl+Shift+E |
| Pane berikutnya / sebelumnya | ⌘] / ⌘[ | Ctrl+Alt+] / Ctrl+Alt+[ |
| Tab berikutnya / sebelumnya | ⇧⌘] / ⇧⌘[ | Ctrl+Shift+] / Ctrl+Shift+[ |
| Pindah ke tab 1–9 | ⌘1 … ⌘9 | Ctrl+Alt+1 … 9 |
| Ganti nama tab | ⇧⌘R | Ctrl+Shift+R |
| Luncurkan agent AI | ⇧⌘A | Ctrl+Shift+A |
| Pengaturan | ⌘, | Ctrl+, |
| Salin / tempel | ⌘C / ⌘V | Ctrl+Shift+C / Ctrl+Shift+V |

Di Linux dan Windows, Ctrl+C, Ctrl+W, Ctrl+D, dan sejenisnya dibiarkan untuk shell, makanya pintasannya memakai Ctrl+Shift.

### Aksi di command palette (⌘K)
Open project folder…, New terminal tab, Launch AI agent…, Split pane right / down, Close pane, Close tab, Rename tab…, Focus next / previous pane, Next / Previous tab, Go to tab: …, Switch workspace: …, Rename workspace…, Remove workspace…, Terminal font…, Terminal font size…, Git status, Refresh Git status, MCP integrations, Agent skills, Settings.

---

## 18. Data, privasi, dan reset

**Yang disimpan** (metadata saja): nama dan path workspace, judul, status, dan waktu sesi, pengaturan, agent kustom dan executable yang disetujui, definisi MCP dan skill, izin persetujuan, worktree yang dibuat Naurix, workflow dan riwayat run, dan catatan Memory yang kamu tulis.

**Yang tidak pernah disimpan:** input atau output terminal, prompt, nilai environment, dan isi source code. Secret hanya di credential store sistem. Tidak ada telemetri, analytics, atau pelaporan crash.

**Jaringan:** Naurix hanya memakainya atas permintaanmu (mengunduh skill dari Git, atau server yang kamu setujui). Tidak ada yang berjalan di latar belakang.

**Memori sementara:** agar sesi bisa tersambung lagi setelah reload, Naurix menyimpan output terakhir tiap sesi (mulai 1 MiB) **hanya di memori**. Ini tidak pernah ditulis ke disk dan hilang saat aplikasi keluar.

Lokasi data di macOS:

```sh
~/Library/Application Support/dev.naurix.app/naurix.db     # database SQLite
rm -rf ~/Library/Application\ Support/dev.naurix.app        # reset semuanya
```

Menjalankan perintah `rm -rf` di atas menghapus semua workspace, pengaturan, catatan Memory, dan definisi. Secret di Keychain tidak terhapus oleh perintah itu.

Izin akses folder macOS: macOS bisa menanyakan akses ke Downloads, Documents, atau Desktop. Naurix sendiri tidak membaca folder itu. Shell dan agent berjalan sebagai proses anak aplikasi, jadi macOS menganggap akses mereka sebagai akses Naurix. **Don't Allow** aman, hanya akses ke folder itu yang ditolak.

---

## 19. Pemecahan masalah

| Masalah | Solusi |
|---|---|
| Agent tampil **Not found** padahal terpasang | Pilih **Set up…** lalu tunjuk executable-nya |
| Ikon prompt tampil sebagai kotak kosong | Pasang Nerd Font (bagian 5), lalu mulai ulang Naurix |
| `cargo: command not found` | `source ~/.cargo/env` atau buka terminal baru |
| `Port 1420 is already in use` | Ada instance dev lain: `pkill -f "tauri dev"; pkill -f vite` |
| Jendela kosong saat mode dev | Tunggu server Vite, lihat galat di terminal yang menjalankan `pnpm tauri dev` |
| Prompt pertama lama muncul | Shell login-mu lambat mulai. Ukur dengan `time zsh -l -i -c exit` |
| "App is damaged / can't be opened" di Mac lain | Aplikasi belum ditandatangani: klik kanan → **Open**, atau `xattr -dr com.apple.quarantine "Naurix.app"` |
| Peluncuran agent ditolak karena batas | Naikkan **Agents running at once** atau tutup agent lain. Tidak ada izin yang terpakai saat ditolak |
| Tombol **Insert into agent terminal** ditolak | Pane aktif harus sesi agent. Catatan beberapa baris butuh agent dengan bracketed paste. Coba catatan satu baris |
| Catatan Memory ditolak | Teks mirip rahasia. Lihat baris yang disebut dan ubah |
| Langkah workflow `blocked` | Dependensinya gagal atau dibatalkan. Perbaiki lalu jalankan run baru, atau **Skip** langkah yang gagal |
| Git: "This repository runs commands from its own configuration" | Percayai repositori itu hanya bila kamu mengenalnya (**Trust this repository…**) |
| Build `.dmg` gagal dengan `AppleEvent timed out` | Izinkan terminalmu mengendalikan Finder (System Settings → Privacy & Security → Automation), atau `CI=true pnpm tauri build --bundles dmg` |

---

## 20. Pertanyaan umum

**Apakah Naurix mengirim kode atau prompt-ku ke server?** Tidak. Naurix tidak punya telemetri dan tidak berbicara ke layanan AI. Agent yang kamu jalankan sendiri yang berkomunikasi dengan penyedianya, di luar kendali Naurix.

**Apakah Naurix memerlukan akun?** Tidak.

**Apakah saya bisa memakainya hanya sebagai terminal?** Ya. Semua fitur agent dan orkestrasi bersifat tambahan.

**Mengapa Naurix tidak menampilkan "agent sudah selesai"?** Karena ia tidak bisa mengetahuinya dengan andal, kecuali agent melaporkannya sendiri lewat sinyal terverifikasi (saat ini hanya Claude Code, opsional, bagian 8). Naurix lebih memilih jujur daripada menebak.

**Apakah Naurix bisa menyetujui aksi agent secara otomatis?** Tidak. Tidak ada mode "setujui semuanya".

**Apakah sesi bertahan bila saya menutup jendela?** Di macOS ya, selama aplikasi tetap berjalan (⌘Q menghentikan semuanya). Sesi **tidak** bertahan setelah aplikasi benar-benar keluar atau dimulai ulang. Sesi lama tampil sebagai *interrupted*.

**Apakah proses agent tetap hidup bila Naurix crash?** Belum dijamin. Perilaku saat aplikasi dihentikan paksa belum diuji dan tidak boleh dianggap sebagai jaminan.

**Platform apa yang didukung?** macOS (Intel dan Apple Silicon, dengan validasi Apple Silicon di perangkat nyata masih menunggu). Linux dan Windows: kode ada, belum divalidasi.

**Di mana saya melapor bug atau kerentanan?** Bug: issue di repositori. Kerentanan keamanan: lewat GitHub Security Advisories secara privat, jangan lewat issue publik (lihat `SECURITY.md`).

---

*Dokumen teknis lengkap: [`NAURIX_OVERVIEW.md`](NAURIX_OVERVIEW.md). Keputusan arsitektur: folder [`adr/`](adr/).*
