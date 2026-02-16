<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  const dispatch = createEventDispatcher();

  export let src: string;
  export let initialAspectRatio: number | null = 1;

  let container: HTMLDivElement;
  let imgElement: HTMLImageElement;
  let canvas: HTMLCanvasElement;
  
  let imgW = 0;
  let imgH = 0;
  let displayW = 0;
  let displayH = 0;
  let scale = 1;
  let loadedSrc = '';

  onMount(async () => {
    try {
      loadedSrc = await invoke('load_image', { path: src });
    } catch (e) {
      console.error(e);
    }
  });

  // Crop area in image coordinates
  let cropX = 0;
  let cropY = 0;
  let cropW = 0;
  let cropH = 0;

  let aspectRatio = initialAspectRatio;
  let isDragging = false;
  let dragType = ''; // 'move', 'nw', 'ne', 'sw', 'se'
  let startX = 0;
  let startY = 0;
  let startCropX = 0;
  let startCropY = 0;
  let startCropW = 0;
  let startCropH = 0;

  $: if (aspectRatio && imgW && imgH) {
    updateCropForAspectRatio();
  }

  function updateCropForAspectRatio() {
    if (aspectRatio === null) return; // Free crop - don't reset
    
    // Maintain center if possible, or just reset
    if (imgW / imgH > aspectRatio) {
      // Image is wider than crop
      cropH = imgH;
      cropW = imgH * aspectRatio;
      cropX = (imgW - cropW) / 2;
      cropY = 0;
    } else {
      // Image is taller than crop
      cropW = imgW;
      cropH = imgW / aspectRatio;
      cropX = 0;
      cropY = (imgH - cropH) / 2;
    }
  }

  function handleImageLoad() {
    imgW = imgElement.naturalWidth;
    imgH = imgElement.naturalHeight;
    displayW = imgElement.clientWidth;
    displayH = imgElement.clientHeight;
    scale = displayW / imgW;
    updateCropForAspectRatio();
  }

  function onMouseDown(e: MouseEvent, type: string) {
    isDragging = true;
    dragType = type;
    startX = e.clientX;
    startY = e.clientY;
    startCropX = cropX;
    startCropY = cropY;
    startCropW = cropW;
    startCropH = cropH;
    e.preventDefault();
  }

  function onMouseMove(e: MouseEvent) {
    if (!isDragging) return;
    
    const dx = (e.clientX - startX) / scale;
    const dy = (e.clientY - startY) / scale;
    
    if (dragType === 'move') {
      cropX = Math.max(0, Math.min(imgW - cropW, startCropX + dx));
      cropY = Math.max(0, Math.min(imgH - cropH, startCropY + dy));
    } else {
        let newX = startCropX;
        let newY = startCropY;
        let newW = startCropW;
        let newH = startCropH;

        if (dragType.includes('e')) {
            newW = Math.max(20, startCropW + dx);
            if (newX + newW > imgW) newW = imgW - newX;
        }
        if (dragType.includes('s')) {
            newH = Math.max(20, startCropH + dy);
            if (newY + newH > imgH) newH = imgH - newY;
        }
        if (dragType.includes('w')) {
            newW = Math.max(20, startCropW - dx);
            newX = startCropX + startCropW - newW;
            if (newX < 0) {
                newX = 0;
                newW = startCropX + startCropW;
            }
        }
        if (dragType.includes('n')) {
            newH = Math.max(20, startCropH - dy);
            newY = startCropY + startCropH - newH;
            if (newY < 0) {
                newY = 0;
                newH = startCropY + startCropH;
            }
        }

        if (aspectRatio) {
            // Constrain to aspect ratio
            if (dragType === 'se') {
                newH = newW / aspectRatio;
                if (newY + newH > imgH) {
                    newH = imgH - newY;
                    newW = newH * aspectRatio;
                }
            } else if (dragType === 'sw') {
                newH = newW / aspectRatio;
                if (newY + newH > imgH) {
                    newH = imgH - newY;
                    newW = newH * aspectRatio;
                    newX = startCropX + startCropW - newW;
                }
            } else if (dragType === 'ne') {
                newH = newW / aspectRatio;
                newY = startCropY + startCropH - newH;
                if (newY < 0) {
                    newY = 0;
                    newH = startCropY + startCropH;
                    newW = newH * aspectRatio;
                }
            } else if (dragType === 'nw') {
                newH = newW / aspectRatio;
                newY = startCropY + startCropH - newH;
                if (newY < 0) {
                     newY = 0;
                     newH = startCropY + startCropH;
                     newW = newH * aspectRatio;
                     newX = startCropX + startCropW - newW;
                }
            }
        }
        
        cropX = newX;
        cropY = newY;
        cropW = newW;
        cropH = newH;
    }
  }

  function onMouseUp() {
    isDragging = false;
  }

  function done() {
    dispatch('done', {
      x: Math.round(cropX),
      y: Math.round(cropY),
      width: Math.round(cropW),
      height: Math.round(cropH)
    });
  }

  const aspectRatios = [
    { label: 'Free', value: null },
    { label: '1:1', value: 1 },
    { label: '4:3', value: 4/3 },
    { label: '16:9', value: 16/9 },
    { label: '3:2', value: 3/2 },
    { label: '3:4', value: 3/4 },
    { label: '9:16', value: 9/16 },
  ];
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="fixed inset-0 bg-black/90 dark:bg-black/95 z-[100] flex flex-col items-center justify-center p-8 transition-colors" on:mousemove={onMouseMove} on:mouseup={onMouseUp}>
  <div class="flex-1 relative w-full flex items-center justify-center overflow-hidden">
    <div class="relative inline-block" bind:this={container}>
      <img 
        src={loadedSrc} 
        bind:this={imgElement} 
        on:load={handleImageLoad}
        class="max-h-[70vh] max-w-full select-none shadow-2xl"
        alt="To crop"
        draggable="false"
      />
      
      {#if imgW}
        <!-- Dimmed overlay -->
        <div class="absolute inset-0 bg-black/50 pointer-events-none"></div>
        
        <!-- Crop area -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div 
          class="absolute border border-white cursor-move shadow-[0_0_0_9999px_rgba(0,0,0,0.5)]"
          style="left: {cropX * scale}px; top: {cropY * scale}px; width: {cropW * scale}px; height: {cropH * scale}px;"
          on:mousedown={(e) => onMouseDown(e, 'move')}
        >
          <!-- Size Display -->
          <div class="absolute -top-6 left-1/2 transform -translate-x-1/2 bg-black/70 text-white text-[10px] px-2 py-0.5 rounded pointer-events-none whitespace-nowrap font-mono">
            {Math.round(cropW)} x {Math.round(cropH)}
          </div>

          <!-- Handles -->
          <div 
            class="absolute -left-1.5 -top-1.5 w-3 h-3 bg-white border border-blue-600 cursor-nw-resize rounded-full shadow-sm"
            on:mousedown|stopPropagation={(e) => onMouseDown(e, 'nw')}
          ></div>
          <div 
            class="absolute -right-1.5 -top-1.5 w-3 h-3 bg-white border border-blue-600 cursor-ne-resize rounded-full shadow-sm"
            on:mousedown|stopPropagation={(e) => onMouseDown(e, 'ne')}
          ></div>
          <div 
            class="absolute -left-1.5 -bottom-1.5 w-3 h-3 bg-white border border-blue-600 cursor-sw-resize rounded-full shadow-sm"
            on:mousedown|stopPropagation={(e) => onMouseDown(e, 'sw')}
          ></div>
          <div 
            class="absolute -right-1.5 -bottom-1.5 w-3 h-3 bg-white border border-blue-600 cursor-se-resize rounded-full shadow-sm"
            on:mousedown|stopPropagation={(e) => onMouseDown(e, 'se')}
          ></div>
        </div>
      {/if}
    </div>
  </div>

  
  <div class="mt-8 flex flex-col items-center gap-4 w-full max-w-2xl">
    <div class="flex flex-wrap justify-center gap-2">
      {#each aspectRatios as ar}
        <button 
          on:click={() => aspectRatio = ar.value}
          class="px-4 py-1.5 rounded-full text-sm font-medium transition-all {aspectRatio === ar.value ? 'bg-blue-600 text-white shadow-lg scale-105' : 'bg-gray-800 hover:bg-gray-700 text-gray-300 border border-gray-700'}"
        >
          {ar.label}
        </button>
      {/each}
    </div>
    
    <div class="flex gap-4">
      <button on:click={() => dispatch('cancel')} class="px-8 py-2.5 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded-lg font-bold border border-gray-700 transition-colors">Cancel</button>
      <button on:click={done} class="px-8 py-2.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-bold shadow-lg transition-all active:scale-95">Done</button>
    </div>
  </div>
</div>
