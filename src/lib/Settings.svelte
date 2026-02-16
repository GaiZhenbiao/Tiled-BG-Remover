<script>
  import { createEventDispatcher, onMount } from 'svelte';
  
  const dispatch = createEventDispatcher();
  
  let apiKey = localStorage.getItem('gemini_api_key') || '';
  let modelName = localStorage.getItem('gemini_model') || 'gemini-1.5-pro';
  let prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
  
  function save() {
    localStorage.setItem('gemini_api_key', apiKey);
    localStorage.setItem('gemini_model', modelName);
    localStorage.setItem('gemini_prompt', prompt);
    dispatch('close');
  }
</script>

<div class="fixed inset-0 bg-black bg-opacity-70 flex items-center justify-center z-50">
  <div class="bg-gray-800 p-6 rounded-lg shadow-xl w-96 border border-gray-700">
    <h2 class="text-xl font-bold mb-4">Settings</h2>
    
    <div class="mb-4">
      <label class="block text-sm font-medium mb-1">Google AI API Key</label>
      <input type="password" bind:value={apiKey} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white" placeholder="AIzaSy..." />
    </div>

    <div class="mb-4">
      <label class="block text-sm font-medium mb-1">Model Name</label>
      <input type="text" bind:value={modelName} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white" placeholder="gemini-1.5-pro" />
      <p class="text-xs text-gray-400 mt-1">Experimental: try 'gemini-2.0-flash-exp' or 'gemini-3-pro-image-preview' if available.</p>
    </div>

    <div class="mb-4">
      <label class="block text-sm font-medium mb-1">System Prompt</label>
      <textarea bind:value={prompt} rows="3" class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white"></textarea>
    </div>

    <div class="flex justify-end gap-2 mt-6">
      <button on:click={() => dispatch('close')} class="bg-gray-700 hover:bg-gray-600 px-4 py-2 rounded">Cancel</button>
      <button on:click={save} class="bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded">Save</button>
    </div>
  </div>
</div>
