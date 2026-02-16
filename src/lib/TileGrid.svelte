<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { generateImage } from './api';
  import { t } from '../lib/i18n';

  const dispatch = createEventDispatcher();

  export let src: string;
  export let rows: number;
  export let cols: number;
  export let overlap: number;
  export let isProcessing: boolean;

  let container: HTMLDivElement;
  let imgElement: HTMLImageElement;
  let displaySrc = '';
  
  // Grid visualization state
  let tiles: any[] = [];
  
  // Result state
  let resultSrc: string | null = null;
  let tempDir = '';
  let originalW = 0;
  let originalH = 0;
  
  $: if (src) {
    loadImage(src);
    resultSrc = null; // Reset result when source changes
  }
  
  async function loadImage(path: string) {
    try {
      const b64 = await invoke('load_image', { path });
      displaySrc = b64 as string;
    } catch (e) {
      console.error("Failed to load image:", e);
    }
  }
  
  $: if (rows && cols && overlap && imgElement) {
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
          status: 'pending'
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
        const apiKey = localStorage.getItem('gemini_api_key');
        const model = localStorage.getItem('gemini_model') || 'gemini-3-pro-image-preview'; 
        const prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
        
        if (!apiKey) throw new Error("API Key not found. Please set it in Settings.");

        // Read tile
        const b64Data = await invoke('load_image', { path: tile.path }) as string;
        
        // Convert to Blob
        const res = await fetch(b64Data);
        const blob = await res.blob();
        
        // API Call
        const resultBlob = await generateImage(blob, prompt, model, apiKey);
        console.log(`API returned blob: ${resultBlob.size} bytes, type: ${resultBlob.type}`);
        
        // Save
        const reader = new FileReader();
        reader.readAsDataURL(resultBlob); 
        const resultB64 = await new Promise<string>(resolve => {
             reader.onloadend = () => resolve(reader.result as string);
        });
        
        await invoke('save_image', { path: tile.path, base64Data: resultB64 });
        console.log(`Saved updated tile to ${tile.path}`);
        
        tiles[index].status = 'done';
    } catch (e) {
        console.error(`Error processing tile ${tile.r},${tile.c}`, e);
        tiles[index].status = 'error';
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
      
      const mergedB64: string = await invoke('merge_img', {
        tiles: updatePayload,
        originalW: originalW,
        originalH: originalH,
        overlapRatio: overlap
      });
      
      console.log(`Merged result size: ${mergedB64.length}`);
      resultSrc = mergedB64;
  }

  async function splitImageAndAssignPaths() {
      const splitRes: any = await invoke('split_img', {
        path: src,
        rows,
        cols,
        overlapRatio: overlap
      });
      
      tempDir = splitRes.temp_dir;
      let i = 0;
      for (const resTile of splitRes.tiles) {
          if (tiles[i]) tiles[i].path = resTile.path;
          i++;
      }
      return splitRes;
  }

  async function processAll() {
    try {
      // 1. Split
      await splitImageAndAssignPaths();
      tiles = [...tiles]; // Trigger update
      
      // 2. Process
      await Promise.all(tiles.map((_, index) => processSingleTile(index)));
      
      // 3. Merge
      await mergeAll();
      
    } catch (e) {
      console.error(e);
      alert('Error processing: ' + e);
    } finally {
      isProcessing = false;
    }
  }
  
  export async function cropCenter() {
     if (!imgElement) return;
     const size = Math.min(imgElement.naturalWidth, imgElement.naturalHeight);
     const x = (imgElement.naturalWidth - size) / 2;
     const y = (imgElement.naturalHeight - size) / 2;
     
     const newPath: string = await invoke('crop_img', {
       path: src, x: Math.round(x), y: Math.round(y), width: Math.round(size), height: Math.round(size)
     });
     
     dispatch('crop', newPath);
  }

  function handleImageLoad() {
    calculateGrid();
  }
  
  async function regenerateTile(index: number) {
    if (!tiles[index].path) {
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
</script>

<div class="relative w-full h-full flex items-center justify-center overflow-auto" bind:this={container}>
  {#if displaySrc}
    <div class="relative inline-block shadow-2xl">
      <!-- Main Image -->
      <img 
        src={resultSrc || displaySrc} 
        bind:this={imgElement}
        on:load={handleImageLoad}
        class="max-w-none block"
        style="max-height: 80vh; object-fit: contain;"
        alt="Source"
      />
      
      <!-- Overlay Grid -->
      {#if tiles.length > 0}
        <svg class="absolute inset-0 pointer-events-none" viewBox={`0 0 ${originalW} ${originalH}`} preserveAspectRatio="none">
           {#each tiles as tile}
             <rect 
               x={tile.x} y={tile.y} width={tile.w} height={tile.h} 
               fill="none" 
               stroke={tile.status === 'processing' ? '#fbbf24' : tile.status === 'done' ? '#4ade80' : tile.status === 'error' ? '#ef4444' : 'rgba(255, 255, 255, 0.5)'} 
               stroke-width="2"
             />
           {/each}
        </svg>
      {/if}
      
      <!-- Interactive Layer -->
      {#if tiles.length > 0}
         <div class="absolute inset-0">
           {#each tiles as tile, index}
             <!-- svelte-ignore a11y-click-events-have-key-events -->
             <div 
               role="button"
               tabindex="0"
               class="absolute group transition-colors border border-transparent hover:border-blue-400 hover:bg-blue-500/20 flex items-center justify-center"
               style="left: {tile.x / originalW * 100}%; top: {tile.y / originalH * 100}%; width: {tile.w / originalW * 100}%; height: {tile.h / originalH * 100}%;"
             >
                {#if tile.status === 'processing'}
                  <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
                {:else}
                  <button 
                    on:click|stopPropagation={() => regenerateTile(index)}
                    class="hidden group-hover:block bg-blue-600 hover:bg-blue-500 text-white text-xs px-3 py-1.5 rounded shadow transform hover:scale-105 transition"
                  >
                    {tile.status === 'pending' ? $t('generate') : $t('regenerate')}
                  </button>
                {/if}
             </div>
           {/each}
         </div>
      {/if}
    </div>
  {/if}
</div>
