import { writable, derived } from 'svelte/store';
import { browser } from '$app/environment';

const SUPPORTED_LOCALES = ['en', 'zh', 'ja'] as const;
type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];
const LOCALE_STORAGE_KEY = 'app_locale';
const FALLBACK_LOCALE: SupportedLocale = 'en';

function normalizeLocale(input: string | null | undefined): SupportedLocale {
  const value = (input || '').toLowerCase();
  if (value.startsWith('zh')) return 'zh';
  if (value.startsWith('ja')) return 'ja';
  if (value.startsWith('en')) return 'en';
  return FALLBACK_LOCALE;
}

function getInitialLocale(): SupportedLocale {
  if (!browser) return FALLBACK_LOCALE;

  const saved = localStorage.getItem(LOCALE_STORAGE_KEY);
  if (saved) return normalizeLocale(saved);

  const preferred = navigator.languages && navigator.languages.length > 0
    ? navigator.languages
    : [navigator.language];

  for (const lang of preferred) {
    const normalized = normalizeLocale(lang);
    if (SUPPORTED_LOCALES.includes(normalized)) {
      return normalized;
    }
  }

  return FALLBACK_LOCALE;
}

export const locale = writable<SupportedLocale>(getInitialLocale());

if (browser) {
  locale.subscribe((value) => {
    localStorage.setItem(LOCALE_STORAGE_KEY, value);
  });
}

const translations = {
  en: {
    appTitle: "Tiled BG Remover",
    tools: "Tools",
    cropImage: "Crop Image",
    centerCrop: "1:1 Center",
    gridLayout: "Grid Layout",
    smartGrid: "Smart",
    rows: "Rows",
    cols: "Cols",
    overlap: "Overlap",
    aiOutputRes: "AI Output Resolution",
    smartQuality: "Tile Size Tolerance (Lower Quality)",
    smartQualityHint: "Higher value = larger tiles, fewer tiles, lower quality.",
    tileCount: "Tiles",
    lowerQuality: "Lower Quality",
    higherQuality: "Higher Quality",
    showTileLines: "Show Tile Lines",
    resizeInput: "Resize input tile to match output",
    concurrency: "Max Concurrency",
    bgRemoval: "Background Removal",
    bgRemoved: "BGRemoved",
    bgRemovedSuffix: "BGRemoved",
    keyColor: "Key Color",
    tolerance: "Tolerance",
    mainSubject: "Main Subject",
    subjectForPrompt: "Subject For Prompt",
    subjectPlaceholder: "Enter subject (e.g. bicycle)",
    usingSubject: "Using",
    useDetectedSubject: "Use detected",
    detectingSubject: "Detecting subject...",
    noApiKeySubject: "Set API key to detect subject",
    subjectDetectFailed: "Subject detection failed",
    resolutionInfo: "Resolution Info",
    wholeImage: "Whole Image",
    perTile: "Per Tile",
    processAll: "Process All Tiles",
    processing: "Processing Tiles...",
    uploadInstruction: "Click or Drop Image Here",
    generate: "Generate",
    regenerate: "Regenerate",
    save: "Save",
    revert: "Revert",
    holdToShowOriginal: "Hold to show original",
    controls: "Controls",
    logs: "Logs",
    settings: {
      title: "Settings",
      language: "Language",
      appearance: "Appearance",
      operationMode: "Operation Mode",
      operationModeDefault: "Default (Image-to-Image)",
      operationModeMock: "Mock (Local Noise)",
      operationModeTestT2i: "Test (AI Text-to-Image)",
      themeLight: "Light",
      themeDark: "Dark",
      apiKey: "Google AI API Key",
      apiKeyHint: "Used for Google AI requests. Saved locally on this device.",
      apiUrl: "API Base URL",
      show: "Show",
      hide: "Hide",
      clear: "Clear",
      modelName: "Model Name",
      verboseLogging: "Verbose Logging (log final prompts)",
      fullImageReference: "Use full image reference",
      systemPrompt: "Prompt",
      promptTemplate: "Prompt Template",
      promptTemplateWithReference: "Prompt Template (With Reference)",
      promptTemplateWithoutReference: "Prompt Template (No Reference)",
      placeholdersLabel: "Placeholders",
      restoreDefault: "Restore default",
      cancel: "Cancel",
      save: "Save",
      experimental: "Experimental: try 'gemini-3-pro-image-preview' if available.",
      testMode: "Test Generation Mode (Mock)"
    }
  },
  zh: {
    appTitle: "区块化背景去除工具",
    tools: "工具",
    cropImage: "裁剪图片",
    centerCrop: "1:1 居中",
    gridLayout: "网格布局",
    smartGrid: "智能",
    rows: "行数",
    cols: "列数",
    overlap: "重叠率",
    aiOutputRes: "AI 输出分辨率",
    smartQuality: "切片尺寸容差（质量更低）",
    smartQualityHint: "数值越高，切片越大、数量越少、质量越低。",
    tileCount: "切片数量",
    lowerQuality: "较低质量",
    higherQuality: "较高质量",
    showTileLines: "显示切片线",
    resizeInput: "调整输入区块以匹配输出",
    concurrency: "最大并发数",
    bgRemoval: "背景移除",
    bgRemoved: "已去底",
    bgRemovedSuffix: "已去底",
    keyColor: "抠像颜色",
    tolerance: "容差",
    mainSubject: "主体识别",
    subjectForPrompt: "用于提示词的主体",
    subjectPlaceholder: "输入主体（例如：自行车）",
    usingSubject: "当前使用",
    useDetectedSubject: "使用识别结果",
    detectingSubject: "正在识别主体...",
    noApiKeySubject: "设置 API Key 后可识别主体",
    subjectDetectFailed: "主体识别失败",
    resolutionInfo: "分辨率信息",
    wholeImage: "完整图片",
    perTile: "单个区块",
    processAll: "处理所有区块",
    processing: "处理中...",
    uploadInstruction: "点击或拖拽图片到此处",
    generate: "生成",
    regenerate: "重新生成",
    save: "保存",
    revert: "还原",
    holdToShowOriginal: "按住显示原图",
    controls: "控制",
    logs: "日志",
    settings: {
      title: "设置",
      language: "语言",
      appearance: "外观",
      operationMode: "运行模式",
      operationModeDefault: "默认（图生图）",
      operationModeMock: "模拟（本地噪声）",
      operationModeTestT2i: "测试（AI 文生图）",
      themeLight: "浅色",
      themeDark: "深色",
      apiKey: "Google AI API 密钥",
      apiKeyHint: "用于 Google AI 请求。仅保存在当前设备本地。",
      apiUrl: "API 基础地址",
      show: "显示",
      hide: "隐藏",
      clear: "清空",
      modelName: "模型名称",
      verboseLogging: "详细日志（记录最终提示词）",
      fullImageReference: "使用整图参考",
      systemPrompt: "提示词",
      promptTemplate: "提示词模板",
      promptTemplateWithReference: "提示词模板（有参考图）",
      promptTemplateWithoutReference: "提示词模板（无参考图）",
      placeholdersLabel: "占位符",
      restoreDefault: "恢复默认",
      cancel: "取消",
      save: "保存",
      experimental: "实验性：如果可用，请尝试 'gemini-3-pro-image-preview'。",
      testMode: "测试生成模式 (模拟)"
    }
  },
  ja: {
    appTitle: "Gemini タイル超解像",
    tools: "ツール",
    cropImage: "画像をクロップ",
    centerCrop: "1:1 中央",
    gridLayout: "グリッドレイアウト",
    smartGrid: "スマート",
    rows: "行数",
    cols: "列数",
    overlap: "オーバーラップ",
    aiOutputRes: "AI 出力解像度",
    smartQuality: "タイルサイズ許容値（低品質側）",
    smartQualityHint: "値が高いほどタイルが大きくなり、枚数が減り、品質は下がります。",
    tileCount: "タイル数",
    lowerQuality: "低品質",
    higherQuality: "高品質",
    showTileLines: "タイル線を表示",
    resizeInput: "入力タイルを出力に合わせてリサイズ",
    concurrency: "最大同時実行数",
    bgRemoval: "背景削除",
    bgRemoved: "背景除去済み",
    bgRemovedSuffix: "背景除去済み",
    keyColor: "クロマキー色",
    tolerance: "許容値",
    mainSubject: "主要被写体",
    subjectForPrompt: "プロンプト用の被写体",
    subjectPlaceholder: "被写体を入力（例: 自転車）",
    usingSubject: "使用中",
    useDetectedSubject: "認識結果を使用",
    detectingSubject: "被写体を認識中...",
    noApiKeySubject: "被写体認識には API キーが必要です",
    subjectDetectFailed: "被写体の認識に失敗しました",
    resolutionInfo: "解像度情報",
    wholeImage: "画像全体",
    perTile: "タイルごと",
    processAll: "すべてのタイルを処理",
    processing: "処理中...",
    uploadInstruction: "ここをクリックまたは画像をドロップ",
    generate: "生成",
    regenerate: "再生成",
    save: "保存",
    revert: "元に戻す",
    holdToShowOriginal: "長押しでオリジナルを表示",
    controls: "操作",
    logs: "ログ",
    settings: {
      title: "設定",
      language: "言語",
      appearance: "外観",
      operationMode: "動作モード",
      operationModeDefault: "デフォルト（画像から画像）",
      operationModeMock: "モック（ローカルノイズ）",
      operationModeTestT2i: "テスト（AI テキストから画像）",
      themeLight: "ライト",
      themeDark: "ダーク",
      apiKey: "Google AI API キー",
      apiKeyHint: "Google AI リクエストに使用します。この端末にのみ保存されます。",
      apiUrl: "API ベース URL",
      show: "表示",
      hide: "非表示",
      clear: "クリア",
      modelName: "モデル名",
      verboseLogging: "詳細ログ（最終プロンプトを記録）",
      fullImageReference: "全体画像を参照として使用",
      systemPrompt: "プロンプト",
      promptTemplate: "プロンプトテンプレート",
      promptTemplateWithReference: "プロンプトテンプレート（参照あり）",
      promptTemplateWithoutReference: "プロンプトテンプレート（参照なし）",
      placeholdersLabel: "プレースホルダー",
      restoreDefault: "デフォルトに戻す",
      cancel: "キャンセル",
      save: "保存",
      experimental: "実験的：利用可能な場合は 'gemini-3-pro-image-preview' を試してください。",
      testMode: "テスト生成モード (モック)"
    }
  }
} as const;

export const t = derived(locale, ($locale) => (key: string) => {
  const keys = key.split('.');
  const localeTable =
    (translations as Record<string, unknown>)[$locale] ??
    (translations as Record<string, unknown>).en;
  let val: unknown = localeTable;
  for (const k of keys) {
    if (!val || typeof val !== 'object') {
      return key;
    }
    val = (val as Record<string, unknown>)[k];
  }
  return typeof val === 'string' && val.length > 0 ? val : key;
});
