<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
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
  export let resizeInputToOutput: boolean;
  export let bgRemovalEnabled: boolean;
  export let keyColor: string;
  export let tolerance: number = 10;
  export let resultSrc: string = '';

  let container: HTMLDivElement;
  let imgElement: HTMLImageElement;
  let displaySrc = '';
  let isSplitting = false;
  let isMerging = false;
  
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
  
  $: if (src) {
    loadImage(src);
    resultSrc = ''; // Reset result when source changes
  }
  
  $: if ((bgRemovalEnabled !== prevBG || keyColor !== prevKey || tolerance !== prevTol) && tiles.length > 0 && tiles.some(t => t.status === 'done') && !isProcessing && !isMerging) {
      prevBG = bgRemovalEnabled;
      prevKey = keyColor;
      prevTol = tolerance;
      mergeAll();
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
            const model = localStorage.getItem('gemini_model') || 'gemini-1.5-pro'; 
            
            if (!apiKey) throw new Error("API Key not found. Please set it in Settings.");

            let prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
            
            if (bgRemovalEnabled) {
              prompt = `Remove the background and replace it with pure ${keyColor}. The background must be a solid, flat ${keyColor} color with no shadows, gradients or textures. The foreground object is part of a bicycle. Focus on clean edges.`;
            }

            let inputBlob: Blob | null = null;

            if (operationMode === 'test_t2i') {
                prompt = `Generate a beautiful scenery with a big, black text saying '(${tile.r},${tile.c})' in the center.`;
            } else {
                // Read via Rust to bypass scope restrictions
                const b64Data = await invoke('load_image', { path: tile.originalPath }) as string;
                const res = await fetch(b64Data);
                inputBlob = await res.blob();
            }
            
            // API Call
            resultBlob = await generateImage(inputBlob, prompt, model, apiKey);
        }
        
        // Save result
        const reader = new FileReader();
        reader.readAsDataURL(resultBlob); 
        const resultB64 = await new Promise<string>(resolve => {
             reader.onloadend = () => resolve(reader.result as string);
        });
        
        // Use resize if requested
        if (resizeInputToOutput) {
            await invoke('save_image_resized', { path: tile.path, base64Data: resultB64, width: Math.round(tile.w), height: Math.round(tile.h) });
        } else {
            await invoke('save_image', { path: tile.path, base64Data: resultB64 });
        }
        
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
        
        resultSrc = mergedB64;
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
        let i = 0;
        for (const resTile of splitRes.tiles) {
            if (tiles[i]) {
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
      const CONCURRENCY = 2;
      const queue = [...tiles.keys()];
      
      const workers = Array(CONCURRENCY).fill(null).map(async () => {
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
    <div class="relative inline-block shadow-2xl {bgRemovalEnabled ? 'checkerboard' : ''}">
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
               stroke={tile.status === 'processing' ? '#fbbf24' : tile.status === 'done' ? (resultSrc ? 'rgba(74, 222, 128, 0.3)' : '#4ade80') : tile.status === 'error' ? '#ef4444' : 'rgba(255, 255, 255, 0.5)'} 
               stroke-width={resultSrc ? "1" : "2"}
             />
           {/each}
        </svg>
      {/if}
      
      <!-- Interactive Layer -->
      {#if tiles.length > 0}
         <div class="absolute inset-0">
           {#if isSplitting || isMerging}
             <div class="absolute inset-0 bg-black/50 flex flex-col items-center justify-center z-10 backdrop-blur-[1px]">
               <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-white mb-4"></div>
               <span class="text-white font-bold bg-black/50 px-3 py-1 rounded">
                 {isSplitting ? 'Splitting Image...' : 'Merging Tiles...'}
               </span>
             </div>
           {/if}
           
           {#each tiles as tile, index}
             <!-- svelte-ignore a11y-click-events-have-key-events -->
             <div 
               role="button"
               tabindex="0"
               class="absolute group transition-colors border border-transparent hover:border-blue-400 hover:bg-blue-500/20 flex items-center justify-center"
               style="left: {tile.x / originalW * 100}%; top: {tile.y / originalH * 100}%; width: {tile.w / originalW * 100}%; height: {tile.h / originalH * 100}%;"
               on:click={() => regenerateTile(index)}
             >
                {#if tile.status === 'processing'}
                  <div class="absolute inset-0 bg-black/50 flex items-center justify-center backdrop-blur-[1px]">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
                  </div>
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

<style>
  .checkerboard {
    background-image: linear-gradient(45deg, #333 25%, transparent 25%), 
                      linear-gradient(-45deg, #333 25%, transparent 25%), 
                      linear-gradient(45deg, transparent 75%, #333 75%), 
                      linear-gradient(-45deg, transparent 75%, #333 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
    background-color: #222;
  }
</style>