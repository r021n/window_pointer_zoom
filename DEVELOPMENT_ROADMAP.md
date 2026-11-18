# Development Roadmap & Project Specification: Window Pointer Zoom (Full-Screen Zoom)

Aplikasi utilitas Windows desktop untuk melakukan pembesaran **seluruh layar (*Full-Screen Desktop Zoom*)** secara dinamis, di mana tampilan layar membesar (*zoom in/out*) dan viewport kamera layar bergeser (*panning*) mengikuti pergerakan kursor mouse secara *real-time*.

---

## 1. Ringkasan Proyek (Project Overview)

* **Nama Proyek:** Window Pointer Zoom (Full-Screen Desktop Zoom Utility)
* **Target Platform:** Windows 10 & Windows 11 (64-bit)
* **Arsitektur:** **Hybrid Architecture** (Tauri 2.0 + React TypeScript + Native Windows Fullscreen Magnification Engine)
* **Konsep Utama (Bukan Lensa Bulat / Bukan Kaca Pembesar):**
  - **Full-Screen Scaling:** Ketika fitur zoom aktif, **seluruh monitor/layar** diperbesar (*zoom in* ke 1.5x, 2.0x, 3.0x, dst.).
  - **Pointer-Centric Viewport:** Area yang ditampilkan dan difokuskan pada layar yang diperbesar adalah area di sekitar pointer mouse.
  - **Dynamic Panning:** Saat pointer mouse digerakkan, viewport layar secara dinamis dan mulus bergeser (*pan*) mengikuti pergerakan kursor agar kursor dan area sekitarnya selalu terlihat di layar.
  - **Smooth Zoom In / Out:** Transisi pembesaran dan pengecilan layar kembali ke 1.0x (normal) berjalan halus (*animated/smooth interpolation*).
  - **Nilai Guna:** Sangat efektif untuk presentasi teknis, rekaman tutorial video (*screen recording*), demonstrasi UI/coding, serta alat bantu aksesibilitas visual.

---

## 2. Arsitektur Teknis (Hybrid Architecture)

```mermaid
flowchart TD
    subgraph Frontend ["Frontend (Webview2 / React 18+ TS)"]
        UI["Control Center / Settings Window"]
        TrayMenu["System Tray Menu"]
        StateStore["Config Store (Zoom steps, Panning mode, Shortcuts)"]
    end

    subgraph IPC ["Tauri 2.0 Bridge (IPC)"]
        Cmds["Tauri Commands (zoom_in, zoom_out, set_panning_mode)"]
        Plugins["Tauri Plugins (GlobalShortcut, Store, Autostart)"]
    end

    subgraph Backend ["Backend Native (Rust)"]
        AppCore["Tauri App Core & Coordinator"]
        ZoomEngine["Fullscreen Magnification Engine Thread"]
        ViewportCalc["Viewport & Panning Math (xOffset, yOffset, Lerp)"]
    end

    subgraph OS ["Windows Operating System"]
        Win32API["Windows Magnification API (MagSetFullscreenTransform)"]
        DWM["Desktop Window Manager (DWM / Hardware GPU Accelerated)"]
        MouseTrack["Cursor Position Tracking (GetCursorPos)"]
    end

    UI -->|Pengaturan & Konfigurasi| Cmds
    Cmds --> AppCore
    AppCore --> ZoomEngine
    MouseTrack -->|Posisi Kursor| ViewportCalc
    ViewportCalc -->|Kalkulasi xOffset, yOffset, scale| ZoomEngine
    ZoomEngine -->|MagSetFullscreenTransform| Win32API
    Win32API --> DWM
    Plugins --> AppCore
```

### Mengapa Pendekatan Hybrid untuk Full-Screen Zoom?
1. **Dukungan Native OS Level (`MagSetFullscreenTransform`):**  
   Windows memiliki fungsi native resmi untuk memperbesar seluruh layar langsung di tingkat kompositor DWM (*Desktop Window Manager*). Hasilnya adalah pembesaran 60–144+ FPS dengan akselerasi GPU tanpa lag render.
2. **Tanpa Rekursi Gambar:**  
   Karena DWM langsung memanipulasi transformasi layar sistem, kita tidak memerlukan teknik penangkapan frame/screenshot manual.
3. **UI Pengaturan Modern (React TS):**  
   Pengguna mengonfigurasi tombol shortcut, kecepatan animasi zoom, batas deadzone panning, dan tingkat pembesaran melalui UI desktop yang estetik.

---

## 3. Rumus & Cara Kerja Matematis Viewport Panning

Ketika layar diperbesar dengan faktor $\text{scale} > 1.0$:

1. **Ukuran Viewport yang Tampak:**
   $$\text{viewWidth} = \frac{\text{screenWidth}}{\text{scale}}$$
   $$\text{viewHeight} = \frac{\text{screenHeight}}{\text{scale}}$$

2. **Kalkulasi Offset Sudut Kiri-Atas Layar ($xOffset, yOffset$):**
   Agar kursor berada tepat di tengah layar yang di-zoom:
   $$\text{targetX} = \text{cursor.x} - \left(\frac{\text{viewWidth}}{2}\right)$$
   $$\text{targetY} = \text{cursor.y} - \left(\frac{\text{viewHeight}}{2}\right)$$

3. **Clamping (Mencegah Layar Melebihi Batas Fisik Desktop):**
   $$\text{xOffset} = \max(0, \min(\text{targetX}, \text{screenWidth} - \text{viewWidth}))$$
   $$\text{yOffset} = \max(0, \min(\text{targetY}, \text{screenHeight} - \text{viewHeight}))$$

4. **Smooth Interpolation (Lerp):**  
   Untuk transisi yang halus (tidak kaku), nilai $xOffset$, $yOffset$, dan $scale$ diinterpolasi menggunakan LERP (*Linear Interpolation*) setiap tick (misal 60–120 Hz).

---

## 4. Komponen Teknologi Kunci

| Komponen | Teknologi | Deskripsi / Peran |
| :--- | :--- | :--- |
| **Framework Aplikasi** | **Tauri 2.x (Latest Stable)** | Runtime desktop ringan berbasis Rust & Webview2. |
| **Frontend UI** | **React 18/19 + TypeScript + Vite** | Antarmuka dashboard, kontrol slider zoom, preset, dan shortcut. |
| **Styling & Komponen** | **Tailwind CSS + Shadcn UI / Radix** | Komponen UI modern, bersih, dan mendukung Dark/Light mode. |
| **Backend Core** | **Rust (2021 edition)** | Concurrency, mathematical interpolation, dan thread koordinasi. |
| **Magnification Engine**| **`windows` Crate (`Win32_UI_Magnification`)** | Memanggil `MagInitialize`, `MagSetFullscreenTransform`, `MagUninitialize`. |
| **Sistem Tombol Cepat** | **`@tauri-apps/plugin-global-shortcut`** | Mendengarkan hotkey global OS (misal `Win + Alt + Z`, `Ctrl + Alt + Scroll`). |
| **Penyimpanan State** | **`@tauri-apps/plugin-store`** | Menyimpan konfigurasi pengguna (step zoom, max zoom, mode panning) ke JSON. |

---

## 5. Rencana & 7 Fase Pengembangan (Revisi untuk Full-Screen Zoom)

### Fase 1: Core Engine Full-Screen Magnification (PoC Rust)
*Fokus: Memvalidasi kemampuan pembesaran seluruh layar dan kalkulasi offset viewport mengikuti kursor.*
* **Aktivitas yang dikerjakan:**
  - Menyiapkan crate `windows` (fitur `Win32_UI_Magnification`, `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`).
  - Menginisialisasi Magnification API (`MagInitialize`).
  - Menguji pemanggilan `MagSetFullscreenTransform(magLevel, xOffset, yOffset)`:
    - Mengubah level dari 1.0x ke 2.0x (seluruh layar membesar).
    - Menghitung koordinat sudut $(xOffset, yOffset)$ berdasarkan posisi mouse (`GetCursorPos`).
    - Menggeser viewport saat mouse bergerak sehingga area di sekitar pointer selalu terlihat.
  - Menerapkan perataan pergerakan (*smoothing / lerp*) agar perpindahan viewport terasa sinematik dan tidak memicu pusing.
  - Memastikan reset kembali ke layar normal 1.0x berfungsi mulus.
* **Output / Definition of Done (DoD):**
  - [ ] Binary prototipe Rust berhasil memperbesar seluruh layar saat diaktifkan.
  - [ ] Viewport seluruh layar bergeser mulus mengikuti kursor mouse saat kursor mendekati tepi atau bergerak.
  - [ ] Layar dapat di-reset kembali ke ukuran 1.0x tanpa distorsi visual.

---

### Fase 2: Integrasi Arsitektur Tauri 2.0 & Jembatan IPC
*Fokus: Mengintegrasikan engine fullscreen zoom ke dalam Tauri 2.0 dan menyediakan API perintah IPC.*
* **Aktivitas yang dikerjakan:**
  - Setup template proyek Tauri 2.0 (`tauri` + `react-ts` + `vite`).
  - Memindahkan engine fullscreen ke background worker thread di Rust (`src-tauri/src/engine/`).
  - Merancang state thread-safe (`AppState`) untuk mengontrol:
    - Target zoom level vs Current zoom level (animator lerp).
    - Status aktif / nonaktif.
    - Konfigurasi mode panning (*Center Follow* vs *Edge Panning*).
  - Menyediakan Tauri Commands untuk frontend:
    - `zoom_in()` (menaikkan zoom per step, misal +0.25x).
    - `zoom_out()` (menurunkan zoom per step).
    - `reset_zoom()` (kembali ke 1.0x).
    - `set_zoom_level(scale: f32)`.
    - `update_panning_config(...)`.
  - Mengonfigurasi capabilities Tauri 2.0 (`src-tauri/capabilities/default.json`).
* **Output / Definition of Done (DoD):**
  - [ ] Aplikasi Tauri 2.0 dapat dijalankan (`npm run tauri dev`).
  - [ ] Tombol pengujian di React dapat memicu Zoom In, Zoom Out, dan Reset pada seluruh layar dengan transisi yang halus.

---

### Fase 3: Integrasi Sistem Operasi (Global Shortcuts, Scroll Trigger & Tray)
*Fokus: Menyediakan kontrol global cepat yang bisa digunakan saat membuka aplikasi lain.*
* **Aktivitas yang dikerjakan:**
  - Memasang `@tauri-apps/plugin-global-shortcut`.
  - Mendaftarkan kombinasi hotkey fleksibel:
    - Toggle Zoom Instan (misal `Win + Alt + Z` untuk beralih antara 1.0x dan 2.0x).
    - Tombol Zoom Bertahap (misal `Win + Alt + Up / Down` atau `Win + Plus / Minus`).
  - Eksplorasi deteksi modifier key + Mouse Wheel (misal `Ctrl + Alt + Mouse Wheel Up/Down` untuk zoom in/out dinamis di bawah kursor).
  - Mengonfigurasi System Tray di Tauri 2.0 (ikon tray, opsi *Reset Zoom*, *Settings*, *Exit*).
  - Mengatur window utama agar tersembunyi ke Tray saat di-minimize/close.
  - Memasang `@tauri-apps/plugin-single-instance`.
* **Output / Definition of Done (DoD):**
  - [ ] Pengguna dapat men-zoom in/out seluruh layar menggunakan keyboard shortcut atau scroll mouse di aplikasi mana pun.
  - [ ] Aplikasi menetap di System Tray dan dapat dikontrol tanpa harus menampilkan window utama.

---

### Fase 4: Pengembangan UI/UX (React + TS + State Persistence)
*Fokus: Antarmuka dashboard kontrol yang kaya fitur, intuitif, dan responsif.*
* **Aktivitas yang dikerjakan:**
  - Setup Tailwind CSS dan komponen UI (Shadcn UI / Radix Primitives).
  - Membangun halaman pengaturan:
    - **Slider Batas Zoom:** Menentukan rentang zoom (misal Min: 1.0x, Default: 2.0x, Max: 5.0x).
    - **Step Zoom:** Besar kelipatan setiap zoom in/out (0.1x, 0.25x, 0.5x).
    - **Mode Panning:**
      - *Center-locked:* Pointer selalu berada di tengah viewport.
      - *Edge Panning / Deadzone:* Viewport baru bergeser saat pointer mendekati area tepi layar.
    - **Kecepatan Animasi Zoom & Panning:** Slider untuk mengatur responsivitas lerp (instan hingga sangat halus/cinematic).
    - **Shortcut Configurator:** UI untuk merekam kombinasi tombol kustom pengguna.
    - **Toggle Start on Boot:** Menggunakan `@tauri-apps/plugin-autostart`.
  - Integrasi `@tauri-apps/plugin-store` untuk persistensi preferensi ke disk (JSON).
* **Output / Definition of Done (DoD):**
  - [ ] Tampilan UI React TS yang modern dan mendukung Dark/Light mode.
  - [ ] Seluruh perubahan pengaturan di UI langsung berdampak pada perilaku fullscreen zoom dan tersimpan permanen.

---

### Fase 5: Input Mapping, Multi-Monitor & DPI Scaling
*Fokus: Memastikan klik mouse akurat saat layar diperbesar dan kompatibel dengan setup monitor yang beragam.*
* **Aktivitas yang dikerjakan:**
  - **Akurasi Klik Mouse (Input Coordinate Mapping):**  
    Ketika layar diperbesar secara penuh via `MagSetFullscreenTransform`, memeriksa dan menangani pemetaan klik agar klik mouse tetap mengenai target yang benar pada aplikasi di bawahnya dengan memanggil `MagSetInputTransform(TRUE, &pRectSource, &pRectDest)` yang disinkronkan dengan transformasi viewport.
    - **Catatan Teknis & Keterbatasan Windows OS:** Pada Windows 10 (1703+) & Windows 11, tanpa `MagSetInputTransform`, OS merutekan klik mouse ke koordinat desktop asli (*unmagnified*), bukan ke posisi visual yang di-zoom (sehingga area seperti baris teratas File Explorer mode *Details* bisa gagal diklik/diseleksi seiring kenaikan faktor zoom). Selain itu, `MagSetInputTransform` **wajib memiliki hak `UIAccess`** (jika tidak, Windows menolak dengan `ERROR_ACCESS_DENIED`, bahkan saat dijalankan sebagai Administrator biasa). Oleh karena itu, pemanggilan `MagSetInputTransform` harus melakukan *graceful fallback* pada build dev lokal (*unsigned*) agar fungsi zoom tetap berjalan normal, sedangkan akurasi klik penuh baru dapat diverifikasi pada build ter-sign dengan `UIAccess` (lihat Fase 6). Komponen WinUI/XAML Islands pada bar atas File Explorer Windows 11 juga memiliki perilaku *hit-testing* bawaan OS di bawah fullscreen magnification yang berada di luar kendali aplikasi.
  - **Per-Monitor DPI Awareness:**  
    Memastikan kalkulasi offset viewport akurat pada display dengan scaling DPI 125%, 150%, 175%, atau 200%.
  - **Multi-Monitor Handling:**  
    Mengatur perilaku zoom saat sistem memiliki lebih dari satu monitor (apakah zoom hanya berlaku di monitor aktif di mana kursor berada, atau mencakup virtual desktop).
  - **Clean Shutdown:**  
    Memastikan saat aplikasi keluar (*exit*), layar otomatis kembali ke 1.0x secara instan dan memanggil `MagUninitialize()`.
* **Output / Definition of Done (DoD):**
  - [ ] Klik mouse pada tombol atau teks saat layar diperbesar tetap presisi (diuji pada build ter-sign dengan `UIAccess` aktif di lokasi instalasi terlindungi; build dev lokal *unsigned* hanya memvalidasi *graceful fallback* tanpa crash).
  - [ ] Berjalan stabil pada monitor high-DPI (laptop 1080p 125% maupun 4K).
  - [ ] Tidak ada distorsi layar saat aplikasi ditutup.

---

### Fase 6: Packaging, Installer & Dokumentasi
*Fokus: Pembuatan installer resmi dan dokumentasi penggunaan.*
* **Aktivitas yang dikerjakan:**
  - Menyiapkan icon pack aplikasi (`.ico`).
  - Mengonfigurasi bundler Tauri 2.0 untuk menghasilkan installer **NSIS (`.exe`)** dan **MSI (`.msi`)**.
  - **Konfigurasi Manifest & Syarat Wajib `UIAccess` (untuk `MagSetInputTransform`):**
    Agar Windows mengizinkan pemetaan klik `MagSetInputTransform` pada build rilis (tidak ditolak dengan `ERROR_ACCESS_DENIED`), distribusi produksi harus memenuhi ketiga syarat keamanan Windows UAC/UIPI secara bersamaan:
    1. Manifest Windows menyertakan `<requestedExecutionLevel level="asInvoker" uiAccess="true" />`.
    2. Binary ditandatangani secara digital (**Authenticode code signing**) dengan sertifikat tepercaya yang berantai ke *Trusted Root* (untuk pengujian lokal dengan *self-signed certificate*, sertifikat perlu dipasang manual ke *Trusted Root* mesin).
    3. Installer (NSIS/MSI) memasang aplikasi ke lokasi terlindungi Windows (`%ProgramFiles%`, `%ProgramFiles(x86)%`, atau `%WinDir%\System32\`), karena menjalankan binary dari folder dev/Desktop/Downloads akan otomatis mencabut hak `UIAccess`.
  - Pengujian instalasi & uninstalasi bersih pada Windows 10 & Windows 11.
  - Pembuatan panduan penggunaan (README / cheatsheet tombol cepat).
* **Output / Definition of Done (DoD):**
  - [ ] Installer standalone (`.exe` / `.msi`) yang teruji dan siap didistribusikan.
  - [ ] Dokumentasi lengkap siap pakai untuk pengguna.

---

### Fase 7: Pointer Sizing Dinamis & Efek Klik (Pendekatan Efisien)
*Fokus: Menambahkan kontrol ukuran pointer berbasis shortcut keyboard dan efek visual saat klik mouse dengan overhead CPU/GPU minimal (zero-idle overhead).*

#### Prinsip Arsitektur Efisien (Hybrid Native Cursor + On-Demand Overlay)
Alih-alih menggambar ulang custom pointer di seluruh layar pada 60+ FPS secara terus-menerus, fase ini memisahkan tanggung jawab menjadi dua mekanisme ringan:
1. **Native OS Cursor Sizing (`CursorBaseSize` + `SystemParametersInfoW`):**  
   Ukuran pointer diubah langsung di level OS Windows sehingga seluruh bentuk kursor kontekstual (*arrow*, *I-beam*, *hand*, *resize*) tetap tajam, berjalan dengan **0% beban render saat idle**, dan otomatis ikut ter-scale oleh `MagSetFullscreenTransform`.
2. **On-Demand Lightweight Click Overlay:**  
   Efek klik (seperti *compressing ring / click pulse* yang memberi sensasi *squash/tekan* atau *ripple* di sekitar ujung pointer) dirender hanya selama animasi berlangsung (~200–350 ms) pada jendela transparan *click-through* berukuran kecil di sekitar titik klik, lalu kembali *idle/hidden* setelah selesai.

* **Aktivitas yang dikerjakan:**
  - **1. Dynamic Pointer Sizing via Win32 API:**
    - Menambahkan modul backend (misal `src-tauri/src/engine/pointer_size.rs`) untuk membaca ukuran awal `CursorBaseSize` di `HKCU\Control Panel\Cursors` sebagai *baseline* saat aplikasi mulai.
    - Mengubah ukuran kursor secara bertahap (misal rentang `32` px hingga `128` px dengan step konfigurabel) dengan memperbarui `CursorBaseSize` dan memicu hot-reload OS secara instan melalui `SystemParametersInfoW` (action `0x2029`, `SPIF_SENDCHANGE`).
    - **Clean Restore & Safety:** Menyimpan nilai *baseline* pengguna dan memastikan ukuran kursor otomatis dikembalikan ke nilai awal saat fitur dimatikan, aplikasi di-*exit*, maupun melalui *console ctrl / drop guard*.
  - **2. Shortcut Keyboard & Pengaturan UI:**
    - Memperluas `ShortcutConfig` dan `ShortcutManager` (`src-tauri/src/shortcuts.rs`) untuk mendukung aksi baru: *Pointer Size Up* (perbesar pointer), *Pointer Size Down* (perkecil pointer), dan *Reset Pointer Size*.
    - Menambahkan konfigurasi ukuran default/step pointer, toggle efek klik, serta perekam shortcut baru di UI React (`Settings` / panel kontrol) dan menyimpannya ke `settings.json`.
  - **3. Deteksi Klik Global pada Hook `WH_MOUSE_LL` yang Sudah Ada:**
    - Memperluas `low_level_mouse_proc` di `src-tauri/src/engine/wheel_hook.rs` agar ikut memantau pesan klik mouse (`WM_LBUTTONDOWN`, `WM_LBUTTONUP`, `WM_RBUTTONDOWN`, `WM_RBUTTONUP`) tanpa memblokir event (`CallNextHookEx` tetap selalu dipanggil) dan mengirim event secara non-blocking (`try_send`) ke thread efek klik.
  - **4. On-Demand Click Effect Overlay (Efek Squash Ring / Ripple):**
    - Membuat jendela overlay transparan *click-through* (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`) berukuran lokal kecil (misal ~128×128 px mengikuti titik klik & DPI monitor aktif, bukan kanvas layar penuh).
    - Merender animasi efek klik berbasis waktu (*pressed/squash-in* saat `WM_LBUTTONDOWN` dan *spring-release/ripple-out* saat `WM_LBUTTONUP`) menggunakan GDI+ / Direct2D + `UpdateLayeredWindow`, lalu menyembunyikan jendela & menghentikan render loop begitu animasi selesai.
    - Menyelaraskan posisi dan skala visual efek klik dengan transformasi `FullscreenMagnifier` saat mode zoom aktif, serta memastikan kompatibilitas *Per-Monitor DPI* dan *multi-monitor* (termasuk koordinat virtual desktop negatif).
  - **5. Opsi Lanjutan (Opsional / Future Extension):**
    - Jika ke depannya dibutuhkan efek *squash & stretch* langsung pada badan ikon panahnya sendiri (bukan di sekitar kursor), tambahkan sebagai mode opsional terpisah (*Presentation Custom Pointer*) agar mode standar tetap super ringan.
* **Output / Definition of Done (DoD):**
  - [ ] Pointer dapat diperbesar dan diperkecil secara instan lewat shortcut keyboard di aplikasi mana pun, dan otomatis kembali ke ukuran awal saat aplikasi ditutup.
  - [ ] Klik kiri/kanan memicu efek visual responsif (*pressed/squash pulse* atau *ripple*) tepat di ujung pointer tanpa menyebabkan *input lag* atau memblokir klik ke aplikasi di bawahnya.
  - [ ] Penggunaan CPU dan memori tetap mendekati nol saat mouse tidak sedang diklik (*zero-idle render overhead*).
  - [ ] Bekerja akurat baik pada skala normal (`1.0x`) maupun saat layar sedang di-zoom (`MagSetFullscreenTransform`), di setup multi-monitor & mixed-DPI.

---