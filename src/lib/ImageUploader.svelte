<script>
  import { createEventDispatcher } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';

  const dispatch = createEventDispatcher();

  async function selectFile() {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Image',
        extensions: ['png', 'jpg', 'jpeg', 'webp']
      }]
    });
    if (selected) {
      if (Array.isArray(selected)) {
        dispatch('selected', selected[0]);
      } else {
        dispatch('selected', selected);
      }
    }
  }

  function handleDrop(e) {
    e.preventDefault();
  }

  // Listen for file drop
  listen('tauri://file-drop', event => {
    const payload = event.payload;
    if (Array.isArray(payload) && payload.length > 0) {
      dispatch('selected', payload[0]);
    }
  });

</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<!-- svelte-ignore a11y-click-events-have-key-events -->
<div 
  role="button"
  tabindex="0"
  class="border-2 border-dashed border-gray-600 rounded-xl p-12 flex flex-col items-center justify-center text-gray-400 hover:border-blue-500 hover:text-blue-500 transition cursor-pointer"
  on:click={selectFile}
  on:dragover|preventDefault
  on:drop={handleDrop}
>
  <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" class="mb-4"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
  <p class="text-lg">Click or Drop Image Here</p>
</div>