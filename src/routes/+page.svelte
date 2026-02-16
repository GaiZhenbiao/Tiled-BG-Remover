<script lang="ts">
  import { onMount } from 'svelte';
  import ImageUploader from '../lib/ImageUploader.svelte';
  import TileGrid from '../lib/TileGrid.svelte';
  import Settings from '../lib/Settings.svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { t } from '../lib/i18n';

  let imagePath = '';
  let showSettings = false;
  
  // State
  let rows = 2;
  let cols = 2;
  let overlap = 0.1;
  let tileRes = 1024;
  
  // Processing state
  let isProcessing = false;
  let logs: { type: string, message: string, time: string }[] = [];
  
  function handleImageSelected(path: string) {
    imagePath = path;
  }
  
  function handleLog(e: CustomEvent) {
    logs = [{ ...e.detail, time: new Date().toLocaleTimeString() }, ...logs].slice(0, 20);
  }
  
  async function cropToSquare() {
     if (!imagePath) return;
     // Load image to get dims
     const img = new Image();
     // We use load_image in TileGrid, here we just need dims. 
     // Using convertFileSrc might fail if asset protocol restricted, but we configured fs:scope-home-recursive.
     // However, simpler to just assume TileGrid handles display.
     // For dimensions, we can try to load it.
     img.src = convertFileSrc(imagePath);
     await new Promise(r => img.onload = r);
     
     const size = Math.min(img.naturalWidth, img.naturalHeight);
     const x = Math.round((img.naturalWidth - size) / 2);
     const y = Math.round((img.naturalHeight - size) / 2);
     
     const newPath: string = await invoke('crop_img', {
       path: imagePath, x, y, width: size, height: size
     });
     
     imagePath = newPath;
  }
</script>

<main class="h-screen w-screen flex flex-col bg-gray-900 text-white overflow-hidden">
  <!-- Header -->
  <header class="h-12 border-b border-gray-700 flex items-center justify-between px-4 bg-gray-800">
    <div class="font-bold text-lg flex items-center gap-2">
      <span class="text-blue-400">{$t('appTitle')}</span>
    </div>
    <div class="flex items-center gap-4">
      <button 
        aria-label="Settings"
        on:click={() => showSettings = true} 
        class="p-2 hover:bg-gray-700 rounded-full"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
      </button>
    </div>
  </header>

  <!-- Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar Controls -->
    <aside class="w-64 bg-gray-800 border-r border-gray-700 flex flex-col overflow-hidden">
      {#if imagePath}
        <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-6">
          <div class="flex flex-col gap-2">
            <label for="tools-label" class="text-xs font-semibold text-gray-400 uppercase">{$t('tools')}</label>
            <button id="tools-label" on:click={cropToSquare} class="bg-gray-700 hover:bg-gray-600 text-white text-sm py-1 rounded">
              {$t('cropSquare')}
            </button>
          </div>

          <div class="flex flex-col gap-2">
            <label for="grid-rows" class="text-xs font-semibold text-gray-400 uppercase">{$t('gridLayout')}</label>
            <div class="flex gap-2 items-center">
              <span class="w-8 text-sm">{$t('rows')}</span>
              <input id="grid-rows" type="range" min="1" max="8" bind:value={rows} class="flex-1 accent-blue-500">
              <span class="w-4 text-sm text-right">{rows}</span>
            </div>
            <div class="flex gap-2 items-center">
              <span class="w-8 text-sm">{$t('cols')}</span>
              <input id="grid-cols" type="range" min="1" max="8" bind:value={cols} class="flex-1 accent-blue-500">
              <span class="w-4 text-sm text-right">{cols}</span>
            </div>
          </div>

          <div class="flex flex-col gap-2">
            <label for="overlap-slider" class="text-xs font-semibold text-gray-400 uppercase">{$t('overlap')} ({Math.round(overlap*100)}%)</label>
            <input id="overlap-slider" type="range" min="0" max="0.5" step="0.05" bind:value={overlap} class="accent-blue-500">
          </div>
          
          <div class="flex flex-col gap-2">
            <label for="tile-res-select" class="text-xs font-semibold text-gray-400 uppercase">{$t('tileRes')}</label>
            <select id="tile-res-select" bind:value={tileRes} class="bg-gray-700 border border-gray-600 rounded p-1 text-sm">
              <option value={512}>512 x 512</option>
              <option value={1024}>1024 x 1024</option>
              <option value={2048}>2048 x 2048</option>
            </select>
          </div>

          <hr class="border-gray-700">

          <button 
            on:click={() => isProcessing = true}
            class="bg-blue-600 hover:bg-blue-500 text-white py-2 rounded font-medium disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={isProcessing}
          >
            {isProcessing ? $t('processing') : $t('processAll')}
          </button>
        </div>
        
        <div class="h-48 border-t border-gray-700 bg-gray-900/50 p-4 flex flex-col gap-2 overflow-y-auto text-xs">
          <label class="font-semibold text-gray-400 uppercase sticky top-0 bg-inherit py-1">Log</label>
          {#each logs as log}
            <div class={log.type === 'error' ? 'text-red-400' : 'text-green-400'}>
              <span class="text-gray-500">[{log.time}]</span> {log.message}
            </div>
          {/each}
        </div>
      {/if}
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
          bind:isProcessing
          on:log={handleLog} 
        />
      {/if}
    </section>
  </div>

  {#if showSettings}
    <Settings on:close={() => showSettings = false} />
  {/if}
</main>
