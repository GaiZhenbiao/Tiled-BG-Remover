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
  export let overlapXRatio: number = overlap;
  export let overlapYRatio: number = overlap;
  export let isProcessing: boolean;
  export let aiOutputRes: number;
  export let bgRemovalEnabled: boolean;
  export let keyColor: string;
  export let nonBgBackgroundHex: string = '#FFFFFF';
  export let tolerance: number = 10;
  export let concurrency: number = 2;
  export let resultSrc: string = '';
  export let showTileLines: boolean = true;
  export let isAdjustingGrid: boolean = false;
  export let showOriginalInput: boolean = false;
  export let detectedSubject: string = '';
  export let exportTiles: any[] = [];
  export let exportOverlays: any[] = [];
  export let exportRegularLayers: any[] = [];
  export let boxGenerateMode: boolean = false;
  export let boxGenerateAspectRatio: number | null = 1;

  let container: HTMLDivElement;
  let imgElement: HTMLImageElement;
  let effectiveSrc = '';
  let lastPropSrc = '';
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
  let originalW = 0;
  let originalH = 0;

  // Track previous settings to avoid infinite loops in reactive re-merge
  let prevBG = bgRemovalEnabled;
  let prevKey = keyColor;
  let prevTol = tolerance;
  let hasActiveWorkers = false;
  let isRegionProcessing = false;
  let regionOverlays: Array<{ id: number; x: number; y: number; width: number; height: number; dataUrl: string }> = [];
  let compositeRenderSeq = 0;
  const dataUrlImageCache = new Map<string, Promise<HTMLImageElement>>();
  const previewErrorKeys = new Set<string>();
  let isBoxDragging = false;
  let boxDragType = '';
  let boxStartPointerX = 0;
  let boxStartPointerY = 0;
  let boxStartX = 0;
  let boxStartY = 0;
  let boxStartW = 0;
  let boxStartH = 0;
  let boxX = 0;
  let boxY = 0;
  let boxW = 0;
  let boxH = 0;
  let prevBoxGenerateMode = false;
  let prevBoxGenerateAspectRatio: number | null = boxGenerateAspectRatio;
  const MIN_SELECTION_SIZE = 20;
  let hasEditedSelectionBox = false;
  let prevResultSrcState = '';
  let statusActive = false;
  let statusTitle = '';
  let statusDetail = '';
  let statusProgress: number | null = null;
  
  // Sync from parent prop only when the prop value itself changes.
  // This prevents temporary local source updates from being overwritten
  // by a stale parent value during async `update_src` propagation.
  $: if (src && src !== lastPropSrc) {
    lastPropSrc = src;
    effectiveSrc = src;
  }

  $: if (effectiveSrc) {
    loadImage(effectiveSrc);
  }

  $: buildResultPreview(resultSrc);
  $: {
    const current = (resultSrc || '').trim();
    const hadPrevious = prevResultSrcState.length > 0;
    if (hadPrevious && !current && !isProcessing && !isRegionProcessing) {
      clearGeneratedTileState();
    }
    prevResultSrcState = current;
  }

  $: if (effectiveSrc && effectiveSrc !== prevSrc) {
    prevSrc = effectiveSrc;
    pendingFitOnLoad = true;
    cachedFullImageBlob = null;
    cachedFullImageSrc = '';
  }
  $: if (
    boxGenerateMode &&
    originalW > 0 &&
    originalH > 0 &&
    (!prevBoxGenerateMode || prevBoxGenerateAspectRatio !== boxGenerateAspectRatio)
  ) {
    resetSelectionBox();
  }
  $: if (!boxGenerateMode && prevBoxGenerateMode) {
    isBoxDragging = false;
    boxDragType = '';
  }
  $: prevBoxGenerateMode = boxGenerateMode;
  $: prevBoxGenerateAspectRatio = boxGenerateAspectRatio;
  
  $: if ((bgRemovalEnabled !== prevBG || keyColor !== prevKey || tolerance !== prevTol) && tiles.length > 0 && tiles.some(t => t.status === 'done') && !isProcessing && !isMerging) {
      prevBG = bgRemovalEnabled;
      prevKey = keyColor;
      prevTol = tolerance;
      scheduleCompositePreviewRender();
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
  $: exportOverlays = regionOverlays.map((layer, index) => ({
    id: Number(layer.id) || 0,
    x: Math.round(layer.x),
    y: Math.round(layer.y),
    width: Math.max(1, Math.round(layer.width)),
    height: Math.max(1, Math.round(layer.height)),
    dataUrl: layer.dataUrl || '',
    layerOrder: index
  }));
  $: exportRegularLayers = tiles
    .filter((tile) => !!tile.previewDataUrl)
    .map((tile, index) => ({
      r: tile.r,
      c: tile.c,
      x: Math.round(tile.x),
      y: Math.round(tile.y),
      width: Math.max(1, Math.round(tile.w)),
      height: Math.max(1, Math.round(tile.h)),
      dataUrl: tile.previewDataUrl || '',
      layerOrder: Number.isFinite(tile.renderOrder) ? Number(tile.renderOrder) : index
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
      dispatch('log', { type: 'error', message: `Failed to load source image: ${String((e as any)?.message || e)}` });
    }
  }

  async function loadImageFromDataUrl(dataUrl: string): Promise<HTMLImageElement> {
    if (!dataUrl) throw new Error('Missing image data');
    if (!dataUrlImageCache.has(dataUrl)) {
      dataUrlImageCache.set(
        dataUrl,
        new Promise<HTMLImageElement>((resolve, reject) => {
          const img = new Image();
          img.onload = () => resolve(img);
          img.onerror = () => {
            dataUrlImageCache.delete(dataUrl);
            reject(new Error('Failed to load image data URL'));
          };
          img.src = dataUrl;
        })
      );
    }
    return dataUrlImageCache.get(dataUrl)!;
  }

  function logPreviewErrorOnce(key: string, message: string) {
    if (previewErrorKeys.has(key)) return;
    previewErrorKeys.add(key);
    dispatch('log', { type: 'error', message });
  }

  function updateStatus(title: string, detail = '', progress: number | null = null) {
    statusActive = true;
    statusTitle = title;
    statusDetail = detail;
    statusProgress = progress;
  }

  function clearStatus() {
    statusActive = false;
    statusTitle = '';
    statusDetail = '';
    statusProgress = null;
  }

  function clearGeneratedTileState() {
    for (const tile of tiles) {
      if (tile.status !== 'processing') {
        tile.status = 'pending';
      }
      tile.previewDataUrl = '';
      tile.path = '';
      tile.originalPath = '';
      tile.renderOrder = tile.r * 1000 + tile.c;
    }
    regionOverlays = [];
    tiles = [...tiles];
    scheduleCompositePreviewRender();
  }

  function createFeatheredTileCanvas(tile: any, img: HTMLImageElement): HTMLCanvasElement {
    const tileW = Math.max(1, Math.round(tile.w));
    const tileH = Math.max(1, Math.round(tile.h));
    const layer = document.createElement('canvas');
    layer.width = tileW;
    layer.height = tileH;
    const ctx = layer.getContext('2d');
    if (!ctx) return layer;

    ctx.clearRect(0, 0, tileW, tileH);
    ctx.drawImage(img, 0, 0, tileW, tileH);

    const leftFeather = tile.c > 0 ? Math.max(0, Math.round(tileW * overlapXRatio)) : 0;
    const topFeather = tile.r > 0 ? Math.max(0, Math.round(tileH * overlapYRatio)) : 0;
    if (leftFeather <= 0 && topFeather <= 0) {
      return layer;
    }

    ctx.globalCompositeOperation = 'destination-in';
    ctx.fillStyle = 'rgba(255,255,255,1)';
    ctx.fillRect(0, 0, tileW, tileH);

    if (leftFeather > 0) {
      const g = ctx.createLinearGradient(0, 0, leftFeather, 0);
      g.addColorStop(0, 'rgba(255,255,255,0)');
      g.addColorStop(1, 'rgba(255,255,255,1)');
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, leftFeather, tileH);
    }

    if (topFeather > 0) {
      const g = ctx.createLinearGradient(0, 0, 0, topFeather);
      g.addColorStop(0, 'rgba(255,255,255,0)');
      g.addColorStop(1, 'rgba(255,255,255,1)');
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, tileW, topFeather);
    }

    ctx.globalCompositeOperation = 'source-over';
    return layer;
  }

  async function buildTilePreviewsFromComposite(compositeDataUrl: string) {
    if (!compositeDataUrl || tiles.length === 0 || originalW <= 0 || originalH <= 0) return;
    let img: HTMLImageElement;
    try {
      img = await loadImageFromDataUrl(compositeDataUrl);
    } catch (e: any) {
      logPreviewErrorOnce('preview-load-composite', `Failed to build tile previews from composite: ${e?.message || e}`);
      return;
    }
    for (const tile of tiles) {
      const tileW = Math.max(1, Math.round(tile.w));
      const tileH = Math.max(1, Math.round(tile.h));
      const tileX = Math.round(tile.x);
      const tileY = Math.round(tile.y);
      const canvas = document.createElement('canvas');
      canvas.width = tileW;
      canvas.height = tileH;
      const ctx = canvas.getContext('2d');
      if (!ctx) continue;
      ctx.drawImage(img, tileX, tileY, tileW, tileH, 0, 0, tileW, tileH);
      tile.previewDataUrl = canvas.toDataURL('image/jpeg', 0.92);
      tile.status = 'done';
    }
  }

  async function renderCompositePreview() {
    const renderSeq = ++compositeRenderSeq;
    if (originalW <= 0 || originalH <= 0) return;

    const readyTiles = tiles.filter((tile) => !!tile.previewDataUrl);
    const hasLayers = readyTiles.length > 0 || regionOverlays.length > 0;
    if (!hasLayers) {
      if (!isProcessing && !isRegionProcessing) {
        resultSrc = '';
      }
      return;
    }

    const canvas = document.createElement('canvas');
    canvas.width = originalW;
    canvas.height = originalH;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    if (displaySrc) {
      try {
        const baseImg = await loadImageFromDataUrl(displaySrc);
        if (renderSeq !== compositeRenderSeq) return;
        ctx.drawImage(baseImg, 0, 0, originalW, originalH);
      } catch (e: any) {
        logPreviewErrorOnce(
          'preview-base-image',
          `Failed to render base image for composite preview: ${e?.message || e}`
        );
      }
    }

    const sortedTiles = [...readyTiles].sort((a, b) => {
      const ao = Number.isFinite(a.renderOrder) ? a.renderOrder : 0;
      const bo = Number.isFinite(b.renderOrder) ? b.renderOrder : 0;
      if (ao !== bo) return ao - bo;
      return a.r - b.r || a.c - b.c;
    });
    for (const tile of sortedTiles) {
      if (renderSeq !== compositeRenderSeq) return;
      const tileW = Math.max(1, Math.round(tile.w));
      const tileH = Math.max(1, Math.round(tile.h));
      const tileX = Math.round(tile.x);
      const tileY = Math.round(tile.y);
      try {
        const img = await loadImageFromDataUrl(tile.previewDataUrl);
        if (renderSeq !== compositeRenderSeq) return;
        const feathered = createFeatheredTileCanvas(tile, img);
        ctx.globalAlpha = 1;
        ctx.drawImage(feathered, tileX, tileY, tileW, tileH);
      } catch (e: any) {
        logPreviewErrorOnce(
          `preview-tile-${tile.r}-${tile.c}`,
          `Failed to render tile preview ${tile.r},${tile.c}: ${e?.message || e}`
        );
      }
    }
    ctx.globalAlpha = 1;

    for (const layer of regionOverlays) {
      if (renderSeq !== compositeRenderSeq) return;
      try {
        const img = await loadImageFromDataUrl(layer.dataUrl);
        if (renderSeq !== compositeRenderSeq) return;
        ctx.drawImage(
          img,
          Math.round(layer.x),
          Math.round(layer.y),
          Math.max(1, Math.round(layer.width)),
          Math.max(1, Math.round(layer.height))
        );
      } catch (e: any) {
        logPreviewErrorOnce(
          `preview-region-${layer.id}`,
          `Failed to render selection overlay: ${e?.message || e}`
        );
      }
    }

    const mime = bgRemovalEnabled ? 'image/png' : 'image/jpeg';
    resultSrc = canvas.toDataURL(mime, 0.92);
  }

  function scheduleCompositePreviewRender() {
    const seq = ++compositeRenderSeq;
    requestAnimationFrame(() => {
      if (seq !== compositeRenderSeq) return;
      void renderCompositePreview().catch((e: any) => {
        logPreviewErrorOnce('preview-render-failed', `Composite preview rendering failed: ${e?.message || e}`);
      });
    });
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

  function normalizeHexColor(value: string): string {
    const raw = (value || '').trim();
    const match = raw.match(/^#?([0-9a-fA-F]{6})$/);
    if (match) {
      return `#${match[1].toUpperCase()}`;
    }
    return '#FFFFFF';
  }

  function getNonBgColorLabel(hex: string): string {
    if (hex === '#FFFFFF') return 'pure white';
    if (hex === '#000000') return 'pure black';
    if (hex === '#00FF00') return 'pure green';
    if (hex === '#0000FF') return 'pure blue';
    return 'custom color';
  }

  function getKeyColorBackgroundInstruction(color: string): string {
    const c = color.toLowerCase();
    if (!bgRemovalEnabled) {
      const hex = normalizeHexColor(nonBgBackgroundHex);
      const label = getNonBgColorLabel(hex);
      return `Set background to solid ${label} (${hex}). No shadows or gradients.`;
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
      key_color: bgRemovalEnabled ? keyColor : normalizeHexColor(nonBgBackgroundHex),
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

  function ensureImageDataUrl(value: string, fallbackMime = 'image/png'): string {
    const normalized = normalizeMergedImageSrc(value);
    if (!normalized) return '';
    if (normalized.startsWith('data:image/')) return normalized;
    const marker = ';base64,';
    const idx = normalized.indexOf(marker);
    if (idx < 0) return normalized;
    const payload = normalized.slice(idx + marker.length);
    return `data:${fallbackMime};base64,${payload}`;
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

    if (parsed.dataUrl.startsWith('data:image/')) {
      cleanupResultPreviewUrl();
      resultPreviewSrc = parsed.dataUrl;
      return;
    }

    try {
      const blob = base64ToBlob(parsed.base64, parsed.mime);
      if (currentBuildId !== previewBuildId) return;
      const url = URL.createObjectURL(blob);
      cleanupResultPreviewUrl();
      resultPreviewObjectUrl = url;
      resultPreviewSrc = url;
    } catch (e: any) {
      if (currentBuildId !== previewBuildId) return;
      cleanupResultPreviewUrl();
      resultPreviewSrc = parsed.dataUrl;
      logPreviewErrorOnce('preview-result-url-fallback', `Preview blob conversion failed; using data URL fallback: ${e?.message || e}`);
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

  function handleMouseDown(event: MouseEvent) {
    if (!container) return;
    // Fallback for WebView environments where middle-button pointer events are inconsistent.
    if (event.button !== 1) return;

    container.focus();
    isPanning = true;
    activePointerId = null;
    panStartX = event.clientX;
    panStartY = event.clientY;
    panStartScrollLeft = container.scrollLeft;
    panStartScrollTop = container.scrollTop;
    event.preventDefault();
  }

  function handleMouseMove(event: MouseEvent) {
    if (!isPanning || activePointerId !== null) return;

    const dx = event.clientX - panStartX;
    const dy = event.clientY - panStartY;
    container.scrollLeft = panStartScrollLeft - dx;
    container.scrollTop = panStartScrollTop - dy;
  }

  function stopMousePanning() {
    if (!isPanning || activePointerId !== null) return;
    isPanning = false;
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

  function resetSelectionBox() {
    if (originalW <= 0 || originalH <= 0) return;
    const ratio = boxGenerateAspectRatio;
    if (ratio && ratio > 0) {
      let targetW = originalW * 0.6;
      let targetH = targetW / ratio;
      if (targetH > originalH * 0.6) {
        targetH = originalH * 0.6;
        targetW = targetH * ratio;
      }
      if (targetW > originalW) {
        targetW = originalW;
        targetH = targetW / ratio;
      }
      if (targetH > originalH) {
        targetH = originalH;
        targetW = targetH * ratio;
      }
      boxW = Math.max(MIN_SELECTION_SIZE, Math.round(targetW));
      boxH = Math.max(MIN_SELECTION_SIZE, Math.round(targetH));
    } else {
      boxW = Math.max(MIN_SELECTION_SIZE, Math.round(originalW * 0.6));
      boxH = Math.max(MIN_SELECTION_SIZE, Math.round(originalH * 0.6));
    }
    boxX = Math.round((originalW - boxW) / 2);
    boxY = Math.round((originalH - boxH) / 2);
    hasEditedSelectionBox = false;
  }

  function toImagePoint(event: MouseEvent, overlay: HTMLElement) {
    const rect = overlay.getBoundingClientRect();
    const localX = Math.min(Math.max(0, event.clientX - rect.left), rect.width);
    const localY = Math.min(Math.max(0, event.clientY - rect.top), rect.height);
    return {
      x: rect.width > 0 ? (localX / rect.width) * originalW : 0,
      y: rect.height > 0 ? (localY / rect.height) * originalH : 0
    };
  }

  function startSelectionDrag(event: MouseEvent, dragType: string) {
    if (!boxGenerateMode || isSplitting || isMerging || isProcessing || isRegionProcessing) return;
    const sourceEl = event.currentTarget as HTMLElement;
    const overlay =
      sourceEl.dataset.boxOverlay === '1'
        ? sourceEl
        : (sourceEl.closest('[data-box-overlay="1"]') as HTMLElement | null);
    if (!overlay) return;
    const point = toImagePoint(event, overlay);
    isBoxDragging = true;
    boxDragType = dragType;
    boxStartPointerX = point.x;
    boxStartPointerY = point.y;
    boxStartX = boxX;
    boxStartY = boxY;
    boxStartW = boxW;
    boxStartH = boxH;
    event.preventDefault();
    event.stopPropagation();
  }

  function updateSelectionDrag(event: MouseEvent) {
    if (!isBoxDragging) return;
    const overlay = event.currentTarget as HTMLElement;
    const point = toImagePoint(event, overlay);
    const dx = point.x - boxStartPointerX;
    const dy = point.y - boxStartPointerY;
    const ratio = boxGenerateAspectRatio;

    if (boxDragType === 'move') {
      boxX = Math.round(Math.max(0, Math.min(originalW - boxW, boxStartX + dx)));
      boxY = Math.round(Math.max(0, Math.min(originalH - boxH, boxStartY + dy)));
      return;
    }

    if (boxDragType === 'new') {
      let nextX = Math.min(boxStartPointerX, point.x);
      let nextY = Math.min(boxStartPointerY, point.y);
      let nextW = Math.max(MIN_SELECTION_SIZE, Math.abs(dx));
      let nextH = Math.max(MIN_SELECTION_SIZE, Math.abs(dy));

      if (ratio && ratio > 0) {
        if (nextW / nextH > ratio) {
          nextH = nextW / ratio;
        } else {
          nextW = nextH * ratio;
        }
        if (point.x < boxStartPointerX) {
          nextX = boxStartPointerX - nextW;
        }
        if (point.y < boxStartPointerY) {
          nextY = boxStartPointerY - nextH;
        }
      }

      if (nextX < 0) nextX = 0;
      if (nextY < 0) nextY = 0;
      if (nextX + nextW > originalW) nextW = originalW - nextX;
      if (nextY + nextH > originalH) nextH = originalH - nextY;

      boxX = Math.round(nextX);
      boxY = Math.round(nextY);
      boxW = Math.round(Math.max(MIN_SELECTION_SIZE, nextW));
      boxH = Math.round(Math.max(MIN_SELECTION_SIZE, nextH));
      return;
    }

    let nextX = boxStartX;
    let nextY = boxStartY;
    let nextW = boxStartW;
    let nextH = boxStartH;

    if (boxDragType.includes('e')) {
      nextW = Math.max(MIN_SELECTION_SIZE, boxStartW + dx);
    }
    if (boxDragType.includes('s')) {
      nextH = Math.max(MIN_SELECTION_SIZE, boxStartH + dy);
    }
    if (boxDragType.includes('w')) {
      nextW = Math.max(MIN_SELECTION_SIZE, boxStartW - dx);
      nextX = boxStartX + boxStartW - nextW;
    }
    if (boxDragType.includes('n')) {
      nextH = Math.max(MIN_SELECTION_SIZE, boxStartH - dy);
      nextY = boxStartY + boxStartH - nextH;
    }

    if (ratio && ratio > 0) {
      if (nextW / nextH > ratio) {
        nextH = nextW / ratio;
      } else {
        nextW = nextH * ratio;
      }
      if (boxDragType.includes('w')) {
        nextX = boxStartX + boxStartW - nextW;
      }
      if (boxDragType.includes('n')) {
        nextY = boxStartY + boxStartH - nextH;
      }
    }

    if (nextX < 0) {
      nextX = 0;
      if (ratio && ratio > 0) nextW = boxStartX + boxStartW;
    }
    if (nextY < 0) {
      nextY = 0;
      if (ratio && ratio > 0) nextH = boxStartY + boxStartH;
    }
    if (nextX + nextW > originalW) {
      nextW = originalW - nextX;
      if (ratio && ratio > 0) nextH = nextW / ratio;
    }
    if (nextY + nextH > originalH) {
      nextH = originalH - nextY;
      if (ratio && ratio > 0) nextW = nextH * ratio;
    }

    boxX = Math.round(nextX);
    boxY = Math.round(nextY);
    boxW = Math.round(Math.max(MIN_SELECTION_SIZE, nextW));
    boxH = Math.round(Math.max(MIN_SELECTION_SIZE, nextH));
  }

  function stopSelectionDrag() {
    if (isBoxDragging) {
      hasEditedSelectionBox = true;
    }
    isBoxDragging = false;
    boxDragType = '';
  }

  function cancelBoxGenerateMode() {
    stopSelectionDrag();
    dispatch('box_generate_mode_change', false);
  }

  function isTileIntersectingSelection(tile: any): boolean {
    const x1 = tile.x;
    const y1 = tile.y;
    const x2 = tile.x + tile.w;
    const y2 = tile.y + tile.h;
    const bx1 = boxX;
    const by1 = boxY;
    const bx2 = boxX + boxW;
    const by2 = boxY + boxH;
    return x1 < bx2 && x2 > bx1 && y1 < by2 && y2 > by1;
  }

  $: if (
    rows &&
    cols &&
    overlapXRatio >= 0 &&
    overlapYRatio >= 0 &&
    imgElement &&
    !isProcessing &&
    !isSplitting &&
    !isMerging &&
    !isRegionProcessing
  ) {
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
    const tileW = w / (cols - (cols - 1) * overlapXRatio);
    const tileH = h / (rows - (rows - 1) * overlapYRatio);
    
    const overlapW = tileW * overlapXRatio;
    const overlapH = tileH * overlapYRatio;
    
    const previousByKey = new Map<string, any>();
    for (const tile of tiles) {
      previousByKey.set(`${tile.r},${tile.c}`, tile);
    }

    tiles = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const x = c * (tileW - overlapW);
        const y = r * (tileH - overlapH);
        const prev = previousByKey.get(`${r},${c}`);
        tiles.push({
          r, c, x, y, w: tileW, h: tileH,
          status: prev?.status || 'pending',
          path: '',          // Target path for results
          originalPath: '',   // Source path for input
          previewDataUrl: prev?.previewDataUrl || '',
          renderOrder: prev?.renderOrder ?? (r * 1000 + c)
        });
      }
    }
  }

  function getTileRect(tile: any) {
    const sourceW = Math.max(1, Math.round(originalW));
    const sourceH = Math.max(1, Math.round(originalH));
    const x = Math.max(0, Math.min(sourceW - 1, Math.round(tile.x)));
    const y = Math.max(0, Math.min(sourceH - 1, Math.round(tile.y)));
    const width = Math.max(1, Math.min(Math.round(tile.w), sourceW - x));
    const height = Math.max(1, Math.min(Math.round(tile.h), sourceH - y));
    return { x, y, width, height };
  }

  async function cropTileInputDataUrl(tile: any, preferJpeg: boolean): Promise<string> {
    if (!displaySrc) {
      throw new Error('Source image is not loaded.');
    }
    const sourceImage = await loadImageFromDataUrl(displaySrc);
    const rect = getTileRect(tile);
    const canvas = document.createElement('canvas');
    canvas.width = rect.width;
    canvas.height = rect.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Failed to create tile canvas context.');
    }
    ctx.drawImage(
      sourceImage,
      rect.x,
      rect.y,
      rect.width,
      rect.height,
      0,
      0,
      rect.width,
      rect.height
    );
    return canvas.toDataURL(preferJpeg ? 'image/jpeg' : 'image/png', 0.92);
  }

  async function cropRegionInputDataUrl(
    x: number,
    y: number,
    width: number,
    height: number,
    preferJpeg: boolean
  ): Promise<string> {
    if (!displaySrc) {
      throw new Error('Source image is not loaded.');
    }
    const sourceImage = await loadImageFromDataUrl(displaySrc);
    const sourceW = Math.max(1, Math.round(originalW));
    const sourceH = Math.max(1, Math.round(originalH));
    const safeX = Math.max(0, Math.min(sourceW - 1, Math.round(x)));
    const safeY = Math.max(0, Math.min(sourceH - 1, Math.round(y)));
    const safeW = Math.max(1, Math.min(Math.round(width), sourceW - safeX));
    const safeH = Math.max(1, Math.min(Math.round(height), sourceH - safeY));
    const canvas = document.createElement('canvas');
    canvas.width = safeW;
    canvas.height = safeH;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Failed to create region canvas context.');
    }
    ctx.drawImage(sourceImage, safeX, safeY, safeW, safeH, 0, 0, safeW, safeH);
    return canvas.toDataURL(preferJpeg ? 'image/jpeg' : 'image/png', 0.92);
  }

  function readPreparedValue(prepared: any, camelKey: string, snakeKey: string): string {
    const value = prepared?.[camelKey] ?? prepared?.[snakeKey] ?? '';
    return typeof value === 'string' ? value : '';
  }

  async function ensureTilePrepared(index: number, includeInputData: boolean): Promise<string> {
    const tile = tiles[index];
    if (!tile) {
      throw new Error(`Tile ${index} not found.`);
    }
    const rect = getTileRect(tile);
    const tileDataUrl = await cropTileInputDataUrl(tile, true);
    const prepared = (await invoke('prepare_tile_from_data_url', {
      tileBase64: tileDataUrl,
      row: tile.r,
      col: tile.c,
      width: rect.width,
      height: rect.height,
      preferJpeg: true
    })) as any;

    const outputPath = readPreparedValue(prepared, 'outputPath', 'output_path');
    const originalPath = readPreparedValue(prepared, 'originalPath', 'original_path');
    const inputDataUrl = readPreparedValue(prepared, 'inputDataUrl', 'input_data_url');

    if (!outputPath || !originalPath) {
      throw new Error(`Backend did not return valid tile paths for ${tile.r},${tile.c}.`);
    }

    tile.path = outputPath;
    tile.originalPath = originalPath;

    if (!includeInputData) {
      return '';
    }
    if (!inputDataUrl) {
      throw new Error(`Backend did not return tile input data for ${tile.r},${tile.c}.`);
    }
    return ensureImageDataUrl(inputDataUrl, 'image/jpeg');
  }

  async function processSingleTile(index: number) {
    const tile = tiles[index];
    if (!tile) return;

    tiles[index].status = 'processing';
    tiles = [...tiles];

    try {
        const operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
        let resultBlob: Blob;
        const preparedInputDataUrl = await ensureTilePrepared(index, operationMode !== 'test_t2i');

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
                const res = await fetch(preparedInputDataUrl);
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
        const outputPath = tile.path || '';
        if (!outputPath) {
          throw new Error(`Tile output path missing for ${tile.r},${tile.c}.`);
        }
        await invoke('save_image_resized', {
          path: outputPath,
          base64Data: resultB64,
          width: Math.round(tile.w),
          height: Math.round(tile.h)
        });
        if (!tile.path) {
          tile.path = outputPath;
        }
        try {
          const refreshedPreview = (await invoke('load_image', { path: outputPath })) as string;
          tile.previewDataUrl = ensureImageDataUrl(
            refreshedPreview,
            bgRemovalEnabled ? 'image/png' : 'image/jpeg'
          );
        } catch (previewLoadErr: any) {
          tile.previewDataUrl = ensureImageDataUrl(resultB64, bgRemovalEnabled ? 'image/png' : 'image/jpeg');
          dispatch('log', {
            type: 'error',
            message: `Tile preview load fallback for ${tile.r},${tile.c}: ${previewLoadErr?.message || previewLoadErr}`
          });
        }
        tile.renderOrder = Date.now();
        
        tiles[index].status = 'done';
        dispatch('log', { type: 'success', message: `Tile ${tile.r},${tile.c} processed.` });
    } catch (e: any) {
        console.error(`Error processing tile ${tile.r},${tile.c}`, e);
        tiles[index].status = 'error';
        dispatch('log', { type: 'error', message: `Tile ${tile.r},${tile.c}: ${e.message || e}` });
    } finally {
        tiles = [...tiles];
        scheduleCompositePreviewRender();
    }
  }

  async function runTileQueue(
    queue: number[],
    shouldStopWhenProcessingOff: boolean,
    statusLabel: string
  ) {
      const pending = [...queue];
      const workerCount = Math.max(1, concurrency);
      const total = pending.length;
      let completed = 0;
      updateStatus(statusLabel, `0/${total}`, 0);
      const workers = Array(workerCount).fill(null).map(async () => {
          while (pending.length > 0) {
              const index = pending.shift();
              if (index === undefined) continue;
              if (shouldStopWhenProcessingOff && !isProcessing) break;
              const tile = tiles[index];
              updateStatus(
                statusLabel,
                `Tile ${tile?.r ?? 0},${tile?.c ?? 0} (${Math.min(completed + 1, total)}/${total})`,
                total > 0 ? Math.round((completed / total) * 100) : 0
              );
              await processSingleTile(index);
              completed += 1;
              updateStatus(
                statusLabel,
                `Completed ${completed}/${total}`,
                total > 0 ? Math.round((completed / total) * 100) : 100
              );
          }
      });
      await Promise.all(workers);
  }

  async function processAll() {
    try {
      const queue = tiles.map((_tile, index) => index);
      if (queue.length === 0) {
        throw new Error('No tiles available. Please adjust overlap/grid settings.');
      }
      await runTileQueue(queue, true, 'Processing tiles...');
      
      if (isProcessing) {
         scheduleCompositePreviewRender();
         dispatch('log', { type: 'success', message: 'Processing complete.' });
      }
      
    } catch (e: any) {
      console.error(e);
      dispatch('log', { type: 'error', message: `Processing failed: ${e.message || e}` });
    } finally {
      isProcessing = false;
      clearStatus();
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
    const tile = tiles[index];
    if (tile) {
      updateStatus('Generating tile...', `Tile ${tile.r},${tile.c}`, null);
    }
    try {
      await processSingleTile(index);
      scheduleCompositePreviewRender();
    } finally {
      clearStatus();
    }
  }

  async function generateSelectionTiles() {
    if (!boxGenerateMode) return;
    if (isProcessing || isSplitting || isMerging || isRegionProcessing) return;

    try {
      isRegionProcessing = true;
      updateStatus('Preparing selection...', '', null);
      if (!hasEditedSelectionBox) {
        throw new Error('Please draw or adjust the selection box before generating.');
      }

      const selectedIndices = tiles
        .map((tile, index) => (isTileIntersectingSelection(tile) ? index : -1))
        .filter((index) => index >= 0);

      if (selectedIndices.length === 0) {
        throw new Error('No tiles overlap with the selected box.');
      }
      const selectedCount = selectedIndices.length;
      const region = {
        x: Math.round(boxX),
        y: Math.round(boxY),
        width: Math.max(1, Math.round(boxW)),
        height: Math.max(1, Math.round(boxH))
      };
      updateStatus(
        'Generating selection region...',
        `${region.width}x${region.height}, ${selectedCount} tiles affected`,
        10
      );

      dispatch('log', {
        type: 'info',
        message: `Generating one continuous region overlay for ${selectedCount} tiles...`
      });

      const operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
      let resultBlob: Blob;

      if (operationMode === 'mock') {
        resultBlob = await generateMockTile(region.width, region.height, 0, 0);
      } else {
        const apiKey = localStorage.getItem('gemini_api_key');
        const model = localStorage.getItem('gemini_model') || 'gemini-2.5-flash-image';
        const apiBaseUrl = getApiBaseUrl();
        if (!apiKey) throw new Error("API Key not found. Please set it in Settings.");

        let prompt = buildPromptForTile({
          r: 0,
          c: 0,
          w: region.width,
          h: region.height
        });
        prompt += `\nGenerate only the selected box region (${region.width}x${region.height}) from the input image.`;

        const useFullImageReference = isFullImageReferenceEnabled();
        const regionDataUrl = await cropRegionInputDataUrl(
          region.x,
          region.y,
          region.width,
          region.height,
          true
        );
        const regionResponse = await fetch(regionDataUrl);
        if (!regionResponse.ok) {
          throw new Error(`Failed to read region image (${regionResponse.status})`);
        }
        const croppedBlob = await regionResponse.blob();

        if (isVerboseLoggingEnabled()) {
          dispatch('log', {
            type: 'info',
            message: `[Prompt box ${region.x},${region.y},${region.width},${region.height}] ${prompt.replace(/\n/g, '\\n')}`
          });
        }

        resultBlob = await generateImage(croppedBlob, prompt, model, apiKey, {
          apiBaseUrl,
          fullImageBlob: useFullImageReference ? await getFullImageBlob() : null
        });
      }

      const reader = new FileReader();
      reader.readAsDataURL(resultBlob);
      const resultB64Raw = await new Promise<string>((resolve) => {
        reader.onloadend = () => resolve(reader.result as string);
      });
      const resultB64 = ensureImageDataUrl(
        resultB64Raw,
        bgRemovalEnabled ? 'image/png' : 'image/jpeg'
      );
      updateStatus('Applying selection overlay...', `Region ${region.width}x${region.height}`, 90);

      regionOverlays = [
        ...regionOverlays,
        {
          id: Date.now(),
          x: region.x,
          y: region.y,
          width: region.width,
          height: region.height,
          dataUrl: resultB64
        }
      ];
      scheduleCompositePreviewRender();
      dispatch('log', {
        type: 'success',
        message: `Selection-box generation complete (${selectedCount} tiles).`
      });
      cancelBoxGenerateMode();
    } catch (e: any) {
      dispatch('log', {
        type: 'error',
        message: `Selection-box generation failed: ${e?.message || e}`
      });
    } finally {
      isRegionProcessing = false;
      clearStatus();
    }
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
  on:mousedown={handleMouseDown}
  on:mousemove={handleMouseMove}
  on:mouseup={stopMousePanning}
  on:mouseleave={stopMousePanning}
  on:auxclick|preventDefault
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

           {#if statusActive}
             <div class="fixed right-4 bottom-4 z-[80] pointer-events-none">
               <div class="rounded-lg bg-black/65 text-white border border-white/20 px-3 py-2 min-w-[230px] shadow-lg backdrop-blur-sm">
                 <div class="flex items-center gap-2">
                   <div class="animate-spin rounded-full h-3.5 w-3.5 border-b-2 border-white/90"></div>
                   <span class="text-xs font-semibold">{statusTitle}</span>
                 </div>
                 {#if statusDetail}
                   <div class="text-[11px] text-white/80 mt-1">{statusDetail}</div>
                 {/if}
                 {#if statusProgress !== null}
                   <div class="mt-2 h-1.5 w-full rounded bg-white/20 overflow-hidden">
                     <div
                       class="h-full bg-blue-400 transition-all duration-200"
                       style="width: {Math.max(0, Math.min(100, statusProgress))}%;"
                     ></div>
                   </div>
                 {/if}
               </div>
             </div>
           {/if}

           {#if boxGenerateMode}
             <!-- svelte-ignore a11y_no_static_element_interactions -->
             <div
               class="absolute inset-0 z-40"
               data-box-overlay="1"
               on:mousedown={(e) => startSelectionDrag(e, 'new')}
               on:mousemove={updateSelectionDrag}
               on:mouseup={stopSelectionDrag}
               on:mouseleave={stopSelectionDrag}
             >
               <div class="absolute inset-0 bg-blue-900/10 border border-blue-400/25 pointer-events-none"></div>
               <!-- svelte-ignore a11y_no_static_element_interactions -->
               <div
                 class="absolute border-2 border-blue-400 bg-blue-500/10 shadow-[0_0_0_9999px_rgba(15,23,42,0.28)]"
                 style="left: {boxX / originalW * 100}%; top: {boxY / originalH * 100}%; width: {boxW / originalW * 100}%; height: {boxH / originalH * 100}%;"
                 on:mousedown|stopPropagation={(e) => startSelectionDrag(e, 'move')}
               >
                 <div class="absolute -top-6 left-1/2 -translate-x-1/2 bg-black/70 text-white text-[10px] px-2 py-0.5 rounded pointer-events-none whitespace-nowrap font-mono">
                   {Math.round(boxW)} x {Math.round(boxH)}
                 </div>
                 <div class="absolute -left-1.5 -top-1.5 w-3 h-3 bg-white border border-blue-600 cursor-nw-resize rounded-full shadow-sm" on:mousedown|stopPropagation={(e) => startSelectionDrag(e, 'nw')}></div>
                 <div class="absolute -right-1.5 -top-1.5 w-3 h-3 bg-white border border-blue-600 cursor-ne-resize rounded-full shadow-sm" on:mousedown|stopPropagation={(e) => startSelectionDrag(e, 'ne')}></div>
                 <div class="absolute -left-1.5 -bottom-1.5 w-3 h-3 bg-white border border-blue-600 cursor-sw-resize rounded-full shadow-sm" on:mousedown|stopPropagation={(e) => startSelectionDrag(e, 'sw')}></div>
                 <div class="absolute -right-1.5 -bottom-1.5 w-3 h-3 bg-white border border-blue-600 cursor-se-resize rounded-full shadow-sm" on:mousedown|stopPropagation={(e) => startSelectionDrag(e, 'se')}></div>
               </div>
               <div class="absolute right-4 bottom-4 flex items-center gap-2">
                 <button
                   on:mousedown|stopPropagation
                   on:click|stopPropagation={cancelBoxGenerateMode}
                   class="px-3 py-1.5 rounded-md bg-black/65 hover:bg-black/75 text-white text-xs font-semibold border border-white/20 shadow"
                 >
                   {$t('settings.cancel')}
                 </button>
                 <button
                   on:mousedown|stopPropagation
                   on:click|stopPropagation={generateSelectionTiles}
                   disabled={isSplitting || isMerging || isProcessing || isRegionProcessing}
                   class="px-3 py-1.5 rounded-md bg-blue-600 hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed text-white text-xs font-semibold shadow"
                 >
                   {$t('generateInBox')}
                 </button>
               </div>
             </div>
           {/if}
           
           {#each tiles as tile, index}
             <!-- svelte-ignore a11y_no_static_element_interactions -->
             <div 
               class="absolute group transition-all duration-200 border flex items-center justify-center cursor-default overflow-hidden {boxGenerateMode ? 'pointer-events-none' : ''} {hoveredTileIndex === index ? 'border-blue-400 bg-blue-500/30' : 'border-transparent hover:border-blue-400 hover:bg-blue-500/30'}"
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

                  {#if hoveredTileIndex === index && !boxGenerateMode}
                  <button 
                    on:click|stopPropagation={() => regenerateTile(index)}
                    class="bg-blue-600 hover:bg-blue-500 text-white text-xs font-bold px-4 py-2 rounded-full shadow-lg transition-all duration-150 ease-out z-40 cursor-pointer disabled:cursor-not-allowed"
                    disabled={isSplitting || isMerging || isProcessing || isRegionProcessing}
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
