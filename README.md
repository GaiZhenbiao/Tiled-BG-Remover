<p align="right"> English | <a href="README.zh.md">中文</a> | <a href="README.ja.md">日本語</a></p>

# Tiled BG Remover

Desktop app for high-resolution AI image processing with tile-based workflow.

## Overview
Tiled BG Remover splits a large image into tiles, processes tiles with a Gemini image model, and composites results in the frontend for fast iteration.

It is optimized for:
- Background cleanup to flat color (or keyed removal)
- Large image processing with overlap and seam control
- Region-only regeneration (`Generate In Box`)
- Exporting merged image, tiles, and layered PSD

## Main Features
- Smart grid layout + manual rows/cols
- Adjustable overlap, optional square tiles
- Tile-level generate/regenerate
- Selection-box generation with its own overlay layers
- Layer panel for box overlays: show/hide/delete
- Zoom, pan, scroll, middle-click drag
- Localized UI (`en`, `zh`, `ja`)
- Settings for API key, API URL, model, prompt templates, verbose logs

## Tech Stack
- Frontend: SvelteKit + TypeScript
- Desktop shell: Tauri v2
- Backend: Rust (`src-tauri`)
- AI bridge: Google Gemini API (`src/lib/api.ts`)

## Requirements
- Node.js 18+
- npm 9+
- Rust stable toolchain
- Tauri prerequisites for your OS

## Quick Start
```bash
npm install
npm run tauri dev
```

## Build
```bash
npm run tauri build
```

## Configure Gemini
Open Settings and set:
- `Google AI API Key`
- Optional `API Base URL`
- Model name (default can be changed in Settings)

## Basic Workflow
1. Upload an image.
2. Adjust grid and overlap (or use Smart mode).
3. Click `Process All Tiles` or generate tile-by-tile.
4. Optionally enable `Generate In Box` for local non-destructive edits.
5. Use the box layer panel to manage overlay layers.
6. Export selected outputs (merged image, tiles folder, PSD).

## Project Structure
- `src/routes/+page.svelte`: main app UI
- `src/lib/TileGrid.svelte`: canvas, tiles, box generation, preview compositing
- `src/lib/CropModal.svelte`: frontend crop overlay
- `src/lib/Settings.svelte`: app settings
- `src/lib/api.ts`: Gemini requests and subject detection
- `src-tauri/src/lib.rs`: Tauri commands and export pipeline
- `src-tauri/src/image_processing.rs`: backend image utilities
