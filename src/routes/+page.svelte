<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import ImageUploader from '../lib/ImageUploader.svelte';
  import TileGrid from '../lib/TileGrid.svelte';
  import Settings from '../lib/Settings.svelte';
  import CropModal from '../lib/CropModal.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { t } from '../lib/i18n';
  import { detectMainSubject } from '../lib/api';

  let imagePath = '';
  let originalFilename = '';
  let showSettings = false;
  let showCropModal = false;
  type ThemeMode = 'auto' | 'light' | 'dark';
  let theme = parseThemeMode(localStorage.getItem('theme'));
  let themeMediaQuery: MediaQueryList | null = null;
  let systemPrefersDark = false;
  
  // Sidebar Tabs
  let activeTab = 'controls'; // 'controls' or 'logs'
  let logs: { type: string, message: string, time: string }[] = [];
  let selectedModel = localStorage.getItem('gemini_model') || 'gemini-2.5-flash-image';
  
  // State
  let rows = 2;
  let cols = 2;
  let overlap = 0;
  let aiOutputRes = selectedModel.includes('gemini-3-pro') ? 2048 : 1024;
  let concurrency = 2;
  let smartGridEnabled = true;
  const smartGridMaxCount = 64;
  let smartTileTolerancePx = clampInt(
    parseInt(localStorage.getItem('smart_tile_tolerance_px') || String(aiOutputRes * 2)),
    1,
    20000
  );
  let smartQualitySliderPx = smartTileTolerancePx;
  let showTileLines = localStorage.getItem('show_tile_lines') === 'true';
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
  });

  function handleSettingsClose() {
    showSettings = false;
    bgRemovalEnabled = localStorage.getItem('bg_removal_enabled') === 'true';
    keyColor = localStorage.getItem('key_color') || 'green';
    tolerance = parseInt(localStorage.getItem('key_tolerance') || '10');
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
  }

  $: if (theme) {
    localStorage.setItem('theme', theme);
    applyTheme();
  }

  $: localStorage.setItem('smart_tile_tolerance_px', smartTileTolerancePx.toString());
  $: localStorage.setItem('show_tile_lines', showTileLines.toString());
  $: localStorage.setItem('non_bg_color_mode', nonBgColorMode);
  $: localStorage.setItem('non_bg_custom_hex', normalizeHexColor(nonBgCustomHex));
  $: nonBgBackgroundHex = resolveNonBgHex(nonBgColorMode, nonBgCustomHex);
  
  // Processing state
  let isProcessing = false;
  let resultSrc = '';
  let exportTiles: any[] = [];
  
  // Image Info
  let imgWidth = 0;
  let imgHeight = 0;
  let detectedSubject = '';
  let manualSubject = '';
  let userEditedSubject = false;
  let subjectStatus: 'idle' | 'detecting' | 'ready' | 'no_api' | 'error' = 'idle';
  let subjectError = '';
  let subjectDetectSeq = 0;
  $: promptSubject = (manualSubject.trim() || detectedSubject || 'main subject');
  
  $: if (imagePath) {
    updateImageInfo();
  }

  function clearInput() {
    imagePath = '';
    resultSrc = '';
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
    selectedModel = localStorage.getItem('gemini_model') || 'gemini-2.5-flash-image';
  }

  let previousModel = selectedModel;
  $: if (selectedModel !== previousModel) {
    aiOutputRes = selectedModel.includes('gemini-3-pro') ? 2048 : 1024;
    previousModel = selectedModel;
  }

  $: availableResolutions = selectedModel.includes('gemini-3-pro') 
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
  
  // Resolution Info
  $: tileW = imgWidth / (cols - (cols - 1) * overlap);
  $: tileH = imgHeight / (rows - (rows - 1) * overlap);
  $: totalTiles = rows * cols;

  function handleImageSelected(path: string) {
    imagePath = path;
    resultSrc = '';
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
    performCrop(x, y, width, height);
    showCropModal = false;
  }

  async function performCrop(x: number, y: number, width: number, height: number) {
     const newPath: string = await invoke('crop_img', {
       path: imagePath, x, y, width, height
     });
     imagePath = newPath;
     resultSrc = '';
     manualSubject = '';
     userEditedSubject = false;
     startSubjectDetection(newPath);
  }

  function onSubjectInput(event: Event) {
    const target = event.target as HTMLInputElement;
    manualSubject = target.value;
    userEditedSubject = true;
  }

  function useDetectedSubject() {
    manualSubject = detectedSubject || '';
    userEditedSubject = false;
  }

  function addLog(e: any) {
    logs = [{ ...e.detail, time: new Date().toLocaleTimeString() }, ...logs].slice(0, 100);
  }

  async function saveResult() {
    if (!resultSrc) return;

    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: originalFilename || undefined
    });
    const exportParentDir = Array.isArray(selected) ? selected[0] : selected;
    if (typeof exportParentDir === 'string' && exportParentDir.length > 0) {
      const localizedSuffix = String($t('bgRemovedSuffix'));

      if (exportTiles.length > 0) {
        const verboseLogging = localStorage.getItem('verbose_logging') === 'true';
        const exportRes: any = await invoke('save_export_bundle', {
          outputDir: exportParentDir,
          mergedBase64: resultSrc,
          tiles: exportTiles,
          sourcePath: imagePath,
          inputName: originalFilename || 'image',
          removeBg: bgRemovalEnabled,
          localizedSuffix,
          verboseLogging
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
      } else {
        const ext = bgRemovalEnabled ? 'png' : 'jpg';
        const stem = originalFilename
          ? `${originalFilename}_${localizedSuffix}`
          : `upscaled_${localizedSuffix}`;
        const mergedPath = `${exportParentDir}/${stem}.${ext}`;
        await invoke('save_merged_image', { path: mergedPath, base64Data: resultSrc });
        logs = [{
          type: 'success',
          message: `Image saved to ${mergedPath}`,
          time: new Date().toLocaleTimeString()
        }, ...logs];
      }
    }
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
        const res = await fetch(b64);
        const blob = await res.blob();
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
          class="bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-gray-800 dark:text-white px-3 py-1 rounded text-sm flex items-center gap-2 border border-gray-300 dark:border-gray-600 transition-colors"
          title={$t('cropImage')}
          aria-label={$t('cropImage')}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6.13 1L6 16a2 2 0 0 0 2 2h15"></path><path d="M1 6.13L16 6a2 2 0 0 1 2 2v15"></path></svg>
        </button>

        <button on:click={clearInput} class="bg-red-600/80 hover:bg-red-600 text-white px-3 py-1 rounded text-sm flex items-center gap-2" title="Clear Input Image">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        </button>
      {/if}

      {#if resultSrc}
        <button on:click={() => resultSrc = ''} class="bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-gray-800 dark:text-white px-3 py-1 rounded text-sm flex items-center gap-2 border border-gray-300 dark:border-gray-600 transition-colors">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 4v6h6"></path><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path></svg>
          {$t('revert')}
        </button>
        <button on:click={saveResult} class="bg-green-600 hover:bg-green-500 text-white px-3 py-1 rounded text-sm flex items-center gap-2">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline points="7 3 7 8 15 8"></polyline></svg>
          {$t('save')}
        </button>
      {/if}
      <button 
        aria-label="Settings"
        on:click={() => showSettings = true} 
        class="p-2 hover:bg-gray-200 dark:hover:bg-gray-700 rounded-full transition-colors"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
      </button>
    </div>
  </header>

  <!-- Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar Controls -->
    <aside class="w-80 shrink-0 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden transition-colors">
      <!-- Tabs Header -->
      <div class="flex border-b border-gray-200 dark:border-gray-700">
        <button 
          on:click={() => activeTab = 'controls'}
          class="flex-1 py-2 text-sm font-medium {activeTab === 'controls' ? 'bg-gray-100 dark:bg-gray-700 text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
        >
          {$t('controls')}
        </button>
        <button 
          on:click={() => activeTab = 'logs'}
          class="flex-1 py-2 text-sm font-medium {activeTab === 'logs' ? 'bg-gray-100 dark:bg-gray-700 text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'}"
        >
          {$t('logs')} ({logs.length})
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-6">
        {#if activeTab === 'controls'}
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <label for="grid-rows" class="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">{$t('gridLayout')}</label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <span class="text-[10px] text-gray-500 dark:text-gray-400">{$t('smartGrid')}</span>
                  <input type="checkbox" bind:checked={smartGridEnabled} class="toggle toggle-xs accent-blue-500">
                </label>
              </div>
              
              {#if smartGridEnabled}
                <div class="bg-gray-50 dark:bg-gray-900/50 p-2 rounded border border-gray-200 dark:border-gray-700 flex flex-col gap-2 transition-colors">
                  <div class="flex justify-between text-xs">
                    <span class="text-gray-500">{$t('rows')}</span>
                    <span class="font-mono text-gray-700 dark:text-gray-200">{rows}</span>
                  </div>
                  <div class="flex justify-between text-xs">
                    <span class="text-gray-500">{$t('cols')}</span>
                    <span class="font-mono text-gray-700 dark:text-gray-200">{cols}</span>
                  </div>
                  <div class="flex justify-between items-center text-xs mt-1">
                    <span class="text-gray-500 dark:text-gray-400">{$t('tileCount')}</span>
                    <span class="font-mono text-gray-700 dark:text-gray-200">{totalTiles}</span>
                  </div>
                  <div class="flex justify-between text-[10px] text-gray-500 dark:text-gray-400">
                    <span>{$t('lowerQuality')}</span>
                    <span>{$t('higherQuality')}</span>
                  </div>
                  <input
                    type="range"
                    min={smartTileLimitMin}
                    max={smartTileLimitMax}
                    step="1"
                    value={smartQualitySliderPx}
                    on:input={onSmartQualityInput}
                    class="w-full accent-blue-500"
                  >
                </div>
              {:else}
                <div class="flex gap-2 items-center">
                  <span class="w-8 text-sm text-gray-600 dark:text-gray-300">{$t('rows')}</span>
                  <input id="grid-rows" type="range" min="1" max="16" bind:value={rows} on:input={markGridAdjusting} class="flex-1 accent-blue-500">
                  <span class="w-4 text-sm text-right font-mono text-gray-700 dark:text-gray-200">{rows}</span>
                </div>
                <div class="flex gap-2 items-center">
                  <span class="w-8 text-sm text-gray-600 dark:text-gray-300">{$t('cols')}</span>
                  <input id="grid-cols" type="range" min="1" max="16" bind:value={cols} on:input={markGridAdjusting} class="flex-1 accent-blue-500">
                  <span class="w-4 text-sm text-right font-mono text-gray-700 dark:text-gray-200">{cols}</span>
                </div>
              {/if}
              
              <div class="flex flex-col gap-1">
                <div class="flex justify-between items-center">
                  <span class="text-xs text-gray-500 dark:text-gray-400">{$t('overlap')} ({Math.round(overlap*100)}%)</span>
                  <input id="overlap-slider" type="range" min="0" max="0.5" step="0.05" bind:value={overlap} on:input={markGridAdjusting} class="w-32 accent-blue-500">
                </div>
              </div>

            </div>
            
            <div class="flex flex-col gap-2">
              <label for="tile-res-select" class="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase">{$t('aiOutputRes')}</label>
              <select id="tile-res-select" bind:value={aiOutputRes} class="bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded p-1.5 text-sm text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 transition-colors">
                {#each availableResolutions as res}
                  <option value={res}>{res} x {res}</option>
                {/each}
              </select>
            </div>

            {#if !bgRemovalEnabled}
              <div class="bg-gray-50 dark:bg-gray-900/50 p-3 rounded border border-gray-200 dark:border-gray-700 flex flex-col gap-2 transition-colors">
                <span class="text-[10px] font-bold text-gray-400 dark:text-gray-500 uppercase tracking-wider">{$t('backgroundColor')}</span>
                <div class="flex flex-wrap gap-2">
                  <button
                    type="button"
                    on:click={() => nonBgColorMode = 'white'}
                    title={$t('colorWhite')}
                    aria-label={$t('colorWhite')}
                    class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'white' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"
                  >
                    <span class="w-5 h-5 rounded border border-gray-300" style="background-color: #FFFFFF"></span>
                  </button>
                  <button
                    type="button"
                    on:click={() => nonBgColorMode = 'black'}
                    title={$t('colorBlack')}
                    aria-label={$t('colorBlack')}
                    class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'black' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"
                  >
                    <span class="w-5 h-5 rounded border border-gray-300" style="background-color: #000000"></span>
                  </button>
                  <button
                    type="button"
                    on:click={() => nonBgColorMode = 'green'}
                    title={$t('colorGreen')}
                    aria-label={$t('colorGreen')}
                    class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'green' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"
                  >
                    <span class="w-5 h-5 rounded border border-gray-300" style="background-color: #00FF00"></span>
                  </button>
                  <button
                    type="button"
                    on:click={() => nonBgColorMode = 'blue'}
                    title={$t('colorBlue')}
                    aria-label={$t('colorBlue')}
                    class="w-8 h-8 flex items-center justify-center rounded border transition-colors {nonBgColorMode === 'blue' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"
                  >
                    <span class="w-5 h-5 rounded border border-gray-300" style="background-color: #0000FF"></span>
                  </button>
                  <button
                    type="button"
                    on:click={() => nonBgColorMode = 'custom'}
                    title={$t('colorCustom')}
                    aria-label={$t('colorCustom')}
                    class="h-8 px-2 flex items-center gap-1.5 rounded border text-xs transition-colors {nonBgColorMode === 'custom' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/30' : 'border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800'}"
                  >
                    <span class="text-gray-700 dark:text-gray-200">{$t('colorCustom')}</span>
                    <span class="w-5 h-5 rounded border border-gray-300" style={`background-color: ${normalizeHexColor(nonBgCustomHex)}`}></span>
                  </button>
                </div>
                {#if nonBgColorMode === 'custom'}
                  <div class="flex items-center gap-2">
                    <input
                      type="color"
                      value={normalizeHexColor(nonBgCustomHex)}
                      on:input={(e) => nonBgCustomHex = (e.currentTarget as HTMLInputElement).value}
                      class="w-10 h-8 rounded border border-gray-300 dark:border-gray-600 bg-transparent p-0"
                    >
                    <input
                      type="text"
                      bind:value={nonBgCustomHex}
                      on:blur={() => nonBgCustomHex = normalizeHexColor(nonBgCustomHex)}
                      placeholder="#FFFFFF"
                      class="flex-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded p-1.5 text-xs font-mono text-gray-900 dark:text-white"
                    >
                  </div>
                {/if}
              </div>
            {/if}

            <div class="bg-gray-50 dark:bg-gray-900/50 p-3 rounded border border-gray-200 dark:border-gray-700 flex flex-col gap-2 transition-colors">
              <span class="text-[10px] font-bold text-gray-400 dark:text-gray-500 uppercase tracking-wider">{$t('resolutionInfo')}</span>
              <div class="flex flex-col gap-1">
                <div class="flex justify-between text-xs">
                  <span class="text-gray-500 dark:text-gray-400">{$t('wholeImage')}</span>
                  <span class="font-mono text-gray-800 dark:text-gray-200">{imgWidth} x {imgHeight}</span>
                </div>
                <div class="flex justify-between text-xs">
                  <span class="text-gray-500 dark:text-gray-400">{$t('perTile')}</span>
                  <span class="font-mono text-gray-800 dark:text-gray-200">{Math.round(tileW)} x {Math.round(tileH)}</span>
                </div>
              </div>
            </div>

            <div class="bg-gray-50 dark:bg-gray-900/50 p-3 rounded border border-gray-200 dark:border-gray-700 flex flex-col gap-2 transition-colors">
              <span class="text-[10px] font-bold text-gray-400 dark:text-gray-500 uppercase tracking-wider">{$t('mainSubject')}</span>
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
                <div class="mt-1 flex flex-col gap-2">
                  <label for="subject-input" class="text-[10px] font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                    {$t('subjectForPrompt')}
                  </label>
                  <input
                    id="subject-input"
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
                </div>
              {/if}
            </div>

            <hr class="border-gray-200 dark:border-gray-700">

            <button 
              on:click={() => isProcessing = true}
              class="bg-blue-600 hover:bg-blue-500 text-white py-2.5 rounded font-bold shadow-lg disabled:opacity-50 disabled:cursor-not-allowed transition-all active:scale-95"
              disabled={isProcessing || !imagePath}
            >
              {isProcessing ? $t('processing') : $t('processAll')}
            </button>
        {:else}
          <!-- Logs Tab -->
          <div class="flex flex-col gap-2 font-mono text-[11px]">
            {#if logs.length === 0}
              <div class="text-gray-400 dark:text-gray-500 text-center py-8 italic">No logs yet...</div>
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
        {/if}
      </div>
    </aside>

    <!-- Main View -->
    <section class="flex-1 min-w-0 min-h-0 bg-gray-50 dark:bg-gray-900 relative overflow-hidden transition-colors">
      {#if !imagePath}
        <div class="w-full h-full flex items-center justify-center p-6">
          <ImageUploader on:selected={(e) => handleImageSelected(e.detail)} />
        </div>
      {:else}
        <TileGrid 
          src={imagePath} 
          {rows} 
          {cols} 
          {overlap} 
          {aiOutputRes}
          {bgRemovalEnabled}
          {keyColor}
          {tolerance}
          nonBgBackgroundHex={nonBgBackgroundHex}
          {concurrency}
          {showTileLines}
          {isAdjustingGrid}
          {showOriginalInput}
          detectedSubject={promptSubject}
          bind:isProcessing 
          bind:resultSrc
          bind:exportTiles
          on:log={addLog}
        />

        <div class="absolute top-4 right-4 z-50 flex items-center gap-2 pointer-events-auto">
          <button
            type="button"
            on:click={() => showTileLines = !showTileLines}
            class="p-3 rounded-full shadow-xl border backdrop-blur-sm transition-all active:scale-90 select-none {showTileLines ? 'bg-blue-600 text-white border-blue-500 hover:bg-blue-500' : 'bg-white/80 dark:bg-gray-800/80 hover:bg-white dark:hover:bg-gray-700 text-gray-900 dark:text-white border-gray-200 dark:border-gray-600'}"
            title={$t('showTileLines')}
            aria-label={$t('showTileLines')}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="3" y="3" width="18" height="18" rx="2"></rect>
              <path d="M3 9h18"></path>
              <path d="M9 21V3"></path>
            </svg>
          </button>

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
      {/if}
    </section>
  </div>

  {#if showSettings}
    <Settings bind:concurrency bind:theme on:close={handleSettingsClose} />
  {/if}

  {#if showCropModal}
    <CropModal src={imagePath} on:cancel={() => showCropModal = false} on:done={handleCropDone} />
  {/if}
</main>

<style>
  .toggle {
    appearance: none;
    width: 2rem;
    height: 1rem;
    background: #d1d5db; /* gray-300 */
    border-radius: 1rem;
    position: relative;
    cursor: pointer;
    transition: background 0.2s;
  }
  
  :global(.dark) .toggle {
    background: #4b5563; /* gray-600 */
  }

  .toggle:checked {
    background: #3b82f6;
  }
  .toggle::after {
    content: '';
    position: absolute;
    top: 0.125rem;
    left: 0.125rem;
    width: 0.75rem;
    height: 0.75rem;
    background: white;
    border-radius: 50%;
    transition: left 0.2s;
  }
  .toggle:checked::after {
    left: 1.125rem;
  }
</style>
