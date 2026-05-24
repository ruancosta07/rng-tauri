# Cerebrum

> OpenWolf's learning memory. Updated automatically as the AI learns from interactions.
> Do not edit manually unless correcting an error.
> Last updated: 2026-05-23

## User Preferences

- UI theme: black/white, monospace font, minimal aesthetic (no colors other than white/black)
- Tailwind CSS for all frontend styling
- No co-authors in git commits

## Key Learnings

- **Project:** Renomeador de Guias — OCR PDF renamer
- **Description:** Tauri v2 app. User drops folders of PDFs; app uses OCR to extract a numeric code from each PDF and renames/copies it to a `<folder> (renomeado)/` output directory.
- **OCR stack:** `tesseract` CLI + `pdftoppm` (poppler) via `std::process::Command`. System deps required: `tesseract-ocr`, `poppler-utils`.
- **Numeric code format:** pure digits only, 5+ digits (`\b\d{5,}\b` regex). PDFs have exactly 1 page.
- **Tailwind version:** v4 with `@tailwindcss/vite` plugin. Import via `@import "tailwindcss"` in CSS; no `tailwind.config.js` needed.
- **Package manager:** bun (tauri.conf.json uses `bun run dev` and `bun run build`).
- **Drag & drop:** Tauri v2 `getCurrentWindow().onDragDropEvent()` — payload `{ type: "drop", paths: string[] }`.

## Do-Not-Repeat

<!-- Mistakes made and corrected. Each entry prevents the same mistake recurring. -->
<!-- Format: [YYYY-MM-DD] Description of what went wrong and what to do instead. -->

## Decision Log

- **[2026-05-23] OCR via CLI tools, not Rust FFI:** Used `pdftoppm` + `tesseract` system commands instead of Rust FFI crates (`leptess`, `pdfium-render`) — simpler, no FFI complexity, reliable on Linux/CachyOS.
- **[2026-05-23] Background processing with `std::thread::spawn`:** Tauri command returns immediately; processing runs in OS thread. Avoids blocking tokio thread without adding tokio dependency explicitly.
- **[2026-05-23] Progress events:** Rust emits `progress` events via `app.emit()` (Tauri `Emitter` trait); frontend listens with `@tauri-apps/api/event listen()`.
