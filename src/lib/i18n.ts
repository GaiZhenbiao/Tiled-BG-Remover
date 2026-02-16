import { writable, derived } from 'svelte/store';

export const locale = writable('en');

const translations = {
  en: {
    appTitle: "Tiled BG Remover",
    tools: "Tools",
    cropSquare: "Crop to Square (Center)",
    gridLayout: "Grid Layout",
    rows: "Rows",
    cols: "Cols",
    overlap: "Overlap",
    tileRes: "Tile Resolution",
    processAll: "Process All Tiles",
    processing: "Processing...",
    uploadInstruction: "Click or Drop Image Here",
    generate: "Generate",
    regenerate: "Regenerate",
    settings: {
      title: "Settings",
      apiKey: "Google AI API Key",
      modelName: "Model Name",
      systemPrompt: "Prompt",
      cancel: "Cancel",
      save: "Save",
      experimental: "Experimental: try 'gemini-2.0-flash-exp' or 'gemini-3-pro-image-preview' if available.",
      testMode: "Test Generation Mode (Mock)"
    }
  },
  zh: {
    appTitle: "瓦片化背景去除工具",
    tools: "工具",
    cropSquare: "居中裁剪 (1:1)",
    gridLayout: "网格布局",
    rows: "行数",
    cols: "列数",
    overlap: "重叠率",
    tileRes: "瓦片分辨率",
    processAll: "处理所有瓦片",
    processing: "处理中...",
    uploadInstruction: "点击或拖拽图片到此处",
    generate: "生成",
    regenerate: "重新生成",
    settings: {
      title: "设置",
      apiKey: "Google AI API 密钥",
      modelName: "模型名称",
      systemPrompt: "提示词",
      cancel: "取消",
      save: "保存",
      experimental: "实验性：如果可用，请尝试 'gemini-2.0-flash-exp' 或 'gemini-3-pro-image-preview'。",
      testMode: "测试生成模式 (模拟)"
    }
  },
  ja: {
    appTitle: "タイル式背景除去ツール",
    tools: "ツール",
    cropSquare: "中央クロップ (1:1)",
    gridLayout: "グリッドレイアウト",
    rows: "行数",
    cols: "列数",
    overlap: "オーバーラップ",
    tileRes: "タイル解像度",
    processAll: "すべてのタイルを処理",
    processing: "処理中...",
    uploadInstruction: "ここをクリックまたは画像をドロップ",
    generate: "生成",
    regenerate: "再生成",
    settings: {
      title: "設定",
      apiKey: "Google AI API キー",
      modelName: "モデル名",
      systemPrompt: "プロンプト",
      cancel: "キャンセル",
      save: "保存",
      experimental: "実験的：利用可能な場合は 'gemini-2.0-flash-exp' または 'gemini-3-pro-image-preview' を試してください。",
      testMode: "テスト生成モード (モック)"
    }
  }
};

export const t = derived(locale, ($locale) => (key) => {
  const keys = key.split('.');
  let val = translations[$locale];
  for (const k of keys) {
    val = val?.[k];
  }
  return val || key;
});