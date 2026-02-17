<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { generateImage } from './api';
  import { generateMockTile } from './mock_api';
  import { t } from '../lib/i18n';

  const dispatch = createEventDispatcher();

  export let src: string;
  export let rows: number;
  export let cols: number;
  export let overlap: number;
  export let isProcessing: boolean;
  export let aiOutputRes: number;
  export let bgRemovalEnabled: boolean;
  export let keyColor: string;
  export let tolerance: number = 10;
  export let concurrency: number = 2;
  export let resultSrc: string = '';
  export let showTileLines: boolean = true;
  export let isAdjustingGrid: boolean = false;
  export let showOriginalInput: boolean = false;
  export let detectedSubject: string = '';
  export let exportTiles: any[] = [];

  let container: HTMLDivElement;
  let imgElement: HTMLImageElement;
  let displaySrc = '';
  let resultPreviewSrc = '';
  let resultPreviewObjectUrl: string | null = null;
  let previewBuildId = 0;
  let isSplitting = false;
  let isMerging = false;
  let hoveredTileIndex: number | null = null;
  let zoom = 1;
  let isPanning = false;
  let panStartX = 0;
  let panStartY = 0;
  let panStartScrollLeft = 0;
  let panStartScrollTop = 0;
  let activePointerId: number | null = null;
  let isSpacePressed = false;
  let prevSrc = '';
  let pendingFitOnLoad = true;
  let cachedFullImageBlob: Blob | null = null;
  let cachedFullImageSrc = '';
  let containerW = 0;
  let containerH = 0;
  let contentW = 0;
  let contentH = 0;
  let surfaceW = 0;
  let surfaceH = 0;
  let contentLeft = 0;
  let contentTop = 0;
  const MIN_ZOOM = 0.01;
  const MAX_ZOOM = 8;
  const ZOOM_STEP = 1.15;
  const DEFAULT_PROMPT_TEMPLATE_WITH_REFERENCE =
    `Task: Generate one tile from a larger image.\n` +
    `Main subject: {subject}\n` +
    `Preserve the main subject exactly as-is. Do not change subject shape, geometry, pose, colors, materials, logos, or text.\n` +
    `Background rule: {background_instruction}\n` +
    `Background must be a single flat color only, with clean edges and absolutely no shadows, gradients, reflections, glow, or texture.\n` +
    `Tile position: row {tile_row}/{tile_rows}, column {tile_col}/{tile_cols}.\n` +
    `Use the full-image reference to keep composition, subject scale, and global consistency across neighboring tiles.\n` +
    `Return only the generated tile image.`;
  const DEFAULT_PROMPT_TEMPLATE_WITHOUT_REFERENCE =
    `Task: Generate one tile from a larger image.\n` +
    `Main subject: {subject}\n` +
    `Preserve the main subject exactly as-is. Do not change subject shape, geometry, pose, colors, materials, logos, or text.\n` +
    `Background rule: {background_instruction}\n` +
    `Background must be a single flat color only, with clean edges and absolutely no shadows, gradients, reflections, glow, or texture.\n` +
    `Return only the generated tile image.`;
  
  // Grid visualization state
  let tiles: any[] = [];
  
  // Result state
  let tempDir = '';
  let originalW = 0;
  let originalH = 0;

  // Track previous settings to avoid infinite loops in reactive re-merge
  let prevBG = bgRemovalEnabled;
  let prevKey = keyColor;
  let prevTol = tolerance;
  let hasActiveWorkers = false;
  
  $: if (src) {
    loadImage(src);
  }

  $: buildResultPreview(resultSrc);

  $: if (src && src !== prevSrc) {
    prevSrc = src;
    pendingFitOnLoad = true;
    cachedFullImageBlob = null;
    cachedFullImageSrc = '';
  }
  
  $: if ((bgRemovalEnabled !== prevBG || keyColor !== prevKey || tolerance !== prevTol) && tiles.length > 0 && tiles.some(t => t.status === 'done') && !isProcessing && !isMerging) {
      prevBG = bgRemovalEnabled;
      prevKey = keyColor;
      prevTol = tolerance;
      mergeAll();
  }
  $: hasActiveWorkers = tiles.some((t) => t.status === 'processing');
  $: exportTiles = tiles
    .filter((tile) => tile.path || tile.originalPath)
    .map((tile) => ({
      r: tile.r,
      c: tile.c,
      x: Math.round(tile.x),
      y: Math.round(tile.y),
      width: Math.round(tile.w),
      height: Math.round(tile.h),
      path: tile.path || '',
      originalPath: tile.originalPath || '',
      status: tile.status || 'pending'
    }));
  $: contentW = Math.max(1, originalW * zoom);
  $: contentH = Math.max(1, originalH * zoom);
  $: surfaceW = Math.max(contentW, containerW);
  $: surfaceH = Math.max(contentH, containerH);
  $: contentLeft = (surfaceW - contentW) / 2;
  $: contentTop = (surfaceH - contentH) / 2;
  $: if (pendingFitOnLoad && containerW > 0 && containerH > 0 && originalW > 0 && originalH > 0) {
    fitToViewport();
    pendingFitOnLoad = false;
  }

  async function loadImage(path: string) {
    try {
      const b64 = await invoke('load_image', { path });
      displaySrc = b64 as string;
      cachedFullImageBlob = null;
      cachedFullImageSrc = '';
    } catch (e) {
      console.error("Failed to load image:", e);
    }
  }

  function getPromptTemplate(useFullImageReference: boolean): string {
    const legacyTemplate =
      localStorage.getItem('gemini_prompt_template') ||
      localStorage.getItem('gemini_prompt') ||
      '';
    if (useFullImageReference) {
      return (
        localStorage.getItem('gemini_prompt_template_with_reference') ||
        legacyTemplate ||
        DEFAULT_PROMPT_TEMPLATE_WITH_REFERENCE
      );
    }
    return (
      localStorage.getItem('gemini_prompt_template_without_reference') ||
      legacyTemplate ||
      DEFAULT_PROMPT_TEMPLATE_WITHOUT_REFERENCE
    );
  }

  function getApiBaseUrl(): string {
    return localStorage.getItem('gemini_api_url') || 'https://generativelanguage.googleapis.com';
  }

  function isVerboseLoggingEnabled(): boolean {
    return localStorage.getItem('verbose_logging') === 'true';
  }

  function isFullImageReferenceEnabled(): boolean {
    return localStorage.getItem('use_full_image_reference') === 'true';
  }

  function renderPromptTemplate(template: string, context: Record<string, string>): string {
    return template.replace(/\{([a-zA-Z0-9_]+)\}/g, (_match, key) => {
      return context[key] ?? '';
    });
  }

  function getKeyColorBackgroundInstruction(color: string): string {
    const c = color.toLowerCase();
    if (!bgRemovalEnabled) {
      return 'Remove background to solid pure white (#FFFFFF). No shadows or gradients.';
    }
    if (c === 'white') {
      return 'Remove background to solid pure white (#FFFFFF). No shadows or gradients.';
    }
    if (c === 'black') {
      return 'Remove background to solid pure black (#000000). No shadows or gradients.';
    }
    if (c === 'red') {
      return 'Remove background to solid pure red (#FF0000). No shadows or gradients.';
    }
    if (c === 'blue') {
      return 'Remove background to solid pure blue (#0000FF). No shadows or gradients.';
    }
    return 'Remove background to solid pure green (#00FF00). No shadows or gradients.';
  }

  function getReferencePromptInstruction(useFullImageReference: boolean): string {
    if (useFullImageReference) {
      return 'Use the provided full-image reference to keep composition, subject scale, and global consistency across neighboring tiles.';
    }
    return '';
  }

  function getTilePositionInstruction(tile: any, useFullImageReference: boolean): string {
    if (!useFullImageReference) return '';
    return `Tile position: row ${tile.r + 1}/${rows}, column ${tile.c + 1}/${cols}.`;
  }

  function buildPromptForTile(tile: any): string {
    const useFullImageReference = isFullImageReferenceEnabled();
    const template = getPromptTemplate(useFullImageReference);
    const context: Record<string, string> = {
      subject: detectedSubject || 'main subject',
      background_instruction: getKeyColorBackgroundInstruction(keyColor),
      tile_position_instruction: getTilePositionInstruction(tile, useFullImageReference),
      reference_instruction: getReferencePromptInstruction(useFullImageReference),
      key_color: bgRemovalEnabled ? keyColor : 'white',
      tile_row: String(tile.r + 1),
      tile_col: String(tile.c + 1),
      tile_rows: String(rows),
      tile_cols: String(cols),
      tile_width: String(Math.round(tile.w)),
      tile_height: String(Math.round(tile.h)),
      image_width: String(Math.round(originalW)),
      image_height: String(Math.round(originalH))
    };
    let rendered = renderPromptTemplate(template, context)
      .replace(/\n{3,}/g, '\n\n')
      .trim();
    if (!useFullImageReference) {
      rendered = rendered
        .replace(/^\s*Tile position:.*(?:\r?\n|$)/gim, '')
        .replace(/^\s*Reference guidance:.*(?:\r?\n|$)/gim, '')
        .replace(/^\s*Use the full-image reference.*(?:\r?\n|$)/gim, '')
        .replace(/\n{3,}/g, '\n\n')
        .trim();
      return rendered;
    }
    return rendered;
  }

  async function getFullImageBlob(): Promise<Blob | null> {
    if (!displaySrc) return null;
    if (cachedFullImageBlob && cachedFullImageSrc === displaySrc) {
      return cachedFullImageBlob;
    }
    const response = await fetch(displaySrc);
    const blob = await response.blob();
    cachedFullImageBlob = blob;
    cachedFullImageSrc = displaySrc;
    return blob;
  }

  function normalizeMergedImageSrc(value: string): string {
    const trimmed = (value || '').trim();
    if (!trimmed) return '';
    return trimmed.startsWith('data:') ? trimmed : `data:image/png;base64,${trimmed}`;
  }

  type ParsedMergedImage = {
    mime: string;
    base64: string;
    dataUrl: string;
  };

  function parseMergedImage(value: string): ParsedMergedImage | null {
    const normalized = normalizeMergedImageSrc(value);
    if (!normalized) return null;

    const marker = ';base64,';
    const markerIndex = normalized.indexOf(marker);
    if (!normalized.startsWith('data:') || markerIndex < 0) {
      return {
        mime: 'image/png',
        base64: normalized,
        dataUrl: `data:image/png;base64,${normalized}`
      };
    }

    return {
      mime: normalized.slice(5, markerIndex),
      base64: normalized.slice(markerIndex + marker.length),
      dataUrl: normalized
    };
  }

  function base64ToBlob(base64: string, mime: string): Blob {
    let sanitized = (base64 || '').replace(/\s+/g, '').replace(/-/g, '+').replace(/_/g, '/');
    const mod = sanitized.length % 4;
    if (mod) {
      sanitized += '='.repeat(4 - mod);
    }

    const binary = atob(sanitized);
    const chunkSize = 0x8000;
    const chunks: Uint8Array[] = [];
    for (let i = 0; i < binary.length; i += chunkSize) {
      const slice = binary.slice(i, i + chunkSize);
      const bytes = new Uint8Array(slice.length);
      for (let j = 0; j < slice.length; j++) {
        bytes[j] = slice.charCodeAt(j);
      }
      chunks.push(bytes);
    }
    return new Blob(chunks, { type: mime || 'image/png' });
  }

  function cleanupResultPreviewUrl() {
    if (resultPreviewObjectUrl) {
      URL.revokeObjectURL(resultPreviewObjectUrl);
      resultPreviewObjectUrl = null;
    }
  }

  async function buildResultPreview(value: string) {
    previewBuildId += 1;
    const currentBuildId = previewBuildId;

    const parsed = parseMergedImage(value);
    if (!parsed) {
      cleanupResultPreviewUrl();
      resultPreviewSrc = '';
      return;
    }

    try {
      const blob = base64ToBlob(parsed.base64, parsed.mime);
      if (currentBuildId !== previewBuildId) return;
      const url = URL.createObjectURL(blob);
      cleanupResultPreviewUrl();
      resultPreviewObjectUrl = url;
      resultPreviewSrc = url;
    } catch {
      if (currentBuildId !== previewBuildId) return;
      cleanupResultPreviewUrl();
      resultPreviewSrc = parsed.dataUrl;
    }
  }

  function handleMainImageError() {
    if (!showOriginalInput && resultPreviewSrc) {
      resultPreviewSrc = '';
      dispatch('log', { type: 'error', message: 'Failed to render merged preview. Falling back to source image.' });
    }
  }

  function resetView() {
    isPanning = false;
    activePointerId = null;
    fitToViewport();
  }

  function clampZoom(value: number): number {
    return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
  }

  function centerView() {
    if (!container) return;
    requestAnimationFrame(() => {
      if (!container) return;
      const targetX = Math.max(0, (surfaceW - containerW) / 2);
      const targetY = Math.max(0, (surfaceH - containerH) / 2);
      container.scrollLeft = targetX;
      container.scrollTop = targetY;
    });
  }

  function getFitZoom(): number {
    if (originalW <= 0 || originalH <= 0 || containerW <= 0 || containerH <= 0) return 1;
    const fit = Math.min(containerW / originalW, containerH / originalH, 1);
    return clampZoom(fit);
  }

  function fitToViewport() {
    zoom = getFitZoom();
    centerView();
  }

  function zoomByFactor(factor: number, clientX?: number, clientY?: number) {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const targetClientX = clientX ?? (rect.left + rect.width / 2);
    const targetClientY = clientY ?? (rect.top + rect.height / 2);
    const cursorX = targetClientX - rect.left;
    const cursorY = targetClientY - rect.top;

    const nextZoom = clampZoom(zoom * factor);
    if (Math.abs(nextZoom - zoom) < 0.0001) return;

    const currentScrollLeft = container.scrollLeft;
    const currentScrollTop = container.scrollTop;
    const worldX = (currentScrollLeft + cursorX - contentLeft) / zoom;
    const worldY = (currentScrollTop + cursorY - contentTop) / zoom;

    zoom = nextZoom;

    requestAnimationFrame(() => {
      if (!container) return;
      const nextContentW = Math.max(1, originalW * zoom);
      const nextContentH = Math.max(1, originalH * zoom);
      const nextSurfaceW = Math.max(nextContentW, containerW);
      const nextSurfaceH = Math.max(nextContentH, containerH);
      const nextContentLeft = (nextSurfaceW - nextContentW) / 2;
      const nextContentTop = (nextSurfaceH - nextContentH) / 2;

      const desiredLeft = worldX * zoom + nextContentLeft - cursorX;
      const desiredTop = worldY * zoom + nextContentTop - cursorY;

      const maxLeft = Math.max(0, nextSurfaceW - containerW);
      const maxTop = Math.max(0, nextSurfaceH - containerH);
      container.scrollLeft = Math.min(maxLeft, Math.max(0, desiredLeft));
      container.scrollTop = Math.min(maxTop, Math.max(0, desiredTop));
    });
  }

  function handleWheel(event: WheelEvent) {
    if (!container) return;

    // Trackpad pinch and Ctrl/Cmd+wheel => zoom around cursor.
    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      const factor = Math.exp(-event.deltaY * 0.002);
      zoomByFactor(factor, event.clientX, event.clientY);
      return;
    }
  }

  function handlePointerDown(event: PointerEvent) {
    if (!container) return;
    container.focus();

    // Pan with middle button, or hold Space + left drag.
    const canPan = event.button === 1 || (event.button === 0 && isSpacePressed);
    if (!canPan) return;

    isPanning = true;
    activePointerId = event.pointerId;
    panStartX = event.clientX;
    panStartY = event.clientY;
    panStartScrollLeft = container.scrollLeft;
    panStartScrollTop = container.scrollTop;

    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function handlePointerMove(event: PointerEvent) {
    if (!isPanning || activePointerId !== event.pointerId) return;

    const dx = event.clientX - panStartX;
    const dy = event.clientY - panStartY;
    container.scrollLeft = panStartScrollLeft - dx;
    container.scrollTop = panStartScrollTop - dy;
  }

  function stopPanning(event?: PointerEvent) {
    if (!isPanning) return;
    if (event && activePointerId !== event.pointerId) return;
    isPanning = false;
    activePointerId = null;
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === ' ') {
      isSpacePressed = true;
      event.preventDefault();
      return;
    }

    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      zoomByFactor(ZOOM_STEP);
      return;
    }

    if (event.key === '-' || event.key === '_') {
      event.preventDefault();
      zoomByFactor(1 / ZOOM_STEP);
      return;
    }

    if ((event.key === '0' && (event.ctrlKey || event.metaKey)) || event.key === 'Home') {
      event.preventDefault();
      resetView();
      return;
    }

    const panStep = event.shiftKey ? 120 : 40;
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      container.scrollLeft -= panStep;
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      container.scrollLeft += panStep;
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      container.scrollTop -= panStep;
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      container.scrollTop += panStep;
    }
  }

  function handleKeyUp(event: KeyboardEvent) {
    if (event.key === ' ') {
      isSpacePressed = false;
      event.preventDefault();
    }
  }

  function handleCanvasBlur() {
    isSpacePressed = false;
    isPanning = false;
    activePointerId = null;
  }
  
  $: if (rows && cols && overlap >= 0 && imgElement) {
    calculateGrid();
  }

  // Effect: When isProcessing becomes true, start processing
  $: if (isProcessing) {
    processAll();
  }

  async function calculateGrid() {
    if (!imgElement || !imgElement.complete) return;
    
    const w = imgElement.naturalWidth;
    const h = imgElement.naturalHeight;
    originalW = w;
    originalH = h;
    
    // Calculate tile dimensions
    const tileW = w / (cols - (cols - 1) * overlap);
    const tileH = h / (rows - (rows - 1) * overlap);
    
    const overlapW = tileW * overlap;
    const overlapH = tileH * overlap;
    
    tiles = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const x = c * (tileW - overlapW);
        const y = r * (tileH - overlapH);
        tiles.push({
          r, c, x, y, w: tileW, h: tileH,
          status: 'pending',
          path: '',          // Target path for results
          originalPath: ''   // Source path for input
        });
      }
    }
  }

  async function processSingleTile(index: number) {
    const tile = tiles[index];
    if (!tile) return;

    tiles[index].status = 'processing';
    tiles = [...tiles];

    try {
        const operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
        let resultBlob: Blob;

        if (operationMode === 'mock') {
            resultBlob = await generateMockTile(aiOutputRes, aiOutputRes, tile.r, tile.c);
            await new Promise(r => setTimeout(r, 200));
        } else {
            const apiKey = localStorage.getItem('gemini_api_key');
            const model = localStorage.getItem('gemini_model') || 'gemini-2.5-flash-image'; 
            const apiBaseUrl = getApiBaseUrl();
            
            if (!apiKey) throw new Error("API Key not found. Please set it in Settings.");

            let prompt = buildPromptForTile(tile);
            const useFullImageReference = isFullImageReferenceEnabled();
            
            if (bgRemovalEnabled) {
              prompt += `\nKeying color selected: ${keyColor}.`;
            }

            let inputBlob: Blob | null = null;
            let fullImageBlob: Blob | null = null;

            if (operationMode === 'test_t2i') {
                prompt = `Generate a beautiful scenery with a big, black text saying '(${tile.r},${tile.c})' in the center.`;
            } else {
                if (!tile.originalPath) {
                    throw new Error(`Original tile source missing for ${tile.r},${tile.c}. Please split again.`);
                }
                // Read via Rust to bypass scope restrictions
                const b64Data = await invoke('load_image', { path: tile.originalPath }) as string;
                const res = await fetch(b64Data);
                inputBlob = await res.blob();
                fullImageBlob = useFullImageReference ? await getFullImageBlob() : null;
            }
            
            // API Call
            if (isVerboseLoggingEnabled()) {
              const escapedPrompt = prompt.replace(/\n/g, '\\n');
              dispatch('log', {
                type: 'info',
                message: `[Prompt ${tile.r},${tile.c}] ${escapedPrompt}`
              });
            }
            resultBlob = await generateImage(inputBlob, prompt, model, apiKey, {
              apiBaseUrl,
              fullImageBlob: useFullImageReference ? fullImageBlob : null
            });
        }
        
        // Save result
        const reader = new FileReader();
        reader.readAsDataURL(resultBlob); 
        const resultB64 = await new Promise<string>(resolve => {
             reader.onloadend = () => resolve(reader.result as string);
        });
        
        await invoke('save_image_resized', { path: tile.path, base64Data: resultB64, width: Math.round(tile.w), height: Math.round(tile.h) });
        
        tiles[index].status = 'done';
        dispatch('log', { type: 'success', message: `Tile ${tile.r},${tile.c} processed.` });
    } catch (e: any) {
        console.error(`Error processing tile ${tile.r},${tile.c}`, e);
        tiles[index].status = 'error';
        dispatch('log', { type: 'error', message: `Tile ${tile.r},${tile.c}: ${e.message || e}` });
    } finally {
        tiles = [...tiles];
    }
  }

  async function mergeAll() {
      const updatePayload = tiles.map((t: any) => ({
        r: t.r,
        c: t.c,
        path: t.path
      }));
      
      dispatch('log', { type: 'info', message: 'Merging tiles...' });
      isMerging = true;
      try {
        const mergedB64: string = await invoke('merge_img', {
            tiles: updatePayload,
            originalW: Math.round(originalW),
            originalH: Math.round(originalH),
            overlapRatio: overlap,
            keyColor: bgRemovalEnabled ? keyColor : 'white',
            removeBg: bgRemovalEnabled,
            tolerance: tolerance
        });
        
        resultSrc = normalizeMergedImageSrc(mergedB64);
        dispatch('log', { type: 'success', message: 'Processing complete.' });
      } finally {
        isMerging = false;
      }
  }

  async function splitImageAndAssignPaths() {
      dispatch('log', { type: 'info', message: 'Splitting image into tiles...' });
      isSplitting = true;
      try {
        const splitRes: any = await invoke('split_img', {
          path: src,
          rows,
          cols,
          overlapRatio: overlap
        });
        
        tempDir = splitRes.temp_dir;
        
        // If the source image was in a temp dir that was just replaced, update the source
        if (splitRes.new_input_path && splitRes.new_input_path !== src) {
            dispatch('update_src', splitRes.new_input_path);
        }

        let i = 0;
        for (const resTile of splitRes.tiles) {
            if (tiles[i]) {
                tiles[i].x = resTile.x;
                tiles[i].y = resTile.y;
                tiles[i].w = resTile.width;
                tiles[i].h = resTile.height;
                tiles[i].path = resTile.path;
                tiles[i].originalPath = resTile.original_path;
            }
            i++;
        }
        dispatch('log', { type: 'success', message: 'Image split successfully.' });
        return splitRes;
      } finally {
        isSplitting = false;
      }
  }

  async function processAll() {
    try {
      await splitImageAndAssignPaths();
      tiles = [...tiles]; 
      
      // Process with concurrency limit
      const queue = [...tiles.keys()];
      
      const workers = Array(concurrency).fill(null).map(async () => {
          while (queue.length > 0) {
              const index = queue.shift();
              if (index !== undefined) {
                  if (!isProcessing) break;
                  await processSingleTile(index);
              }
          }
      });
      
      await Promise.all(workers);
      
      if (isProcessing) {
         await mergeAll();
      }
      
    } catch (e: any) {
      console.error(e);
      dispatch('log', { type: 'error', message: `Processing failed: ${e.message || e}` });
    } finally {
      isProcessing = false;
    }
  }

  function handleImageLoad() {
    if (resultSrc) return;
    calculateGrid();
    if (pendingFitOnLoad) {
      fitToViewport();
      pendingFitOnLoad = false;
    }
  }
  
  async function regenerateTile(index: number) {
    if (!tiles[index].path || !tiles[index].originalPath) {
        try {
            await splitImageAndAssignPaths();
            tiles = [...tiles];
        } catch (e) {
            alert("Failed to prepare tiles: " + e);
            return;
        }
    }
    await processSingleTile(index);
    await mergeAll();
  }

  onDestroy(() => {
    cleanupResultPreviewUrl();
  });

</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="relative w-full h-full min-w-0 min-h-0 overflow-auto touch-none {isPanning ? 'cursor-grabbing' : (isSpacePressed ? 'cursor-grab' : 'cursor-default')}"
  bind:this={container}
  bind:clientWidth={containerW}
  bind:clientHeight={containerH}
  tabindex="0"
  role="application"
  aria-label="Tile canvas viewport"
  on:wheel={handleWheel}
  on:pointerdown={handlePointerDown}
  on:pointermove={handlePointerMove}
  on:pointerup={stopPanning}
  on:pointercancel={stopPanning}
  on:keydown={handleKeyDown}
  on:keyup={handleKeyUp}
  on:blur={handleCanvasBlur}
>
  {#if displaySrc}
    <div
      class="relative select-none"
      style="width: {surfaceW}px; height: {surfaceH}px;"
    >
    <div
      class="absolute"
      style="left: {contentLeft}px; top: {contentTop}px; width: {contentW}px; height: {contentH}px;"
    >
    <div class="relative inline-block shadow-2xl {bgRemovalEnabled ? 'checkerboard' : ''}">
      <!-- Main Image -->
      <img 
        src={showOriginalInput ? displaySrc : (resultPreviewSrc || displaySrc)} 
        bind:this={imgElement}
        on:load={handleImageLoad}
        on:error={handleMainImageError}
        draggable="false"
        class="block w-full h-full"
        style="object-fit: fill;"
        alt="Source"
      />
      
      <!-- Overlay Grid -->
      {#if tiles.length > 0 && !showOriginalInput && (showTileLines || isAdjustingGrid)}
        <svg class="absolute inset-0 pointer-events-none z-10" viewBox={`0 0 ${originalW} ${originalH}`} preserveAspectRatio="none">
           {#each tiles as tile}
             <rect 
               x={tile.x} y={tile.y} width={tile.w} height={tile.h} 
               fill={isAdjustingGrid ? 'rgba(59, 130, 246, 0.16)' : 'none'}
               stroke={isAdjustingGrid ? 'rgba(37, 99, 235, 0.98)' : (tile.status === 'processing' ? '#f59e0b' : tile.status === 'done' ? (resultSrc ? 'rgba(74, 222, 128, 0.3)' : '#4ade80') : tile.status === 'error' ? '#ef4444' : 'rgba(255, 255, 255, 0.5)')}
               stroke-width={isAdjustingGrid ? "1.25" : (resultSrc ? "1" : "2")}
               vector-effect="non-scaling-stroke"
             />
           {/each}
        </svg>
      {/if}
      
      <!-- Interactive Layer -->
      {#if tiles.length > 0 && !showOriginalInput}
         <div class="absolute inset-0 z-20">
           {#if isSplitting || isMerging}
             <div class="absolute inset-0 bg-black/50 flex flex-col items-center justify-center z-30 backdrop-blur-[1px]">
               <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-white mb-4"></div>
               <span class="text-white font-bold bg-black/50 px-3 py-1 rounded">
                 {isSplitting ? 'Splitting Image...' : 'Merging Tiles...'}
               </span>
             </div>
           {/if}
           
           {#each tiles as tile, index}
             <!-- svelte-ignore a11y_no_static_element_interactions -->
             <div 
               class="absolute group transition-all duration-200 border flex items-center justify-center cursor-default overflow-hidden {hoveredTileIndex === index ? 'border-blue-400 bg-blue-500/30' : 'border-transparent hover:border-blue-400 hover:bg-blue-500/30'}"
               style="left: {tile.x / originalW * 100}%; top: {tile.y / originalH * 100}%; width: {tile.w / originalW * 100}%; height: {tile.h / originalH * 100}%;"
               on:mouseenter={() => hoveredTileIndex = index}
               on:mouseleave={() => hoveredTileIndex = hoveredTileIndex === index ? null : hoveredTileIndex}
             >
                {#if tile.status === 'processing'}
                  <div class="absolute inset-0 bg-black/50 flex items-center justify-center backdrop-blur-[1px]">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
                  </div>
                {:else}
                  {#if isProcessing && hasActiveWorkers && tile.status === 'pending'}
                    <div class="absolute inset-0 bg-black/45"></div>
                  {/if}

                  {#if tile.status === 'done' && isProcessing && hasActiveWorkers}
                    <div class="absolute inset-0 bg-black/35 flex items-center justify-center">
                      <div class="rounded-full bg-green-500/95 ring-2 ring-green-300/70 w-8 h-8 flex items-center justify-center shadow-lg">
                        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                          <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                      </div>
                    </div>
                  {/if}

                  {#if hoveredTileIndex === index}
                  <button 
                    on:click|stopPropagation={() => regenerateTile(index)}
                    class="bg-blue-600 hover:bg-blue-500 text-white text-xs font-bold px-4 py-2 rounded-full shadow-lg transition-all duration-150 ease-out z-40 cursor-pointer disabled:cursor-not-allowed"
                    disabled={isSplitting || isMerging || isProcessing}
                  >
                    {tile.status === 'pending' ? $t('generate') : $t('regenerate')}
                  </button>
                  {/if}
                {/if}
             </div>
           {/each}
         </div>
      {/if}
    </div>
    </div>
    </div>
  {/if}
</div>

<style>
  .checkerboard {
    background-image: linear-gradient(45deg, #ccc 25%, transparent 25%), 
                      linear-gradient(-45deg, #ccc 25%, transparent 25%), 
                      linear-gradient(45deg, transparent 75%, #ccc 75%), 
                      linear-gradient(-45deg, transparent 75%, #ccc 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
    background-color: #eee;
  }
  
  :global(.dark) .checkerboard {
    background-image: linear-gradient(45deg, #333 25%, transparent 25%), 
                      linear-gradient(-45deg, #333 25%, transparent 25%), 
                      linear-gradient(45deg, transparent 75%, #333 75%), 
                      linear-gradient(-45deg, transparent 75%, #333 75%);
    background-color: #222;
  }

</style>
