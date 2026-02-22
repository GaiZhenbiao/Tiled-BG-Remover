<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { t, locale } from '../lib/i18n';
  
  const dispatch = createEventDispatcher();
  
  export let concurrency = 2;
  export let theme = 'auto';

  const DEFAULT_PROMPT_TEMPLATE_WITH_REFERENCE = `Task: Generate one tile from a larger image.
Main subject: {subject}
Preserve the main subject exactly as-is. Do not change subject shape, geometry, pose, colors, materials, logos, or text.
Background rule: {background_instruction}
Background must be a single flat color only, with clean edges and absolutely no shadows, gradients, reflections, glow, or texture.
Tile position: row {tile_row}/{tile_rows}, column {tile_col}/{tile_cols}.
Use the full-image reference to keep composition, subject scale, and global consistency across neighboring tiles.
Return only the generated tile image.`;

  const DEFAULT_PROMPT_TEMPLATE_WITHOUT_REFERENCE = `Task: Generate one tile from a larger image.
Main subject: {subject}
Preserve the main subject exactly as-is. Do not change subject shape, geometry, pose, colors, materials, logos, or text.
Background rule: {background_instruction}
Background must be a single flat color only, with clean edges and absolutely no shadows, gradients, reflections, glow, or texture.
Return only the generated tile image.`;
  
  let apiKey = localStorage.getItem('gemini_api_key') || '';
  let apiUrl = localStorage.getItem('gemini_api_url') || 'https://generativelanguage.googleapis.com';
  let modelName = localStorage.getItem('gemini_model') || 'gemini-2.5-flash-image';
  const legacyPromptTemplate =
    localStorage.getItem('gemini_prompt_template') ||
    localStorage.getItem('gemini_prompt') ||
    '';
  let promptTemplateWithReference =
    localStorage.getItem('gemini_prompt_template_with_reference') ||
    legacyPromptTemplate ||
    DEFAULT_PROMPT_TEMPLATE_WITH_REFERENCE;
  let promptTemplateWithoutReference =
    localStorage.getItem('gemini_prompt_template_without_reference') ||
    legacyPromptTemplate ||
    DEFAULT_PROMPT_TEMPLATE_WITHOUT_REFERENCE;
  let operationMode = localStorage.getItem('gemini_operation_mode') || 'default';
  let verboseLogging = localStorage.getItem('verbose_logging') === 'true';
  let useFullImageReference = localStorage.getItem('use_full_image_reference') === 'true';
  let bgRemovalSetting = localStorage.getItem('bg_removal_enabled') === 'true';
  let keyColorSetting = localStorage.getItem('key_color') || 'green';
  let toleranceSetting = parseInt(localStorage.getItem('key_tolerance') || '10');
  let showApiKey = false;
  const appVersion = __APP_VERSION__;

  function restorePromptTemplateWithReference() {
    promptTemplateWithReference = DEFAULT_PROMPT_TEMPLATE_WITH_REFERENCE;
  }

  function restorePromptTemplateWithoutReference() {
    promptTemplateWithoutReference = DEFAULT_PROMPT_TEMPLATE_WITHOUT_REFERENCE;
  }
  
  function save() {
    localStorage.setItem('gemini_api_key', apiKey.trim());
    localStorage.setItem('gemini_api_url', apiUrl.trim());
    localStorage.setItem('gemini_model', modelName);
    localStorage.setItem('gemini_prompt_template_with_reference', promptTemplateWithReference);
    localStorage.setItem('gemini_prompt_template_without_reference', promptTemplateWithoutReference);
    const activeTemplate = useFullImageReference ? promptTemplateWithReference : promptTemplateWithoutReference;
    localStorage.setItem('gemini_prompt_template', activeTemplate);
    // Backward compatibility for any older reads.
    localStorage.setItem('gemini_prompt', activeTemplate);
    localStorage.setItem('gemini_operation_mode', operationMode);
    localStorage.setItem('verbose_logging', verboseLogging.toString());
    localStorage.setItem('use_full_image_reference', useFullImageReference.toString());
    localStorage.setItem('bg_removal_enabled', bgRemovalSetting.toString());
    localStorage.setItem('key_color', keyColorSetting);
    localStorage.setItem('key_tolerance', String(toleranceSetting));
    localStorage.setItem('concurrency', concurrency.toString());
    dispatch('close');
  }

  function clearApiKey() {
    apiKey = '';
  }

  function setToleranceSetting(e: Event) {
    const target = e.target as HTMLInputElement;
    const v = parseInt(target.value);
    toleranceSetting = Number.isNaN(v) ? 10 : Math.min(100, Math.max(0, v));
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 backdrop-blur-sm" on:click={() => dispatch('close')}>
  <div class="bg-white dark:bg-gray-800 p-6 rounded-lg shadow-2xl w-96 border border-gray-200 dark:border-gray-700 max-h-[90vh] flex flex-col transition-colors" on:click|stopPropagation>
    <h2 class="text-xl font-bold mb-4 text-gray-900 dark:text-white">{$t('settings.title')}</h2>
    
    <div class="flex-1 overflow-y-auto pr-2 flex flex-col gap-4">
      <div>
        <label for="language" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.language')}</label>
        <select id="language" bind:value={$locale} class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors">
          <option value="en">English</option>
          <option value="zh">中文</option>
          <option value="ja">日本語</option>
        </select>
      </div>

      <div>
        <label for="theme" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.appearance')}</label>
        <select id="theme" bind:value={theme} class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors">
          <option value="auto">{$t('settings.themeAuto')}</option>
          <option value="light">{$t('settings.themeLight')}</option>
          <option value="dark">{$t('settings.themeDark')}</option>
        </select>
      </div>
      
      <div>
        <label for="operation-mode" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.operationMode')}</label>
        <select id="operation-mode" bind:value={operationMode} class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors">
          <option value="default">{$t('settings.operationModeDefault')}</option>
          <option value="mock">{$t('settings.operationModeMock')}</option>
          <option value="test_t2i">{$t('settings.operationModeTestT2i')}</option>
        </select>
      </div>

      <label class="flex items-center justify-between gap-3 rounded border border-gray-200 dark:border-gray-700 p-2">
        <span class="text-sm text-gray-700 dark:text-gray-300">{$t('settings.verboseLogging')}</span>
        <input type="checkbox" bind:checked={verboseLogging} class="accent-blue-600">
      </label>

      <label class="flex items-center justify-between gap-3 rounded border border-gray-200 dark:border-gray-700 p-2">
        <span class="text-sm text-gray-700 dark:text-gray-300">{$t('settings.fullImageReference')}</span>
        <input type="checkbox" bind:checked={useFullImageReference} class="accent-blue-600">
      </label>

      <label class="flex items-center justify-between gap-3 rounded border border-gray-200 dark:border-gray-700 p-2">
        <span class="text-sm text-gray-700 dark:text-gray-300">{$t('bgRemoval')}</span>
        <input type="checkbox" bind:checked={bgRemovalSetting} class="accent-blue-600">
      </label>

      {#if bgRemovalSetting}
        <div class="rounded border border-gray-200 dark:border-gray-700 p-3 flex flex-col gap-2">
          <span class="text-sm text-gray-700 dark:text-gray-300">{$t('keyColor')}</span>
          <div class="flex gap-2">
            {#each ['green', 'red', 'blue', 'black', 'white'] as color}
              <button
                type="button"
                on:click={() => (keyColorSetting = color)}
                class="w-6 h-6 rounded-full border border-gray-300 dark:border-gray-600 {keyColorSetting === color ? 'ring-2 ring-blue-500 dark:ring-blue-400' : ''}"
                style="background-color: {color === 'white' ? '#fff' : color === 'black' ? '#000' : color}"
                title={color}
              ></button>
            {/each}
          </div>
          <div class="flex justify-between items-center mt-1">
            <span class="text-xs text-gray-500 dark:text-gray-400">{$t('tolerance')} ({toleranceSetting})</span>
            <input
              type="range"
              min="0"
              max="100"
              value={toleranceSetting}
              on:input={setToleranceSetting}
              class="w-36 accent-blue-600"
            >
          </div>
        </div>
      {/if}
      
      <div>
        <label for="api-key" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.apiKey')}</label>
        <div class="flex items-center gap-2">
          <input
            id="api-key"
            type={showApiKey ? 'text' : 'password'}
            bind:value={apiKey}
            class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors"
            placeholder="AIzaSy..."
          />
          <button
            type="button"
            class="shrink-0 px-2 py-2 text-xs rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            on:click={() => (showApiKey = !showApiKey)}
          >
            {showApiKey ? $t('settings.hide') : $t('settings.show')}
          </button>
          <button
            type="button"
            class="shrink-0 px-2 py-2 text-xs rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            on:click={clearApiKey}
            disabled={!apiKey}
          >
            {$t('settings.clear')}
          </button>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">{$t('settings.apiKeyHint')}</p>
      </div>

      <div>
        <label for="api-url" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.apiUrl')}</label>
        <input id="api-url" type="text" bind:value={apiUrl} class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors" placeholder="https://generativelanguage.googleapis.com" />
      </div>

      <div>
        <label for="model-name" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('settings.modelName')}</label>
        <select id="model-name" bind:value={modelName} class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors">
          <option value="gemini-2.5-flash-image">gemini-2.5-flash-image</option>
          <option value="gemini-3-pro-image-preview">gemini-3-pro-image-preview</option>
        </select>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">{$t('settings.experimental')}</p>
      </div>

      <div>
        <label for="concurrency" class="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">{$t('concurrency')} ({concurrency})</label>
        <div class="flex items-center gap-2">
          <input id="concurrency" type="range" min="1" max="8" step="1" bind:value={concurrency} class="flex-1 accent-blue-600">
        </div>
      </div>

      <div>
        <div class="mb-1 flex items-center justify-between gap-2">
          <label for="system-prompt-with-ref" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{$t('settings.promptTemplateWithReference')}</label>
          <button
            type="button"
            on:click={restorePromptTemplateWithReference}
            class="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            {$t('settings.restoreDefault')}
          </button>
        </div>
        <textarea id="system-prompt-with-ref" bind:value={promptTemplateWithReference} rows="7" class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors"></textarea>
      </div>

      <div>
        <div class="mb-1 flex items-center justify-between gap-2">
          <label for="system-prompt-no-ref" class="block text-sm font-medium text-gray-700 dark:text-gray-300">{$t('settings.promptTemplateWithoutReference')}</label>
          <button
            type="button"
            on:click={restorePromptTemplateWithoutReference}
            class="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          >
            {$t('settings.restoreDefault')}
          </button>
        </div>
        <textarea id="system-prompt-no-ref" bind:value={promptTemplateWithoutReference} rows="7" class="w-full bg-gray-50 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded p-2 text-gray-900 dark:text-white transition-colors"></textarea>
        <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
          {$t('settings.placeholdersLabel')}: <code>{'{subject}'}</code>, <code>{'{background_instruction}'}</code>,
          <code>{'{key_color}'}</code>, <code>{'{tile_row}'}</code>, <code>{'{tile_col}'}</code>,
          <code>{'{tile_rows}'}</code>, <code>{'{tile_cols}'}</code>, <code>{'{tile_width}'}</code>,
          <code>{'{tile_height}'}</code>, <code>{'{image_width}'}</code>, <code>{'{image_height}'}</code>.
        </p>
      </div>
    </div>

    <div class="flex justify-end gap-2 mt-6 pt-4 border-t border-gray-200 dark:border-gray-700 transition-colors">
      <button on:click={() => dispatch('close')} class="bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-300 px-4 py-2 rounded transition-colors">{$t('settings.cancel')}</button>
      <button on:click={save} class="bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded shadow-lg transition-colors">{$t('settings.save')}</button>
    </div>
    <div class="mt-2 text-center text-xs text-gray-500 dark:text-gray-400">
      {$t('settings.version')}: v{appVersion}
    </div>
  </div>
</div>
