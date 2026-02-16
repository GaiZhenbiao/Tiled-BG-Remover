<script>
  import { createEventDispatcher, onMount } from 'svelte';
  import { t, locale } from '../lib/i18n';
  
  const dispatch = createEventDispatcher();
  
  let apiKey = localStorage.getItem('gemini_api_key') || '';
  let modelName = localStorage.getItem('gemini_model') || 'gemini-1.5-pro';
  let prompt = localStorage.getItem('gemini_prompt') || 'Remove the background to be pure white. No any shadows. The foreground is part of a bicycle.';
  let useTestMode = localStorage.getItem('gemini_test_mode') === 'true';
  
  function save() {
    localStorage.setItem('gemini_api_key', apiKey);
    localStorage.setItem('gemini_model', modelName);
    localStorage.setItem('gemini_prompt', prompt);
    localStorage.setItem('gemini_test_mode', String(useTestMode));
    dispatch('close');
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="fixed inset-0 bg-black bg-opacity-70 flex items-center justify-center z-50" on:click={() => dispatch('close')}>
  <div class="bg-gray-800 p-6 rounded-lg shadow-xl w-96 border border-gray-700" on:click|stopPropagation>
    <h2 class="text-xl font-bold mb-4">{$t('settings.title')}</h2>
    
    <div class="mb-4">
      <label for="language" class="block text-sm font-medium mb-1">Language</label>
      <select id="language" bind:value={$locale} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white">
        <option value="en">English</option>
        <option value="zh">中文</option>
        <option value="ja">日本語</option>
      </select>
    </div>
    
    <div class="mb-4 flex items-center gap-2">
      <input id="test-mode" type="checkbox" bind:checked={useTestMode} class="w-4 h-4 rounded bg-gray-700 border-gray-600 text-blue-600 focus:ring-blue-500">
      <label for="test-mode" class="text-sm font-medium">{$t('settings.testMode')}</label>
    </div>
    
    <div class="mb-4">
      <label for="api-key" class="block text-sm font-medium mb-1">{$t('settings.apiKey')}</label>
      <input id="api-key" type="password" bind:value={apiKey} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white" placeholder="AIzaSy..." disabled={useTestMode} />
    </div>

    <div class="mb-4">
      <label for="model-name" class="block text-sm font-medium mb-1">{$t('settings.modelName')}</label>
      <select id="model-name" bind:value={modelName} class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white">
        <option value="gemini-2.0-flash-exp">gemini-2.0-flash-exp</option>
        <option value="gemini-2.5-flash-image">gemini-2.5-flash-image</option>
        <option value="gemini-3-pro-image-preview">gemini-3-pro-image-preview</option>
      </select>
      <p class="text-xs text-gray-400 mt-1">{$t('settings.experimental')}</p>
    </div>

    <div class="mb-4">
      <label for="system-prompt" class="block text-sm font-medium mb-1">{$t('settings.systemPrompt')}</label>
      <textarea id="system-prompt" bind:value={prompt} rows="3" class="w-full bg-gray-700 border border-gray-600 rounded p-2 text-white"></textarea>
    </div>

    <div class="flex justify-end gap-2 mt-6">
      <button on:click={() => dispatch('close')} class="bg-gray-700 hover:bg-gray-600 px-4 py-2 rounded">{$t('settings.cancel')}</button>
      <button on:click={save} class="bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded">{$t('settings.save')}</button>
    </div>
  </div>
</div>