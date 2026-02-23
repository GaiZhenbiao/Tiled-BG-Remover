<p align="right"><a href="README.md">English</a> | <a href="README.zh.md">中文</a> | 日本語</p>

# Tiled BG Remover

高解像度画像をタイル単位で処理する AI デスクトップアプリです。

## 概要
Tiled BG Remover は大きな画像をタイル分割し、Gemini 画像モデルで処理した結果をフロントエンドで合成プレビューします。

主な用途：
- 背景を単色化、またはキー色ベースで背景除去
- 大判画像の分割処理と継ぎ目制御
- 選択範囲のみ再生成（`Generate In Box`）
- 結合画像、タイル、レイヤー付き PSD の出力

## 主な機能
- スマートグリッド + 手動行列設定
- オーバーラップ調整、正方形タイル固定オプション
- タイル単位の生成/再生成
- 選択ボックス生成（独立オーバーレイレイヤー）
- ボックスレイヤーパネル（表示/非表示/削除）
- ズーム、パン、スクロール、中クリックドラッグ
- UI 多言語対応（`en`、`zh`、`ja`）
- API Key、API URL、モデル、プロンプトテンプレート、詳細ログの設定

## 技術スタック
- フロントエンド: SvelteKit + TypeScript
- デスクトップ: Tauri v2
- バックエンド: Rust（`src-tauri`）
- AI 連携: Google Gemini API（`src/lib/api.ts`）

## 必要環境
- Node.js 18+
- npm 9+
- Rust stable ツールチェーン
- OS ごとの Tauri ビルド依存

## 開発起動
```bash
npm install
npm run tauri dev
```

## ビルド
```bash
npm run tauri build
```

## Gemini 設定
設定画面で以下を入力：
- `Google AI API Key`
- 必要に応じて `API Base URL`
- モデル名（設定で変更可能）

## 基本フロー
1. 画像をアップロード
2. グリッドとオーバーラップを調整（またはスマートモード）
3. `Process All Tiles` で一括処理、またはタイルごとに処理
4. 局所編集は `Generate In Box` を使用
5. ボックスレイヤーパネルでオーバーレイを管理
6. 必要な出力形式（結合画像、タイル、PSD）を選んでエクスポート

## 構成
- `src/routes/+page.svelte`: メイン UI
- `src/lib/TileGrid.svelte`: キャンバス、タイル処理、ボックス生成、プレビュー合成
- `src/lib/CropModal.svelte`: フロントエンドクロップ UI
- `src/lib/Settings.svelte`: 設定画面
- `src/lib/api.ts`: Gemini API 呼び出し/被写体検出
- `src-tauri/src/lib.rs`: Tauri コマンド/エクスポート処理
- `src-tauri/src/image_processing.rs`: 画像処理ユーティリティ
