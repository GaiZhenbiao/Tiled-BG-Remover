<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import ImageUploader from '../lib/ImageUploader.svelte';
  import TileGrid from '../lib/TileGrid.svelte';
  import Settings from '../lib/Settings.svelte';
  import CropModal from '../lib/CropModal.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { t } from '../lib/i18n';
  import { detectMainSubject } from '../lib/api';
  import {
    DEFAULT_IMAGE_GENERATION_MODELS,
    ensureSelectedModelName,
    readImageGenerationModels
  } from '../lib/modelRegistry';

  const EXPORT_DEFAULTS_STORAGE_KEY = 'export_content_defaults';
  const BOX_LAYER_SIDEBAR_WIDTH_STORAGE_KEY = 'box_layer_sidebar_width';
  const BOX_LAYER_SIDEBAR_MIN_WIDTH = 240;
  const BOX_LAYER_SIDEBAR_MAX_WIDTH = 520;
  type ExportContentDefaults = {
    saveTiles: boolean;
    saveMerged: boolean;
    savePsd: boolean;
  };

  function readExportContentDefaults(): ExportContentDefaults {
    try {
      const raw = localStorage.getItem(EXPORT_DEFAULTS_STORAGE_KEY);
      if (!raw) {
        return { saveTiles: true, saveMerged: true, savePsd: true };
      }
      const parsed = JSON.parse(raw) as Partial<ExportContentDefaults>;
      return {
        saveTiles: parsed.saveTiles !== false,
        saveMerged: parsed.saveMerged !== false,
        savePsd: parsed.savePsd !== false
      };
    } catch {
      return { saveTiles: true, saveMerged: true, savePsd: true };
    }
  }

  function writeExportContentDefaults(defaults: ExportContentDefaults) {
    localStorage.setItem(EXPORT_DEFAULTS_STORAGE_KEY, JSON.stringify(defaults));
  }

  let imagePath = '';
  let originalFilename = '';
  let showSettings = false;
  let showCropModal = false;
  type ThemeMode = 'auto' | 'light' | 'dark';
  let theme = parseThemeMode(localStorage.getItem('theme'));
  let themeMediaQuery: MediaQueryList | null = null;
  let systemPrefersDark = false;
  
  let logs: { type: string, message: string, time: string }[] = [];
  let selectedModel = ensureSelectedModelName(
    readImageGenerationModels(),
    localStorage.getItem('gemini_model') || DEFAULT_IMAGE_GENERATION_MODELS[0].name
  );

  function supportsHighResOutput(modelName: string): boolean {
    const value = (modelName || '').toLowerCase();
    return value.includes('gemini-3-pro') || value.includes('gemini-3.1-flash-image-preview');
  }
  
  // State
  let rows = 2;
  let cols = 2;
  let overlap = 0;
  let overlapXRatio = 0;
  let overlapYRatio = 0;
  let aiOutputRes = supportsHighResOutput(selectedModel) ? 2048 : 1024;
  let concurrency = 2;
  let smartGridEnabled = true;
  const smartGridMaxCount = 64;
  let smartTileTolerancePx = clampInt(
    parseInt(localStorage.getItem('smart_tile_tolerance_px') || String(aiOutputRes * 2)),
    1,
    20000
  );
  let smartQualitySliderPx = smartTileTolerancePx;
  let showTileLines = false;
  let alwaysSquareTiles = localStorage.getItem('always_square_tiles') === 'true';
  let isAdjustingGrid = false;
  let gridAdjustTimer: ReturnType<typeof setTimeout> | null = null;
  
  // BG Removal State
  let bgRemovalEnabled = false;
  let keyColor = localStorage.getItem('key_color') || 'green';
  let tolerance = parseInt(localStorage.getItem('key_tolerance') || '10');
  type NonBgColorMode = 'white' | 'black' | 'green' | 'blue' | 'custom';
  const NON_BG_COLOR_HEX: Record<Exclude<NonBgColorMode, 'custom'>, string> = {
    white: '#FFFFFF',
    black: '#000000',
    green: '#00FF00',
    blue: '#0000FF'
  };
  let nonBgColorMode = parseNonBgColorMode(localStorage.getItem('non_bg_color_mode'));
  let nonBgCustomHex = normalizeHexColor(localStorage.getItem('non_bg_custom_hex') || '#FFFFFF');
  let nonBgBackgroundHex = resolveNonBgHex(nonBgColorMode, nonBgCustomHex);
  let showOriginalInput = false;
  let boxGenerateMode = true;
  let boxAspectMode = '1:1';
  let boxGenerateAspectRatio: number | null = 1;
  let tileGridRef: any = null;
  let boxLayers: any[] = [];
  let hasGeneratedBoxLayer = false;
  let showToolbarLogsPopover = false;
  let showToolbarBackgroundPopover = false;
  let showToolbarSubjectPopover = false;
  let boxLayerSidebarWidth = clampInt(
    parseInt(localStorage.getItem(BOX_LAYER_SIDEBAR_WIDTH_STORAGE_KEY) || '320'),
    BOX_LAYER_SIDEBAR_MIN_WIDTH,
    BOX_LAYER_SIDEBAR_MAX_WIDTH
  );
  let isResizingBoxLayerSidebar = false;
  let boxLayerResizeStartX = 0;
  let boxLayerResizeStartWidth = boxLayerSidebarWidth;
  
  onMount(() => {
    const mql = window.matchMedia('(prefers-color-scheme: dark)');
    themeMediaQuery = mql;
    systemPrefersDark = mql.matches;
    const handleSystemThemeChange = (event: MediaQueryListEvent) => {
      systemPrefersDark = event.matches;
      if (theme === 'auto') {
        applyTheme();
      }
    };
    if (typeof mql.addEventListener === 'function') {
      mql.addEventListener('change', handleSystemThemeChange);
    } else {
      const legacyMql = mql as MediaQueryList & {
        addListener?: (listener: (event: MediaQueryListEvent) => void) => void;
      };
      legacyMql.addListener?.(handleSystemThemeChange);
    }

    if (localStorage.getItem('bg_removal_enabled') !== null) {
      bgRemovalEnabled = localStorage.getItem('bg_removal_enabled') === 'true';
    }
    if (localStorage.getItem('concurrency') !== null) {
      concurrency = parseInt(localStorage.getItem('concurrency') || '2');
    }
    const exportDefaults = readExportContentDefaults();
    exportIncludeTiles = exportDefaults.saveTiles;
    exportIncludeMerged = exportDefaults.saveMerged;
    exportIncludePsd = exportDefaults.savePsd;
    applyTheme();

    return () => {
      const cleanupMql = themeMediaQuery;
      if (!cleanupMql) return;
      if (typeof cleanupMql.removeEventListener === 'function') {
        cleanupMql.removeEventListener('change', handleSystemThemeChange);
      } else {
        const legacyCleanupMql = cleanupMql as MediaQueryList & {
          removeListener?: (listener: (event: MediaQueryListEvent) => void) => void;
        };
        legacyCleanupMql.removeListener?.(handleSystemThemeChange);
      }
    };
  });

  onDestroy(() => {
    if (gridAdjustTimer) {
      clearTimeout(gridAdjustTimer);
    }
    stopBoxLayerSidebarResize();
  });

  function handleSettingsClose() {
    showSettings = false;
    bgRemovalEnabled = localStorage.getItem('bg_removal_enabled') === 'true';
    keyColor = localStorage.getItem('key_color') || 'green';
    tolerance = parseInt(localStorage.getItem('key_tolerance') || '10');
    alwaysSquareTiles = localStorage.getItem('always_square_tiles') === 'true';
    const models = readImageGenerationModels();
    selectedModel = ensureSelectedModelName(models, localStorage.getItem('gemini_model') || selectedModel);
    localStorage.setItem('gemini_model', selectedModel);
  }

  function parseThemeMode(value: string | null): ThemeMode {
    if (value === 'light' || value === 'dark' || value === 'auto') {
      return value;
    }
    return 'auto';
  }

  function applyTheme() {
    const isDark = theme === 'dark' || (theme === 'auto' && systemPrefersDark);
    document.documentElement.classList.toggle('dark', isDark);
    document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';
  }

  $: if (theme) {
    localStorage.setItem('theme', theme);
    applyTheme();
  }

  $: localStorage.setItem('smart_tile_tolerance_px', smartTileTolerancePx.toString());
  $: localStorage.setItem('non_bg_color_mode', nonBgColorMode);
  $: localStorage.setItem('non_bg_custom_hex', normalizeHexColor(nonBgCustomHex));
  $: nonBgBackgroundHex = resolveNonBgHex(nonBgColorMode, nonBgCustomHex);
  
  // Processing state
  let isProcessing = false;
  let resultSrc = '';
  let exportTiles: any[] = [];
  let exportOverlays: any[] = [];
  let exportRegularLayers: any[] = [];
  let exportAlertVisible = false;
  let exportProgress = 0;
  let exportMessage = '';
  let exportError = '';
  let exportDone = false;
  let exportDirPath = '';
  let exportMergedPath = '';
  let showFolderNameModal = false;
  let exportFolderNameInput = '';
  let exportIncludeTiles = true;
  let exportIncludeMerged = true;
  let exportIncludePsd = true;
  let exportSetAsDefault = false;
  let exportSelectionError = '';
  let hasTileMetadataForExport = false;
  
  // Image Info
  let imgWidth = 0;
  let imgHeight = 0;
  let detectedSubject = '';
  let manualSubject = '';
  let userEditedSubject = false;
  let subjectStatus: 'idle' | 'detecting' | 'ready' | 'no_api' | 'error' = 'idle';
  let subjectError = '';
  let subjectDetectSeq = 0;
  $: hasTileMetadataForExport = exportTiles.length > 0;
  $: promptSubject = (manualSubject.trim() || detectedSubject || 'main subject');
  
  $: if (imagePath) {
    updateImageInfo();
  }
  $: if (!imagePath) {
    showToolbarLogsPopover = false;
    showToolbarBackgroundPopover = false;
    showToolbarSubjectPopover = false;
  }

  function clearInput() {
    imagePath = '';
    resultSrc = '';
    boxGenerateMode = true;
    detectedSubject = '';
    manualSubject = '';
    userEditedSubject = false;
    subjectStatus = 'idle';
    subjectError = '';
    subjectDetectSeq += 1;
    logs = [];
  }
  
  async function updateImageInfo() {
    const img = new Image();
    try {
      const b64 = await invoke('load_image', { path: imagePath });
      img.src = b64 as string;
      await new Promise(r => img.onload = r);
      imgWidth = img.naturalWidth;
      imgHeight = img.naturalHeight;
    } catch (e) {
      console.error("Failed to load image for info:", e);
    }
  }

  // Re-check selected model when settings modal closes
  $: if (!showSettings) {
    const models = readImageGenerationModels();
    selectedModel = ensureSelectedModelName(models, localStorage.getItem('gemini_model') || selectedModel);
  }

  let previousModel = selectedModel;
  $: if (selectedModel !== previousModel) {
    aiOutputRes = supportsHighResOutput(selectedModel) ? 2048 : 1024;
    previousModel = selectedModel;
  }

  $: availableResolutions = supportsHighResOutput(selectedModel)
    ? [1024, 2048, 4096] 
    : [1024];

  $: if (!availableResolutions.includes(aiOutputRes)) {
    aiOutputRes = availableResolutions[0];
  }

  $: smartTileLimitMax = Math.max(imgWidth, imgHeight) > 0 ? Math.max(imgWidth, imgHeight) : aiOutputRes * 2;
  $: smartTileLimitMin = Math.min(aiOutputRes, smartTileLimitMax);
  $: smartTileTolerancePx = clampInt(smartTileTolerancePx, smartTileLimitMin, smartTileLimitMax);
  $: smartQualitySliderPx = smartTileLimitMin + smartTileLimitMax - smartTileTolerancePx;
  $: smartMaxTileSize = smartTileTolerancePx;

  // Smart Grid Logic
  $: if (smartGridEnabled && imgWidth && imgHeight && aiOutputRes) {
    const O = overlap;
    cols = computeSmartGridCount(imgWidth, smartMaxTileSize, O);
    rows = computeSmartGridCount(imgHeight, smartMaxTileSize, O);
  }

  $: overlapXRatio = overlap;
  $: overlapYRatio = overlap;
  $: if (alwaysSquareTiles && imgWidth > 0 && imgHeight > 0 && rows > 0 && cols > 0) {
    const safeColsDenom = cols - (cols - 1) * overlap;
    const safeRowsDenom = rows - (rows - 1) * overlap;
    if (safeColsDenom > 0 && safeRowsDenom > 0) {
      const baseTileW = imgWidth / safeColsDenom;
      const baseTileH = imgHeight / safeRowsDenom;
      const targetSquareSide = Math.max(baseTileW, baseTileH);
      const nextOverlapX =
        cols <= 1 ? 0 : (cols - imgWidth / targetSquareSide) / (cols - 1);
      const nextOverlapY =
        rows <= 1 ? 0 : (rows - imgHeight / targetSquareSide) / (rows - 1);
      overlapXRatio = Math.min(0.95, Math.max(0, nextOverlapX));
      overlapYRatio = Math.min(0.95, Math.max(0, nextOverlapY));
    }
  }
  
  // Resolution Info
  $: tileW = imgWidth / (cols - (cols - 1) * overlapXRatio);
  $: tileH = imgHeight / (rows - (rows - 1) * overlapYRatio);
  $: totalTiles = rows * cols;

  function handleImageSelected(path: string) {
    imagePath = path;
    resultSrc = '';
    boxGenerateMode = true;
    manualSubject = '';
    userEditedSubject = false;
    startSubjectDetection(path);
    
    // Store original filename for saving later
    const parts = path.split(/[\\/]/);
    const filename = parts.pop();
    if (filename) {
        const lastDot = filename.lastIndexOf('.');
        originalFilename = lastDot > -1 ? filename.substring(0, lastDot) : filename;
    }
  }
  
  function handleCropDone(e: any) {
    const { x, y, width, height } = e.detail;
    void performCrop(x, y, width, height);
    showCropModal = false;
  }

  function parseDataUrlMime(dataUrl: string): string {
    const match = (dataUrl || '').match(/^data:([^;]+);base64,/i);
    return match?.[1]?.toLowerCase() || 'image/jpeg';
  }

  function dataUrlToBlob(dataUrl: string): Blob {
    const match = (dataUrl || '').match(/^data:([^;]+);base64,(.+)$/i);
    if (!match) {
      throw new Error('Invalid image data URL.');
    }
    const mime = match[1] || 'image/jpeg';
    let base64 = match[2] || '';
    base64 = base64.replace(/\s+/g, '').replace(/-/g, '+').replace(/_/g, '/');
    const mod = base64.length % 4;
    if (mod) base64 += '='.repeat(4 - mod);
    const binary = atob(base64);
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
    return new Blob(chunks, { type: mime });
  }

  function extensionFromMime(mime: string): string {
    if (mime === 'image/png') return 'png';
    if (mime === 'image/webp') return 'webp';
    return 'jpg';
  }

  function splitPath(inputPath: string): { dir: string; stem: string; sep: string } {
    const slashIdx = inputPath.lastIndexOf('/');
    const backslashIdx = inputPath.lastIndexOf('\\');
    const idx = Math.max(slashIdx, backslashIdx);
    const sep = backslashIdx > slashIdx ? '\\' : '/';
    const dir = idx >= 0 ? inputPath.slice(0, idx) : '.';
    const fileName = idx >= 0 ? inputPath.slice(idx + 1) : inputPath;
    const dot = fileName.lastIndexOf('.');
    const stem = dot > 0 ? fileName.slice(0, dot) : fileName || 'image';
    return { dir, stem, sep };
  }

  function buildCroppedPath(sourcePath: string, mime: string): string {
    const { dir, stem, sep } = splitPath(sourcePath);
    const ext = extensionFromMime(mime);
    return `${dir}${sep}${stem}_Cropped_${Date.now()}.${ext}`;
  }

  async function loadImageElement(dataUrl: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error('Failed to decode image for cropping.'));
      img.src = dataUrl;
    });
  }

  async function cropImageFrontend(
    sourcePath: string,
    x: number,
    y: number,
    width: number,
    height: number
  ): Promise<{ croppedDataUrl: string; mime: string }> {
    const sourceDataUrl = (await invoke('load_image', { path: sourcePath })) as string;
    const mime = parseDataUrlMime(sourceDataUrl);
    const image = await loadImageElement(sourceDataUrl);

    const sourceW = Math.max(1, image.naturalWidth);
    const sourceH = Math.max(1, image.naturalHeight);
    const cropX = Math.max(0, Math.min(sourceW - 1, Math.round(x)));
    const cropY = Math.max(0, Math.min(sourceH - 1, Math.round(y)));
    const cropW = Math.max(1, Math.min(Math.round(width), sourceW - cropX));
    const cropH = Math.max(1, Math.min(Math.round(height), sourceH - cropY));

    const canvas = document.createElement('canvas');
    canvas.width = cropW;
    canvas.height = cropH;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Failed to create crop canvas context.');
    }
    ctx.drawImage(image, cropX, cropY, cropW, cropH, 0, 0, cropW, cropH);

    const targetMime = mime === 'image/png' ? 'image/png' : 'image/jpeg';
    const croppedDataUrl = canvas.toDataURL(targetMime, targetMime === 'image/jpeg' ? 0.92 : undefined);
    return { croppedDataUrl, mime: targetMime };
  }

  async function performCrop(x: number, y: number, width: number, height: number) {
    try {
      const { croppedDataUrl, mime } = await cropImageFrontend(imagePath, x, y, width, height);
      const newPath = buildCroppedPath(imagePath, mime);
      await invoke('save_image', {
        path: newPath,
        base64Data: croppedDataUrl
      });
      imagePath = newPath;
      resultSrc = '';
      boxGenerateMode = true;
      manualSubject = '';
      userEditedSubject = false;
      startSubjectDetection(newPath);
      addLog({
        detail: {
          type: 'success',
          message: `Image cropped: ${newPath}`
        }
      });
    } catch (e: any) {
      const message = e?.message || String(e);
      addLog({
        detail: {
          type: 'error',
          message: `Crop failed: ${message}`
        }
      });
    }
  }

  function onSubjectInput(event: Event) {
    const target = event.target as HTMLInputElement;
    manualSubject = target.value;
    userEditedSubject = true;
  }

  function handleTileGridSubjectInput(event: CustomEvent<string>) {
    manualSubject = (event.detail || '').toString();
    userEditedSubject = true;
  }

  function handleTileGridBoxAspectModeChange(event: CustomEvent<string>) {
    boxAspectMode = (event.detail || '1:1').toString();
  }

  function closeToolbarPopovers() {
    showToolbarLogsPopover = false;
    showToolbarBackgroundPopover = false;
    showToolbarSubjectPopover = false;
  }

  function toggleToolbarLogsPopover() {
    const next = !showToolbarLogsPopover;
    closeToolbarPopovers();
    showToolbarLogsPopover = next;
  }

  function toggleToolbarBackgroundPopover() {
    const next = !showToolbarBackgroundPopover;
    closeToolbarPopovers();
    showToolbarBackgroundPopover = next;
  }

  function toggleToolbarSubjectPopover() {
    const next = !showToolbarSubjectPopover;
    closeToolbarPopovers();
    showToolbarSubjectPopover = next;
  }

  function useDetectedSubject() {
    manualSubject = detectedSubject || '';
    userEditedSubject = false;
  }

  function handleTileGridSourceUpdate(e: CustomEvent<string>) {
    const nextPath = (e.detail || '').trim();
    if (!nextPath || nextPath === imagePath) return;
    imagePath = nextPath;
  }

  function handleRegenerateBoxLayer(layerId: number) {
    tileGridRef?.regenerateRegionOverlay?.(layerId);
  }

  function handleToggleBoxLayerVisibility(layerId: number) {
    tileGridRef?.toggleRegionOverlayVisibility?.(layerId);
  }

  function handleDeleteBoxLayer(layerId: number) {
    tileGridRef?.deleteRegionOverlay?.(layerId);
  }

  function handleSetBoxLayerVersion(layerId: number, versionIndex: number) {
    tileGridRef?.setRegionOverlayVersion?.(layerId, versionIndex);
  }

  function handleMoveBoxLayerUp(layerId: number) {
    tileGridRef?.moveRegionOverlayById?.(layerId, 'up');
  }

  function handleMoveBoxLayerDown(layerId: number) {
    tileGridRef?.moveRegionOverlayById?.(layerId, 'down');
  }

  function handleLayerHoverStart(layerId: number) {
    tileGridRef?.setHoveredRegionOverlay?.(layerId);
  }

  function handleLayerHoverEnd() {
    tileGridRef?.setHoveredRegionOverlay?.(null);
  }

  function handleRenameBoxLayer(layerId: number, name: string) {
    tileGridRef?.renameRegionOverlay?.(layerId, name);
  }

  function persistBoxLayerSidebarWidth() {
    localStorage.setItem(BOX_LAYER_SIDEBAR_WIDTH_STORAGE_KEY, String(boxLayerSidebarWidth));
  }

  function onBoxLayerSidebarResizeMove(event: MouseEvent) {
    if (!isResizingBoxLayerSidebar) return;
    const delta = event.clientX - boxLayerResizeStartX;
    boxLayerSidebarWidth = clampInt(
      boxLayerResizeStartWidth + delta,
      BOX_LAYER_SIDEBAR_MIN_WIDTH,
      BOX_LAYER_SIDEBAR_MAX_WIDTH
    );
  }

  function stopBoxLayerSidebarResize() {
    if (!isResizingBoxLayerSidebar) return;
    isResizingBoxLayerSidebar = false;
    window.removeEventListener('mousemove', onBoxLayerSidebarResizeMove);
    window.removeEventListener('mouseup', stopBoxLayerSidebarResize);
    persistBoxLayerSidebarWidth();
  }

  function startBoxLayerSidebarResize(event: MouseEvent) {
    if (event.button !== 0) return;
    isResizingBoxLayerSidebar = true;
    boxLayerResizeStartX = event.clientX;
    boxLayerResizeStartWidth = boxLayerSidebarWidth;
    window.addEventListener('mousemove', onBoxLayerSidebarResizeMove);
    window.addEventListener('mouseup', stopBoxLayerSidebarResize);
    event.preventDefault();
  }

  function getLayerStatusLabel(status: string): string {
    if (status === 'queued') return String($t('layerQueued'));
    if (status === 'processing') return String($t('layerProcessing'));
    if (status === 'error') return String($t('layerFailed'));
    return String($t('done'));
  }

  $: hasGeneratedBoxLayer = boxLayers.some(
    (layer) => layer.status === 'done' || (Number(layer.versionCount) || 0) > 0
  );

  function addLog(e: any) {
    logs = [{ ...e.detail, time: new Date().toLocaleTimeString() }, ...logs].slice(0, 100);
  }

  function updateExportAlert(progress: number, message: string) {
    exportProgress = Math.max(0, Math.min(100, Math.round(progress)));
    exportMessage = message;
  }

  function closeExportAlert() {
    exportAlertVisible = false;
  }

  async function openExportDirectory() {
    if (!exportDirPath) return;
    try {
      await invoke('open_path', { path: exportDirPath });
    } catch (e) {
      console.error('Failed to open export directory:', e);
    }
  }

  async function runExportWithFolderName(
    folderName: string,
    options: { saveTiles: boolean; saveMerged: boolean; savePsd: boolean }
  ) {
    if (!resultSrc) return;

    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: originalFilename || undefined
    });
    const exportParentDir = Array.isArray(selected) ? selected[0] : selected;
    if (typeof exportParentDir === 'string' && exportParentDir.length > 0) {
      exportAlertVisible = true;
      exportDone = false;
      exportError = '';
      exportDirPath = '';
      exportMergedPath = '';
      updateExportAlert(10, String($t('exporting')));
      await tick();
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

      const localizedSuffix = String($t('bgRemovedSuffix'));

      try {
        updateExportAlert(35, String($t('exporting')));
        const verboseLogging = localStorage.getItem('verbose_logging') === 'true';
        const exportRes: any = await invoke('save_export_bundle', {
          outputDir: exportParentDir,
          mergedBase64: resultSrc,
          tiles: exportTiles,
          overlays: exportOverlays,
          regularLayers: exportRegularLayers,
          sourcePath: imagePath,
          inputName: originalFilename || 'image',
          folderName,
          removeBg: bgRemovalEnabled,
          localizedSuffix,
          verboseLogging,
          saveTiles: options.saveTiles,
          saveMerged: options.saveMerged,
          savePsd: options.savePsd
        });
        if (verboseLogging && Array.isArray(exportRes?.psd_logs) && exportRes.psd_logs.length > 0) {
          const extraLogs = exportRes.psd_logs.map((message: string) => ({
            type: 'info',
            message,
            time: new Date().toLocaleTimeString()
          }));
          logs = [...extraLogs.reverse(), ...logs];
        }
        logs = [{
          type: 'success',
          message: `Saved export bundle (${exportRes.tile_count} tiles): ${exportRes.export_dir}`,
          time: new Date().toLocaleTimeString()
        }, ...logs];
        exportDirPath = exportRes.export_dir || '';
        exportMergedPath =
          exportRes.merged_path || exportRes.psd_path || exportRes.tiles_dir || exportRes.export_dir || '';
        updateExportAlert(100, String($t('exportComplete')));
        exportDone = true;
      } catch (e: any) {
        exportError = e?.message || String(e);
        updateExportAlert(100, String($t('exportProgressTitle')));
        logs = [{
          type: 'error',
          message: `Export failed: ${exportError}`,
          time: new Date().toLocaleTimeString()
        }, ...logs];
      }
    }
  }

  async function saveResult() {
    if (!resultSrc) return;
    const exportDefaults = readExportContentDefaults();
    exportFolderNameInput = (originalFilename || 'image').trim() || 'image';
    exportSelectionError = '';
    exportSetAsDefault = false;
    exportIncludeMerged = exportDefaults.saveMerged;
    exportIncludeTiles = exportDefaults.saveTiles;
    exportIncludePsd = exportDefaults.savePsd;
    showFolderNameModal = true;
  }

  function setExportSelectionAsDefault() {
    writeExportContentDefaults({
      saveTiles: exportIncludeTiles,
      saveMerged: exportIncludeMerged,
      savePsd: exportIncludePsd
    });
  }

  function cancelFolderNameModal() {
    showFolderNameModal = false;
    exportSelectionError = '';
    exportSetAsDefault = false;
  }

  async function confirmFolderNameModal() {
    if (!exportIncludeTiles && !exportIncludeMerged && !exportIncludePsd) {
      exportSelectionError = String($t('exportSelectAtLeastOne'));
      return;
    }
    const folderName = (exportFolderNameInput || '').trim() || (originalFilename || 'image');
    const saveTiles = exportIncludeTiles;
    const saveMerged = exportIncludeMerged;
    const savePsd = exportIncludePsd;
    if (exportSetAsDefault) {
      setExportSelectionAsDefault();
    }
    showFolderNameModal = false;
    exportSelectionError = '';
    exportSetAsDefault = false;
    await runExportWithFolderName(folderName, { saveTiles, saveMerged, savePsd });
  }

  function clampInt(value: number, min: number, max: number): number {
    if (Number.isNaN(value)) return min;
    return Math.min(max, Math.max(min, Math.round(value)));
  }

  function parseNonBgColorMode(value: string | null): NonBgColorMode {
    if (value === 'black' || value === 'green' || value === 'blue' || value === 'custom') {
      return value;
    }
    return 'white';
  }

  function normalizeHexColor(value: string): string {
    const raw = (value || '').trim();
    const match = raw.match(/^#?([0-9a-fA-F]{6})$/);
    if (match) {
      return `#${match[1].toUpperCase()}`;
    }
    return '#FFFFFF';
  }

  function resolveNonBgHex(mode: NonBgColorMode, customHex: string): string {
    if (mode === 'custom') {
      return normalizeHexColor(customHex);
    }
    return NON_BG_COLOR_HEX[mode];
  }

  function computeSmartGridCount(size: number, maxTileSize: number, overlapRatio: number): number {
    const safeMaxTileSize = Math.max(1, maxTileSize);
    const denom = 1 - overlapRatio;
    if (denom <= 0) return smartGridMaxCount;

    const rawCount = (size / safeMaxTileSize - overlapRatio) / denom;
    return clampInt(Math.ceil(rawCount), 1, smartGridMaxCount);
  }

  function markGridAdjusting() {
    isAdjustingGrid = true;
    if (gridAdjustTimer) {
      clearTimeout(gridAdjustTimer);
    }
    gridAdjustTimer = setTimeout(() => {
      isAdjustingGrid = false;
      gridAdjustTimer = null;
    }, 250);
  }

  function onSmartQualityInput(e: Event) {
    const target = e.target as HTMLInputElement;
    const visualValue = clampInt(parseInt(target.value), smartTileLimitMin, smartTileLimitMax);
    smartTileTolerancePx = smartTileLimitMin + smartTileLimitMax - visualValue;
    markGridAdjusting();
  }

  function startSubjectDetection(path: string) {
    const seq = ++subjectDetectSeq;
    detectedSubject = '';
    subjectError = '';

    const apiKey = localStorage.getItem('gemini_api_key') || '';
    const operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
    const apiUrl = localStorage.getItem('gemini_api_url') || 'https://generativelanguage.googleapis.com';
    if (!path || !apiKey || operationMode === 'mock') {
      subjectStatus = 'no_api';
      return;
    }

    subjectStatus = 'detecting';
    void (async () => {
      try {
        const b64 = (await invoke('load_image', { path })) as string;
        const blob = dataUrlToBlob(b64);
        const subject = await detectMainSubject(blob, apiKey, apiUrl);
        if (seq !== subjectDetectSeq) return;
        detectedSubject = subject || 'main subject';
        if (!userEditedSubject) {
          manualSubject = detectedSubject;
        }
        subjectStatus = 'ready';
      } catch (e: any) {
        if (seq !== subjectDetectSeq) return;
        subjectStatus = 'error';
        subjectError = e?.message || String(e);
        console.error('Subject detection failed:', e);
      }
    })();
  }

  $: {
    if (boxAspectMode === 'free') {
      boxGenerateAspectRatio = null;
    } else {
      const [w, h] = boxAspectMode.split(':').map((v) => parseFloat(v));
      if (Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0) {
        boxGenerateAspectRatio = w / h;
      } else {
        boxGenerateAspectRatio = 1;
      }
    }
  }
</script>

<main class="h-screen w-screen flex flex-col bg-white dark:bg-gray-900 text-gray-900 dark:text-white overflow-hidden transition-colors duration-200">
  <!-- Header -->
  <header class="h-12 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4 bg-gray-50 dark:bg-gray-800 transition-colors">
    <div class="font-bold text-lg flex items-center gap-2">
      <span class="text-blue-600 dark:text-blue-400">{$t('appTitle')}</span>
    </div>
    <div class="flex items-center gap-2">
      {#if imagePath}
        <button
          on:click={() => showCropModal = true}
          class="h-8 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 active:opacity-100 text-gray-800 dark:text-white px-3 rounded text-sm inline-flex items-center gap-2 border border-gray-300 dark:border-gray-600 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          title={$t('cropImage')}
          aria-label={$t('cropImage')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6.13 1L6 16a2 2 0 0 0 2 2h15"></path><path d="M1 6.13L16 6a2 2 0 0 1 2 2v15"></path></svg>
        </button>

        <button
          on:click={clearInput}
          class="h-8 bg-red-600 hover:bg-red-500 active:opacity-100 text-white px-3 rounded text-sm inline-flex items-center gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-400"
          title={$t('clearInputImage')}
          aria-label={$t('clearInputImage')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        </button>
      {/if}

      {#if resultSrc}
        <button
          on:click={() => resultSrc = ''}
          class="h-8 w-8 bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 active:opacity-100 text-gray-800 dark:text-white rounded text-sm inline-flex items-center justify-center border border-gray-300 dark:border-gray-600 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          title={$t('revert')}
          aria-label={$t('revert')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 14-5-5 5-5"></path><path d="M4 9h11a4 4 0 1 1 0 8h-1"></path></svg>
        </button>
      {/if}
      {#if imagePath}
        <select
          id="header-tile-res-select"
          bind:value={aiOutputRes}
          class="h-8 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded px-2 text-sm text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus-visible:outline-none transition-colors"
          title={$t('aiOutputRes')}
          aria-label={$t('aiOutputRes')}
        >
          {#each availableResolutions as res}
            <option value={res}>{res} x {res}</option>
          {/each}
        </select>
      {/if}
      {#if resultSrc}
        <button
          on:click={saveResult}
          class="h-8 bg-green-600 hover:bg-green-500 active:opacity-100 text-white px-3 rounded text-sm inline-flex items-center gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-green-400"
          title={$t('save')}
          aria-label={$t('save')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline points="7 3 7 8 15 8"></polyline></svg>
          {$t('save')}
        </button>
      {/if}
      <div class="relative">
        <button
          type="button"
          on:click={toggleToolbarLogsPopover}
          class="h-8 w-8 inline-flex items-center justify-center bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 active:opacity-100 rounded-full transition-colors border border-gray-300 dark:border-gray-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          title={$t('logs')}
          aria-label={$t('logs')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>
        </button>
        {#if showToolbarLogsPopover}
          <div class="absolute top-11 right-0 w-[34rem] max-w-[90vw] rounded-xl border border-gray-200 dark:border-gray-700 bg-white/95 dark:bg-gray-800/95 backdrop-blur-md shadow-2xl p-3 flex flex-col gap-2 z-[90]">
            <div class="text-xs font-semibold text-gray-700 dark:text-gray-200">{$t('logs')} ({logs.length})</div>
            <div class="max-h-72 overflow-y-auto flex flex-col gap-2 font-mono text-[11px]">
              {#if logs.length === 0}
                <div class="text-gray-400 dark:text-gray-500 text-center py-6 italic">No logs yet...</div>
              {/if}
              {#each logs as log}
                <div class="flex gap-2 border-b border-gray-100 dark:border-gray-700/50 pb-1 transition-colors">
                  <span class="text-gray-500 dark:text-gray-600">[{log.time}]</span>
                  <span class={log.type === 'error' ? 'text-red-600 dark:text-red-400' : log.type === 'success' ? 'text-green-600 dark:text-green-400' : 'text-blue-600 dark:text-blue-400'}>
                    {log.message}
                  </span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <button 
        title={$t('settings.title')}
        aria-label={$t('settings.title')}
        on:click={() => showSettings = true} 
        class="h-8 w-8 inline-flex items-center justify-center bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 active:opacity-100 rounded-full transition-colors border border-gray-300 dark:border-gray-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
      </button>
    </div>
  </header>

  <!-- Content -->
  <div class="flex-1 overflow-hidden">
    <section class="w-full h-full min-w-0 min-h-0 bg-gray-50 dark:bg-gray-900 relative overflow-hidden transition-colors">
      {#if !imagePath}
        <div class="w-full h-full flex items-center justify-center p-6">
          <ImageUploader on:selected={(e) => handleImageSelected(e.detail)} />
        </div>
      {:else}
        <div class="w-full h-full min-w-0 min-h-0 flex">
          <aside
            class="h-full shrink-0 border-r border-gray-200 dark:border-gray-700 bg-white/88 dark:bg-gray-800/82 backdrop-blur-md flex flex-col gap-2 pt-3 pb-3 pl-3 pr-0"
            style={`width: ${boxLayerSidebarWidth}px;`}
          >
            <div class="flex items-center justify-between">
              <span class="text-[10px] font-bold text-gray-400 dark:text-gray-500 uppercase tracking-wider">{$t('boxLayers')}</span>
              <span class="text-xs text-gray-500 dark:text-gray-400">{boxLayers.length}</span>
            </div>
            {#if !hasGeneratedBoxLayer}
              <div class="text-[11px] text-gray-500 dark:text-gray-400 pr-3">
                {$t('drawSelectionHint')}
              </div>
            {/if}
            <div class="flex-1 overflow-y-auto flex flex-col gap-1.5 pr-2">
              {#if boxLayers.length === 0}
                <div class="text-xs text-gray-500 dark:text-gray-400 text-center py-4">
                  {$t('noBoxLayers')}
                </div>
              {:else}
                {#each boxLayers as layer, index (layer.id)}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="relative rounded border px-2 py-1.5 bg-white/80 dark:bg-gray-800/80 border-gray-200 dark:border-gray-700 shadow-sm"
                    on:mouseenter={() => handleLayerHoverStart(layer.id)}
                    on:mouseleave={handleLayerHoverEnd}
                  >
                    <div class="flex items-center gap-2">
                      <input
                        type="text"
                        value={layer.name || `${$t('layer')} ${boxLayers.length - index}`}
                        on:input={(e) => handleRenameBoxLayer(layer.id, (e.currentTarget as HTMLInputElement).value)}
                        on:change={(e) => handleRenameBoxLayer(layer.id, (e.currentTarget as HTMLInputElement).value)}
                        on:blur={(e) => handleRenameBoxLayer(layer.id, (e.currentTarget as HTMLInputElement).value)}
                        class="h-7 min-w-0 flex-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-2 text-xs font-medium text-gray-800 dark:text-gray-100"
                        title={$t('layerName')}
                        aria-label={$t('layerName')}
                      />
                      {#if layer.status !== 'done'}
                        <span
                          class="text-[10px] px-1.5 py-0.5 rounded font-medium
                          {layer.status === 'processing'
                            ? 'text-blue-700 dark:text-blue-300'
                            : layer.status === 'queued'
                            ? 'text-amber-700 dark:text-amber-300 border border-amber-300/70 dark:border-amber-500/60'
                            : 'text-red-700 dark:text-red-300 border border-red-300/70 dark:border-red-500/60'}"
                        >
                          {getLayerStatusLabel(layer.status)}
                        </span>
                      {/if}
                      {#if layer.status === 'processing' || layer.status === 'queued'}
                        <span class="w-3.5 h-3.5 inline-flex items-center justify-center">
                          <span class="w-3 h-3 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></span>
                        </span>
                      {/if}
                    </div>
                    <div class="mt-1 flex items-center gap-1.5">
                      <button
                        type="button"
                        on:click={() => handleToggleBoxLayerVisibility(layer.id)}
                        class="h-7 w-7 inline-flex items-center justify-center rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700"
                        title={layer.visible ? $t('settings.hide') : $t('settings.show')}
                        aria-label={layer.visible ? $t('settings.hide') : $t('settings.show')}
                      >
                        {#if !layer.visible}
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17.94 17.94A10.94 10.94 0 0 1 12 20C7 20 2.73 16.89 1 12c.73-2.07 1.96-3.86 3.5-5.22"></path><path d="M9.9 4.24A10.94 10.94 0 0 1 12 4c5 0 9.27 3.11 11 8a11.02 11.02 0 0 1-4.06 5.94"></path><line x1="1" y1="1" x2="23" y2="23"></line></svg>
                        {:else}
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8S1 12 1 12z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                        {/if}
                      </button>
                      <button
                        type="button"
                        on:click={() => handleMoveBoxLayerUp(layer.id)}
                        disabled={index === 0}
                        class="h-7 w-7 inline-flex items-center justify-center rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed"
                        title={$t('moveUp')}
                        aria-label={$t('moveUp')}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 15 6-6 6 6"></path></svg>
                      </button>
                      <button
                        type="button"
                        on:click={() => handleMoveBoxLayerDown(layer.id)}
                        disabled={index === boxLayers.length - 1}
                        class="h-7 w-7 inline-flex items-center justify-center rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-40 disabled:cursor-not-allowed"
                        title={$t('moveDown')}
                        aria-label={$t('moveDown')}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"></path></svg>
                      </button>
                      <select
                        value={layer.activeVersionIndex}
                        on:change={(e) => handleSetBoxLayerVersion(layer.id, parseInt((e.currentTarget as HTMLSelectElement).value))}
                        disabled={layer.versionCount <= 0}
                        class="h-7 flex-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-[11px] text-gray-900 dark:text-white px-1 disabled:opacity-50"
                        title={$t('layerVersion')}
                        aria-label={$t('layerVersion')}
                      >
                        {#if layer.versionCount > 0}
                          {#each Array.from({ length: layer.versionCount }) as _, versionIndex}
                            <option value={versionIndex}>v{versionIndex + 1}</option>
                          {/each}
                        {:else}
                          <option value="0">v0</option>
                        {/if}
                      </select>
                      <button
                        type="button"
                        on:click={() => handleRegenerateBoxLayer(layer.id)}
                        disabled={!imagePath}
                        class="h-7 w-7 inline-flex items-center justify-center rounded border border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300 hover:bg-blue-50 dark:hover:bg-blue-900/30 disabled:opacity-50"
                        title={$t('regenerate')}
                        aria-label={$t('regenerate')}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2v6h-6"></path><path d="M3 12a9 9 0 0 1 15.5-6.36L21 8"></path><path d="M3 22v-6h6"></path><path d="M21 12a9 9 0 0 1-15.5 6.36L3 16"></path></svg>
                      </button>
                      <button
                        type="button"
                        on:click={() => handleDeleteBoxLayer(layer.id)}
                        class="h-7 w-7 inline-flex items-center justify-center rounded border border-red-300 dark:border-red-700 text-red-700 dark:text-red-300 hover:bg-red-50 dark:hover:bg-red-900/30"
                        title={$t('deleteLayer')}
                        aria-label={$t('deleteLayer')}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
                      </button>
                    </div>
                    {#if layer.status === 'error' && layer.errorMessage}
                      <div class="mt-1 text-[10px] text-red-600 dark:text-red-400 truncate">{layer.errorMessage}</div>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          </aside>
          <div class="relative h-full w-0 shrink-0">
            <button
              type="button"
              class="absolute -left-[3px] top-0 h-full w-[6px] cursor-col-resize group focus:outline-none"
              aria-label={$t('boxLayers')}
              title={$t('boxLayers')}
              on:mousedown={startBoxLayerSidebarResize}
            >
              <div class="absolute inset-y-0 left-1/2 -translate-x-1/2 w-px bg-gray-300 dark:bg-gray-700 group-hover:bg-blue-500 transition-colors"></div>
            </button>
          </div>

          <div class="flex-1 min-w-0 min-h-0 relative">
            <TileGrid 
              bind:this={tileGridRef}
              src={imagePath} 
              {rows} 
              {cols} 
              overlap={overlap}
              overlapXRatio={overlapXRatio}
              overlapYRatio={overlapYRatio}
              {aiOutputRes}
              {bgRemovalEnabled}
              {keyColor}
              {tolerance}
              nonBgBackgroundHex={nonBgBackgroundHex}
              {concurrency}
              {showTileLines}
              {isAdjustingGrid}
              {showOriginalInput}
              {boxGenerateMode}
              boxAspectMode={boxAspectMode}
              boxGenerateAspectRatio={boxGenerateAspectRatio}
              promptSubject={promptSubject}
              detectedSubject={promptSubject}
              bind:isProcessing 
              bind:resultSrc
              bind:exportTiles
              bind:exportOverlays
              bind:exportRegularLayers
              bind:boxLayers
              on:update_src={handleTileGridSourceUpdate}
              on:subject_input={handleTileGridSubjectInput}
              on:box_aspect_mode_change={handleTileGridBoxAspectModeChange}
              on:log={addLog}
            />

            <div class="absolute top-4 right-4 z-50 flex items-start gap-2 pointer-events-auto">
              {#if !bgRemovalEnabled}
                <div class="relative">
                  <button
                    type="button"
                    on:click={toggleToolbarBackgroundPopover}
                    class="relative bg-white/80 dark:bg-gray-800/80 hover:bg-white dark:hover:bg-gray-700 text-gray-900 dark:text-white p-3 rounded-full shadow-xl border border-gray-200 dark:border-gray-600 backdrop-blur-sm transition-all active:scale-90 select-none"
                    title={$t('backgroundColor')}
                    aria-label={$t('backgroundColor')}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="m2 22 1-1h3l9-9"></path><path d="M3 21v-3L14 7l3 3-11 11H3Z"></path><path d="m14 7 1.5-1.5a2.12 2.12 0 1 1 3 3L17 10"></path></svg>
                    <span
                      class="absolute right-2 bottom-2 w-2.5 h-2.5 rounded-full border border-white/80 dark:border-gray-900/80 shadow-sm"
                      style={`background-color: ${normalizeHexColor(nonBgBackgroundHex)};`}
                    ></span>
                  </button>
                  {#if showToolbarBackgroundPopover}
                    <div class="absolute top-14 right-0 w-72 rounded-xl border border-gray-200 dark:border-gray-700 bg-white/95 dark:bg-gray-800/95 backdrop-blur-md shadow-2xl p-3 flex flex-col gap-2">
                      <div class="text-xs font-semibold text-gray-700 dark:text-gray-200">{$t('backgroundColor')}</div>
                      <div class="flex flex-wrap gap-2">
                        <button type="button" on:click={() => nonBgColorMode = 'white'} title={$t('colorWhite')} aria-label={$t('colorWhite')} class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'white' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"><span class="w-5 h-5 rounded border border-gray-300" style="background-color: #FFFFFF"></span></button>
                        <button type="button" on:click={() => nonBgColorMode = 'black'} title={$t('colorBlack')} aria-label={$t('colorBlack')} class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'black' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"><span class="w-5 h-5 rounded border border-gray-300" style="background-color: #000000"></span></button>
                        <button type="button" on:click={() => nonBgColorMode = 'green'} title={$t('colorGreen')} aria-label={$t('colorGreen')} class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'green' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"><span class="w-5 h-5 rounded border border-gray-300" style="background-color: #00FF00"></span></button>
                        <button type="button" on:click={() => nonBgColorMode = 'blue'} title={$t('colorBlue')} aria-label={$t('colorBlue')} class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'blue' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"><span class="w-5 h-5 rounded border border-gray-300" style="background-color: #0000FF"></span></button>
                        <button type="button" on:click={() => nonBgColorMode = 'custom'} title={$t('colorCustom')} aria-label={$t('colorCustom')} class="h-8 px-2 flex items-center gap-1.5 rounded border text-xs transition-colors {nonBgColorMode === 'custom' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}">
                          <span class="text-gray-700 dark:text-gray-200">{$t('colorCustom')}</span>
                          <span class="w-5 h-5 rounded border border-gray-300" style={`background-color: ${normalizeHexColor(nonBgCustomHex)}`}></span>
                        </button>
                      </div>
                      {#if nonBgColorMode === 'custom'}
                        <div class="flex items-center gap-2">
                          <input type="color" value={normalizeHexColor(nonBgCustomHex)} on:input={(e) => nonBgCustomHex = (e.currentTarget as HTMLInputElement).value} class="w-10 h-8 rounded border border-gray-300 dark:border-gray-600 bg-transparent p-0">
                          <input type="text" bind:value={nonBgCustomHex} on:blur={() => nonBgCustomHex = normalizeHexColor(nonBgCustomHex)} placeholder="#FFFFFF" class="flex-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded p-1.5 text-xs font-mono text-gray-900 dark:text-white">
                        </div>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}

              <div class="relative">
                <button
                  type="button"
                  on:click={toggleToolbarSubjectPopover}
                  class="bg-white/80 dark:bg-gray-800/80 hover:bg-white dark:hover:bg-gray-700 text-gray-900 dark:text-white p-3 rounded-full shadow-xl border border-gray-200 dark:border-gray-600 backdrop-blur-sm transition-all active:scale-90 select-none"
                  title={$t('mainSubject')}
                  aria-label={$t('mainSubject')}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"></circle><path d="M4 22c0-4.4 3.6-8 8-8s8 3.6 8 8"></path></svg>
                </button>
                {#if showToolbarSubjectPopover}
                  <div class="absolute top-14 right-0 w-80 rounded-xl border border-gray-200 dark:border-gray-700 bg-white/95 dark:bg-gray-800/95 backdrop-blur-md shadow-2xl p-3 flex flex-col gap-2">
                    <div class="text-xs font-semibold text-gray-700 dark:text-gray-200">{$t('mainSubject')}</div>
                    {#if !imagePath}
                      <span class="text-xs text-gray-500 dark:text-gray-400">-</span>
                    {:else if subjectStatus === 'detecting'}
                      <span class="text-xs text-blue-600 dark:text-blue-400">{$t('detectingSubject')}</span>
                    {:else if subjectStatus === 'ready'}
                      <span class="text-sm font-medium text-gray-800 dark:text-gray-200">{detectedSubject}</span>
                    {:else if subjectStatus === 'no_api'}
                      <span class="text-xs text-gray-500 dark:text-gray-400">{$t('noApiKeySubject')}</span>
                    {:else}
                      <span class="text-xs text-red-600 dark:text-red-400">{$t('subjectDetectFailed')}{subjectError ? `: ${subjectError}` : ''}</span>
                    {/if}
                    {#if imagePath}
                      <input
                        type="text"
                        value={manualSubject}
                        on:input={onSubjectInput}
                        placeholder={$t('subjectPlaceholder')}
                        class="w-full bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded p-1.5 text-xs text-gray-900 dark:text-white transition-colors"
                      />
                      <div class="flex items-center justify-between gap-2">
                        <span class="text-[10px] text-gray-500 dark:text-gray-400 truncate">{$t('usingSubject')}: {promptSubject}</span>
                        <button
                          type="button"
                          class="text-[10px] px-2 py-1 rounded border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                          on:mousedown|preventDefault
                          on:click={useDetectedSubject}
                          disabled={!detectedSubject}
                        >
                          {$t('useDetectedSubject')}
                        </button>
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>

              {#if resultSrc}
                <button
                  on:mousedown={() => showOriginalInput = true}
                  on:mouseup={() => showOriginalInput = false}
                  on:mouseleave={() => showOriginalInput = false}
                  on:touchstart={() => showOriginalInput = true}
                  on:touchend={() => showOriginalInput = false}
                  class="bg-white/80 dark:bg-gray-800/80 hover:bg-white dark:hover:bg-gray-700 text-gray-900 dark:text-white p-3 rounded-full shadow-xl border border-gray-200 dark:border-gray-600 backdrop-blur-sm transition-all active:scale-90 select-none"
                  title={$t('holdToShowOriginal')}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    </section>
  </div>

  {#if showSettings}
    <Settings bind:concurrency bind:theme on:close={handleSettingsClose} />
  {/if}

  {#if showCropModal}
    <CropModal src={imagePath} on:cancel={() => showCropModal = false} on:done={handleCropDone} />
  {/if}

  {#if showFolderNameModal}
    <div class="fixed inset-0 z-[120] bg-black/50 backdrop-blur-sm flex items-center justify-center p-4">
      <div class="w-full max-w-md rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-2xl p-4 flex flex-col gap-3">
        <h3 class="font-semibold text-sm text-gray-900 dark:text-white">{$t('outputFolderNamePrompt')}</h3>
        <input
          type="text"
          bind:value={exportFolderNameInput}
          class="w-full rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-gray-900 dark:text-white px-3 py-2 text-sm"
          placeholder="image"
        />
        <div class="rounded border border-gray-200 dark:border-gray-700 p-3 flex flex-col gap-2">
          <div class="text-xs font-semibold text-gray-700 dark:text-gray-200">{$t('exportContentsPrompt')}</div>
          <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-200">
            <input
              type="checkbox"
              bind:checked={exportIncludeMerged}
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <span>{$t('exportItemMerged')}</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-200">
            <input
              type="checkbox"
              bind:checked={exportIncludeTiles}
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <span>{$t('exportItemTiles')}</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-200">
            <input
              type="checkbox"
              bind:checked={exportIncludePsd}
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <span>{$t('exportItemPsd')}</span>
          </label>
          <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-200 pt-1 border-t border-gray-200 dark:border-gray-700">
            <input
              type="checkbox"
              bind:checked={exportSetAsDefault}
              class="rounded border-gray-300 dark:border-gray-600"
            />
            <span>{$t('exportSetDefault')}</span>
          </label>
          {#if exportSelectionError}
            <div class="text-[11px] text-red-600 dark:text-red-400">{exportSelectionError}</div>
          {/if}
        </div>
        <div class="flex items-center justify-end gap-2">
          <button
            type="button"
            on:click={cancelFolderNameModal}
            class="px-3 py-1.5 rounded bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-xs"
          >
            {$t('settings.cancel')}
          </button>
          <button
            type="button"
            on:click={confirmFolderNameModal}
            class="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold"
          >
            {$t('save')}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if exportAlertVisible}
    <div class="fixed inset-0 z-[120] bg-black/50 backdrop-blur-sm flex items-center justify-center p-4">
      <div class="w-full max-w-lg rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-2xl p-4 flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <h3 class="font-semibold text-sm text-gray-900 dark:text-white">{$t('exportProgressTitle')}</h3>
          <button
            type="button"
            on:click={closeExportAlert}
            class="p-1.5 rounded bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-200"
            aria-label={$t('settings.cancel')}
            title={$t('settings.cancel')}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="w-full h-2 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden">
          <div
            class="h-full bg-blue-600 transition-all duration-300"
            style="width: {exportProgress}%"
          ></div>
        </div>
        <div class="text-xs text-gray-600 dark:text-gray-300">{exportMessage}</div>

        {#if exportError}
          <div class="text-xs text-red-600 dark:text-red-400">{exportError}</div>
        {/if}

        {#if exportDone && !exportError}
          <div class="text-xs text-gray-700 dark:text-gray-200 break-all">{exportMergedPath || exportDirPath}</div>
          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              on:click={openExportDirectory}
              class="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold"
            >
              {$t('openFolder')}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</main>
