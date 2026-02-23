<p align="right"><a href="README.md">English</a> | 中文 | <a href="README.ja.md">日本語</a></p>

# Tiled BG Remover

用于高分辨率 AI 图像处理的桌面应用，采用切片流程。

## 项目简介
Tiled BG Remover 会先将大图切成多个切片，再调用 Gemini 图像模型处理，并在前端实时合成预览，便于快速迭代。

适合场景：
- 背景统一为纯色或抠像去底
- 大图分块处理并控制拼接边缘
- 局部框选重生成（`Generate In Box`）
- 导出合并图、切片目录和分层 PSD

## 主要功能
- 智能网格 + 手动行列控制
- 可调重叠率，支持强制方形切片
- 单切片生成/重生成
- 框选生成（独立叠加层，非破坏式）
- 框选图层面板：显示/隐藏/删除
- 缩放、平移、滚动、中键拖拽
- 界面本地化（`en`、`zh`、`ja`）
- 设置项：API Key、API URL、模型、提示词模板、详细日志

## 技术栈
- 前端：SvelteKit + TypeScript
- 桌面容器：Tauri v2
- 后端：Rust（`src-tauri`）
- AI 接口：Google Gemini API（`src/lib/api.ts`）

## 环境要求
- Node.js 18+
- npm 9+
- Rust stable 工具链
- 对应系统的 Tauri 构建依赖

## 快速启动
```bash
npm install
npm run tauri dev
```

## 构建发布
```bash
npm run tauri build
```

## 配置 Gemini
在设置中配置：
- `Google AI API Key`
- 可选 `API Base URL`
- 模型名称（可在设置中修改默认值）

## 基本使用流程
1. 上传图片。
2. 调整网格与重叠率（或使用智能模式）。
3. 点击 `Process All Tiles` 或逐块生成。
4. 需要局部修改时启用 `Generate In Box`。
5. 在框选图层面板中管理叠加层。
6. 导出所选内容（合并图、切片目录、PSD）。

## 项目结构
- `src/routes/+page.svelte`：主界面
- `src/lib/TileGrid.svelte`：画布、切片、框选生成、预览合成
- `src/lib/CropModal.svelte`：前端裁剪覆盖层
- `src/lib/Settings.svelte`：设置面板
- `src/lib/api.ts`：Gemini 请求与主体识别
- `src-tauri/src/lib.rs`：Tauri 命令与导出流程
- `src-tauri/src/image_processing.rs`：后端图像处理工具
