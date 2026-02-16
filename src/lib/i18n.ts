import { writable, derived } from 'svelte/store';

export const locale = writable('en');

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
    resizeInput: "Resize input tile to match output",
    bgRemoval: "Background Removal",
    keyColor: "Key Color",
    tolerance: "Tolerance",
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
    controls: "Controls",
    logs: "Logs",
    settings: {
      title: "Settings",
      apiKey: "Google AI API Key",
      modelName: "Model Name",
      systemPrompt: "Prompt",
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
    resizeInput: "调整输入区块以匹配输出",
    bgRemoval: "背景移除",
    keyColor: "抠像颜色",
    tolerance: "容差",
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
    controls: "控制",
    logs: "日志",
    settings: {
      title: "设置",
      apiKey: "Google AI API 密钥",
      modelName: "模型名称",
      systemPrompt: "提示词",
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
    resizeInput: "入力タイルを出力に合わせてリサイズ",
    bgRemoval: "背景削除",
    keyColor: "クロマキー色",
    tolerance: "許容値",
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
    controls: "操作",
    logs: "ログ",
    settings: {
      title: "設定",
      apiKey: "Google AI API キー",
      modelName: "モデル名",
      systemPrompt: "プロンプト",
      cancel: "キャンセル",
      save: "保存",
      experimental: "実験的：利用可能な場合は 'gemini-3-pro-image-preview' を試してください。",
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