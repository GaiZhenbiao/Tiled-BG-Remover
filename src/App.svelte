<script lang="ts">
  import { onMount } from 'svelte';
  import ImageUploader from './lib/ImageUploader.svelte';
  import TileGrid from './lib/TileGrid.svelte';
  import Settings from './lib/Settings.svelte';
  import CropModal from './lib/CropModal.svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';

  let imagePath = '';
  let showSettings = false;
  let showCropModal = false;
  
  // Sidebar Tabs
  let activeTab = 'controls'; // 'controls' or 'logs'
  let logs: { type: string, message: string, time: string }[] = [];
  
  // State
  let rows = 2;
  let cols = 2;
  let overlap = 0.1;
  let aiOutputRes = 1024;
  let resizeInputToOutput = true;
  let smartGridEnabled = true;
  
  // BG Removal State
  let bgRemovalEnabled = false;
  let keyColor = localStorage.getItem('key_color') || 'green';
  let tolerance = parseInt(localStorage.getItem('key_tolerance') || '10');

  onMount(() => {
    if (localStorage.getItem('bg_removal_enabled') !== null) {
      bgRemovalEnabled = localStorage.getItem('bg_removal_enabled') === 'true';
    }
  });
  
  // Processing state
  let isProcessing = false;
  let resultSrc = '';
  
  // Image Info
  let imgWidth = 0;
  let imgHeight = 0;
  
  $: if (imagePath) {
    updateImageInfo();
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

  function clearInput() {
    imagePath = '';
    resultSrc = '';
    logs = [];
  }

  $: selectedModel = localStorage.getItem('gemini_model') || 'gemini-1.5-pro';
  
  $: availableResolutions = selectedModel.includes('gemini-3-pro') 
    ? [1024, 2048, 4096] 
    : [1024];

  $: if (!availableResolutions.includes(aiOutputRes)) {
    aiOutputRes = availableResolutions[0];
  }

  // Smart Grid Logic
  $: if (smartGridEnabled && imgWidth && imgHeight && aiOutputRes) {
    const O = overlap;
    let optimalRows = 1;
    let optimalCols = 1;
    
    // Iterate to find smallest cols that satisfies condition
    for (let c = 1; c <= 16; c++) {
        const T_w = imgWidth / (c - (c - 1) * O);
        if (T_w <= 2 * aiOutputRes) {
            optimalCols = c;
            break;
        }
        optimalCols = c; 
    }
    
    for (let r = 1; r <= 16; r++) {
        const T_h = imgHeight / (r - (r - 1) * O);
        if (T_h <= 2 * aiOutputRes) {
            optimalRows = r;
            break;
        }
        optimalRows = r;
    }
    
    cols = optimalCols;
    rows = optimalRows;
  }
  
  // Resolution Info
  $: tileW = imgWidth / (cols - (cols - 1) * overlap);
  $: tileH = imgHeight / (rows - (rows - 1) * overlap);

  function handleImageSelected(path: string) {
    imagePath = path;
    resultSrc = '';
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
  }

  function addLog(e: any) {
    logs = [{ ...e.detail, time: new Date().toLocaleTimeString() }, ...logs].slice(0, 100);
  }

  async function saveResult() {
    if (!resultSrc) return;
    const path = await save({
      filters: [{ name: 'Image', extensions: ['png'] }],
      defaultPath: 'upscaled_image.png'
    });
    if (path) {
      await invoke('save_merged_image', { path, base64Data: resultSrc });
      logs = [{ type: 'success', message: `Image saved to ${path}`, time: new Date().toLocaleTimeString() }, ...logs];
    }
  }

  function toggleBGRemoval() {
    bgRemovalEnabled = !bgRemovalEnabled;
    localStorage.setItem('bg_removal_enabled', bgRemovalEnabled.toString());
  }

  function setKeyColor(color: string) {
    keyColor = color;
    localStorage.setItem('key_color', color);
  }

  function setTolerance(e: any) {
    tolerance = parseInt(e.target.value);
    localStorage.setItem('key_tolerance', tolerance.toString());
  }
</script>

<main class="h-screen w-screen flex flex-col bg-gray-900 text-white overflow-hidden">
  <!-- Header -->
  <header class="h-12 border-b border-gray-700 flex items-center justify-between px-4 bg-gray-800">
    <div class="font-bold text-lg flex items-center gap-2">
      <span class="text-blue-400">Gemini</span> Tile Upscaler
    </div>
    <div class="flex items-center gap-2">
      <!-- BG Removal Toggle -->
      <div class="flex items-center bg-gray-700 rounded-full px-2 py-1 mr-2 border border-gray-600">
        <label class="flex items-center gap-2 cursor-pointer">
          <span class="text-xs font-semibold text-gray-300 uppercase">BG Removal</span>
          <input type="checkbox" checked={bgRemovalEnabled} on:change={toggleBGRemoval} class="toggle toggle-sm accent-blue-500">
        </label>
      </div>

      {#if imagePath}
        <button on:click={clearInput} class="bg-red-600/80 hover:bg-red-600 px-3 py-1 rounded text-sm flex items-center gap-2" title="Clear Input Image">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        </button>
      {/if}

      {#if resultSrc}
        <button on:click={() => resultSrc = ''} class="bg-gray-700 hover:bg-gray-600 px-3 py-1 rounded text-sm flex items-center gap-2 border border-gray-600">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 4v6h6"></path><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path></svg>
          Revert
        </button>
        <button on:click={saveResult} class="bg-green-600 hover:bg-green-500 px-3 py-1 rounded text-sm flex items-center gap-2">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline points="7 3 7 8 15 8"></polyline></svg>
          Save
        </button>
      {/if}
      <button on:click={() => showSettings = true} class="p-2 hover:bg-gray-700 rounded-full">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
      </button>
    </div>
  </header>

  <!-- Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar Controls -->
    <aside class="w-80 bg-gray-800 border-r border-gray-700 flex flex-col overflow-hidden">
      <!-- Tabs Header -->
      <div class="flex border-b border-gray-700">
        <button 
          on:click={() => activeTab = 'controls'}
          class="flex-1 py-2 text-sm font-medium {activeTab === 'controls' ? 'bg-gray-700 text-blue-400 border-b-2 border-blue-400' : 'text-gray-400 hover:bg-gray-700'}"
        >
          Controls
        </button>
        <button 
          on:click={() => activeTab = 'logs'}
          class="flex-1 py-2 text-sm font-medium {activeTab === 'logs' ? 'bg-gray-700 text-blue-400 border-b-2 border-blue-400' : 'text-gray-400 hover:bg-gray-700'}"
        >
          Logs ({logs.length})
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-6">
        {#if activeTab === 'controls'}
          {#if imagePath}
            <div class="flex flex-col gap-2">
              <label class="text-xs font-semibold text-gray-400 uppercase">Tools</label>
              <button on:click={() => showCropModal = true} class="bg-gray-700 hover:bg-gray-600 text-white text-sm py-1.5 rounded flex items-center justify-center gap-2">
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6.13 1L6 16a2 2 0 0 0 2 2h15"></path><path d="M1 6.13L16 6a2 2 0 0 1 2 2v15"></path></svg>
                Crop Image
              </button>
              <div class="flex flex-wrap gap-1 mt-1">
                <button on:click={() => performCrop(Math.round((imgWidth - Math.min(imgWidth, imgHeight)) / 2), Math.round((imgHeight - Math.min(imgWidth, imgHeight)) / 2), Math.min(imgWidth, imgHeight), Math.min(imgWidth, imgHeight))} class="text-[10px] bg-gray-700 hover:bg-gray-600 px-2 py-0.5 rounded text-gray-300">1:1 Center</button>
              </div>
            </div>

            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <label class="text-xs font-semibold text-gray-400 uppercase">Grid Layout</label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <span class="text-[10px] text-gray-400">Smart</span>
                  <input type="checkbox" bind:checked={smartGridEnabled} class="toggle toggle-xs accent-blue-500">
                </label>
              </div>
              
              {#if smartGridEnabled}
                <div class="bg-gray-900/50 p-2 rounded border border-gray-700 flex flex-col gap-1">
                  <div class="flex justify-between text-xs">
                    <span class="text-gray-500">Rows</span>
                    <span class="font-mono">{rows}</span>
                  </div>
                  <div class="flex justify-between text-xs">
                    <span class="text-gray-500">Cols</span>
                    <span class="font-mono">{cols}</span>
                  </div>
                </div>
              {:else}
                <div class="flex gap-2 items-center">
                  <span class="w-8 text-sm">Rows</span>
                  <input type="range" min="1" max="16" bind:value={rows} class="flex-1 accent-blue-500">
                  <span class="w-4 text-sm text-right">{rows}</span>
                </div>
                <div class="flex gap-2 items-center">
                  <span class="w-8 text-sm">Cols</span>
                  <input type="range" min="1" max="16" bind:value={cols} class="flex-1 accent-blue-500">
                  <span class="w-4 text-sm text-right">{cols}</span>
                </div>
              {/if}
              
              <div class="flex flex-col gap-1">
                <div class="flex justify-between items-center">
                  <span class="text-xs text-gray-400">Overlap ({Math.round(overlap*100)}%)</span>
                  <input type="range" min="0" max="0.5" step="0.05" bind:value={overlap} class="w-32 accent-blue-500">
                </div>
              </div>
            </div>
            
            <div class="flex flex-col gap-2">
              <label class="text-xs font-semibold text-gray-400 uppercase">AI Output Resolution</label>
              <select bind:value={aiOutputRes} class="bg-gray-700 border border-gray-600 rounded p-1.5 text-sm">
                {#each availableResolutions as res}
                  <option value={res}>{res} x {res}</option>
                {/each}
              </select>
              <label class="flex items-center gap-2 cursor-pointer mt-1">
                <input type="checkbox" bind:checked={resizeInputToOutput} class="accent-blue-500">
                <span class="text-xs text-gray-300">Resize input tile to match output</span>
              </label>
            </div>

            {#if bgRemovalEnabled}
              <div class="flex flex-col gap-2">
                <div class="flex items-center justify-between">
                  <label class="text-xs font-semibold text-gray-400 uppercase">Key Color</label>
                </div>
                <div class="flex gap-1">
                    {#each ['green', 'red', 'blue', 'black', 'white'] as color}
                      <button 
                        on:click={() => setKeyColor(color)}
                        class="w-5 h-5 rounded-full border border-gray-600 {keyColor === color ? 'ring-2 ring-blue-400' : ''}"
                        style="background-color: {color === 'white' ? '#fff' : color === 'black' ? '#000' : color}"
                        title={color}
                      ></button>
                    {/each}
                </div>
                
                <div class="flex flex-col gap-1 mt-2">
                  <div class="flex justify-between items-center">
                    <span class="text-xs text-gray-400">Tolerance ({tolerance})</span>
                    <input type="range" min="0" max="100" value={tolerance} on:input={setTolerance} class="w-32 accent-blue-500">
                  </div>
                </div>
              </div>
            {/if}

            <div class="bg-gray-900/50 p-3 rounded border border-gray-700 flex flex-col gap-2">
              <label class="text-[10px] font-bold text-gray-500 uppercase tracking-wider">Resolution Info</label>
              <div class="flex flex-col gap-1">
                <div class="flex justify-between text-xs">
                  <span class="text-gray-400">Whole Image</span>
                  <span class="font-mono text-gray-200">{imgWidth} x {imgHeight}</span>
                </div>
                <div class="flex justify-between text-xs">
                  <span class="text-gray-400">Per Tile</span>
                  <span class="font-mono text-gray-200">{Math.round(tileW)} x {Math.round(tileH)}</span>
                </div>
              </div>
            </div>

            <hr class="border-gray-700">

            <button 
              on:click={() => isProcessing = true}
              class="bg-blue-600 hover:bg-blue-500 text-white py-2.5 rounded font-bold shadow-lg disabled:opacity-50 disabled:cursor-not-allowed transition-all active:scale-95"
              disabled={isProcessing}
            >
              {isProcessing ? 'Processing Tiles...' : 'Process All Tiles'}
            </button>
          {/if}
        {:else}
          <!-- Logs Tab -->
          <div class="flex flex-col gap-2 font-mono text-[11px]">
            {#if logs.length === 0}
              <div class="text-gray-500 text-center py-8 italic">No logs yet...</div>
            {/if}
            {#each logs as log}
              <div class="flex gap-2 border-b border-gray-700/50 pb-1">
                <span class="text-gray-600">[{log.time}]</span>
                <span class={log.type === 'error' ? 'text-red-400' : log.type === 'success' ? 'text-green-400' : 'text-blue-400'}>
                  {log.message}
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </aside>

    <!-- Main View -->
    <section class="flex-1 bg-gray-900 relative flex items-center justify-center p-4">
      {#if !imagePath}
        <ImageUploader on:selected={(e) => handleImageSelected(e.detail)} />
      {:else}
        <TileGrid 
          src={imagePath} 
          {rows} 
          {cols} 
          {overlap} 
          {aiOutputRes}
          {resizeInputToOutput}
          {bgRemovalEnabled}
          {keyColor}
          {tolerance}
          bind:isProcessing 
          bind:resultSrc
          on:log={addLog}
          on:update_src={(e) => imagePath = e.detail}
        />
      {/if}
    </section>
  </div>

  {#if showSettings}
    <Settings on:close={() => showSettings = false} />
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
    background: #4b5563;
    border-radius: 1rem;
    position: relative;
    cursor: pointer;
    transition: background 0.2s;
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