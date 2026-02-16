# Agents Development Log & Future Reference

This file serves as a hand-off and reference point for agents working on the **Gemini Tile Upscaler (Tiled-BG-Remover)** project.

## Project Overview
A desktop application (Tauri + Svelte) designed to process high-resolution images by splitting them into overlapping tiles, processing each tile individually via the Gemini AI API (specifically for background removal/upscaling), and merging them back together with seamless blending.

## Technical Stack
- **Frontend:** SvelteKit, Tailwind CSS v4, Lucide Svelte.
- **Backend:** Rust (Tauri v2).
- **Image Processing:** `image` crate (Rust) for tiling and merging.
- **AI Integration:** Google Gemini API (Placeholder logic in `src/lib/api.ts`).

## Key Architectural Decisions

### 1. Tiling & Merging Logic (Rust)
- **Overlap:** Configurable ratio (default 10%).
- **Blending:** Uses a "White as Transparent" strategy for background removal. 
    - If a pixel is pure white (or transparent) in one tile and has content in another, the content is preserved.
    - If both tiles have content, a linear gradient blend is applied to avoid seams.
- **Edge Handling:** Logic aligns tiles to the top-left; edge tiles are handled by intersection to prevent out-of-bounds crashes.

### 2. File Handling
- Uses `tempfile` in Rust to manage tile fragments during processing.
- Frontend uses `convertFileSrc` to display local images and base64 for final results.

### 3. Tauri v2 Migration
- Project was upgraded to Tauri v2 to support modern frontend dependencies.
- Required `tauri-plugin-opener` for basic shell functionality.
- Uses `svelte-kit sync` in the build pipeline to generate TypeScript configurations.

## Critical Files
- `src-tauri/src/image_processing.rs`: Core logic for `split_image`, `merge_tiles`, and `crop_image`.
- `src-tauri/src/lib.rs`: Tauri command definitions and plugin initialization.
- `src/routes/+page.svelte`: Main application UI and orchestration.
- `src/lib/TileGrid.svelte`: Interactive grid visualization and overlay.
- `src/lib/api.ts`: Gemini API integration bridge.

## Operational Instructions

### Local Development
```bash
rustup update stable # Required for Rust 2024 edition crates
npm install
npm run tauri dev
```

### Build & Release
- GitHub Actions is configured in `.github/workflows/build.yml` to build for macOS and Windows.
- Versioning is managed via `tauri.conf.json`.

## Progress Log
- **2026-02-15:** 
    - Initial CLI Python script for tiling created.
    - Full Tauri + Svelte application implemented.
    - Fixed Tailwind v4 / PostCSS build errors.
    - Upgraded to Tauri v2 to fix version mismatches.
    - Added "Center Crop 1:1" tool.
    - Setup GitHub Actions for CI/CD.
    - Initialized `AGENTS.md`.
