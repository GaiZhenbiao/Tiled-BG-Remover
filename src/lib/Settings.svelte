<script>
  import { createEventDispatcher, onMount } from 'svelte';
  import { t, locale } from '../lib/i18n';
  
  const dispatch = createEventDispatcher();
  
  export let concurrency = 2;
  
  let apiKey = localStorage.getItem('gemini_api_key') || '';
  let modelName = localStorage.getItem('gemini_model') || 'gemini-1.5-pro';
  let prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
  let operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
  
  function save() {
    localStorage.setItem('gemini_api_key', apiKey);
    localStorage.setItem('gemini_model', modelName);
    localStorage.setItem('gemini_prompt', prompt);
    localStorage.setItem('gemini_operation_mode', operationMode);
    localStorage.setItem('concurrency', concurrency.toString());
    dispatch('close');
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="fixed inset-0 bg-black bg-opacity-70 flex items-center justify-center z-50" on:click={() => dispatch('close')}>
  <div class="bg-gray-800 p-6 rounded-lg shadow-xl w-96 border border-gray-700 max-h-[90vh] flex flex-col" on:click|stopPropagation>
    <h2 class="text-xl font-bold mb-4">{$t('settings.title')}</h2>
    
    <div class="flex-1 overflow-y-auto pr-2 flex flex-col gap-4">
      <div>
        <label for="language" class="block text-sm font-medium mb-1">Language</label>
        <select id="language" bind:value={$locale} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white">
          <option value="en">English</option>
          <option value="zh">中文</option>
          <option value="ja">日本語</option>
        </select>
      </div>
      
      <div>
        <label for="operation-mode" class="block text-sm font-medium mb-1">Operation Mode</label>
        <select id="operation-mode" bind:value={operationMode} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white">
          <option value="default">Default (Image-to-Image)</option>
          <option value="mock">Mock (Local Noise)</option>
          <option value="test_t2i">Test (AI Text-to-Image)</option>
        </select>
      </div>
      
      <div>
        <label for="api-key" class="block text-sm font-medium mb-1">{$t('settings.apiKey')}</label>
        <input id="api-key" type="password" bind:value={apiKey} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white" placeholder="AIzaSy..." disabled={operationMode === 'mock'} />
      </div>

      <div>
        <label for="model-name" class="block text-sm font-medium mb-1">{$t('settings.modelName')}</label>
        <select id="model-name" bind:value={modelName} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white">
          <option value="gemini-2.5-flash-image">gemini-2.5-flash-image</option>
          <option value="gemini-3-pro-image-preview">gemini-3-pro-image-preview</option>
        </select>
        <p class="text-xs text-gray-400 mt-1">{$t('settings.experimental')}</p>
      </div>

      <div>
        <label for="concurrency" class="block text-sm font-medium mb-1">{$t('concurrency')} ({concurrency})</label>
        <div class="flex items-center gap-2">
          <input id="concurrency" type="range" min="1" max="8" step="1" bind:value={concurrency} class="flex-1 accent-blue-500">
        </div>
      </div>

      <div>
        <label for="system-prompt" class="block text-sm font-medium mb-1">{$t('settings.systemPrompt')}</label>
        <textarea id="system-prompt" bind:value={prompt} rows="3" class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white"></textarea>
      </div>
    </div>

    <div class="flex justify-end gap-2 mt-6 pt-4 border-t border-gray-700">
      <button on:click={() => dispatch('close')} class="bg-gray-700 hover:bg-gray-600 px-4 py-2 rounded">{$t('settings.cancel')}</button>
      <button on:click={save} class="bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded">{$t('settings.save')}</button>
    </div>
  </div>
</div>