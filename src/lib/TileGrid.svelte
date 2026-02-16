<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { generateImage } from './api';

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

  async function processAll() {
    try {
      // 1. Split
      const splitRes: any = await invoke('split_img', {
        path: src,
        rows,
        cols,
        overlapRatio: overlap
      });
      
      tempDir = splitRes.temp_dir;
      const tileFiles = splitRes.tiles;
      
      tiles = tiles.map(t => ({ ...t, status: 'processing' }));
      
      // 2. Process each tile with API
      const apiKey = localStorage.getItem('gemini_api_key');
      const model = localStorage.getItem('gemini_model') || 'gemini-3-pro-image-preview'; // Default as requested
      const prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
      
      if (!apiKey) {
        throw new Error("API Key not found. Please set it in Settings.");
      }

      await Promise.all(tileFiles.map(async (tile: any, index: number) => {
         try {
           // Read tile
           const b64Data = await invoke('load_image', { path: tile.path }) as string;
           
           // Convert to Blob for API (helper in api.ts? Or manually here)
           // generateImage expects Blob.
           // Helper:
           const res = await fetch(b64Data);
           const blob = await res.blob();
           
           // Call API
           // We need to handle potential rate limits. Sequential or parallel?
           // Parallel is faster but might hit limits.
           // Let's do parallel for now.
           
           const resultBlob = await generateImage(blob, prompt, model, apiKey);
           
           // Convert result Blob to Base64 to save
           const reader = new FileReader();
           reader.readAsDataURL(resultBlob); 
           const resultB64 = await new Promise<string>(resolve => {
             reader.onloadend = () => resolve(reader.result as string);
           });
           
           // Save back to tile path (overwrite)
           await invoke('save_image', { path: tile.path, base64Data: resultB64 });
           
           // Update status
           tiles[index].status = 'done';
           // Trigger reactivity?
           tiles = [...tiles];
           
         } catch (err) {
           console.error(`Error processing tile ${tile.r},${tile.c}`, err);
           tiles[index].status = 'error';
           tiles = [...tiles];
           // Don't throw to allow partial success? Or throw?
           // For now log.
         }
      }));
      
      // 3. Merge
      const updatePayload = tileFiles.map((t: any) => ({
        r: t.r,
        c: t.c,
        path: t.path
      }));
      
      const mergedB64: string = await invoke('merge_img', {
        tiles: updatePayload,
        originalW: splitRes.original_width,
        originalH: splitRes.original_height,
        overlapRatio: overlap
      });
      
      resultSrc = mergedB64;
      
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
  
  function regenerateTile(tile: any) {
    alert(`Regenerating tile ${tile.r},${tile.c} (Not implemented fully yet, use Process All)`);
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
               fill="none" stroke={tile.status === 'processing' ? 'yellow' : tile.status === 'done' ? 'green' : 'rgba(255, 255, 255, 0.5)'} stroke-width="2"
             />
           {/each}
        </svg>
      {/if}
      
      <!-- Interactive Layer -->
      {#if tiles.length > 0}
         <div class="absolute inset-0">
           {#each tiles as tile}
             <!-- svelte-ignore a11y-click-events-have-key-events -->
             <div 
               role="button"
               tabindex="0"
               class="absolute hover:bg-blue-500 hover:bg-opacity-20 cursor-pointer group transition-colors border border-transparent hover:border-blue-400"
               style="left: {tile.x / originalW * 100}%; top: {tile.y / originalH * 100}%; width: {tile.w / originalW * 100}%; height: {tile.h / originalH * 100}%;"
               on:click={() => regenerateTile(tile)}
             >
                <div class="hidden group-hover:flex absolute top-2 right-2 bg-blue-600 text-white text-xs px-2 py-1 rounded shadow">
                  Regenerate
                </div>
             </div>
           {/each}
         </div>
      {/if}
    </div>
  {/if}
</div>
