# SnapDown

桌面视频下载器，支持 **Bilibili**、**抖音**、**YouTube** 平台。

基于 Tauri + Vue 3 + Rust + Python（yt-dlp）构建，安装即用，无需额外配置 Python 环境。

## 功能

- 粘贴链接即可下载，自动识别平台
- 支持选择清晰度（1080P / 720P / 480P）
- 多种输出格式：
  - 视频：MP4（视频+音频）、MKV（视频+音频）、仅视频（无声音）
  - 音频：MP3、M4A（AAC）、FLAC（无损）
- 实时显示下载进度、速度、剩余时间
- 参数设置自动持久化，修改后下次启动保留
- 支持导入浏览器 Cookie 文件（用于 YouTube 登录态验证）
- 下载历史持久化，支持单条删除和批量清除

## 安装与使用

### 直接安装（推荐）

下载 `SnapDown_x.x.x_x64-setup.exe` 安装包，双击安装即可使用。

安装包已内嵌 Python 运行时和 yt-dlp 下载引擎（约 12 MB），无需额外安装 Python。

### FFmpeg（推荐安装）

FFmpeg 用于合并最佳画质视频+最佳音质音频、提取音频等操作。应用会自动检测系统中已安装的 FFmpeg。

```bash
# 方式一：Scoop
scoop install ffmpeg

# 方式二：WinGet
winget install ffmpeg

# 方式三：手动下载
# 从 https://www.gyan.dev/ffmpeg/builds/ 下载 essentials 版本
# 将 bin 目录加入系统 PATH
```

未安装 FFmpeg 时仍可下载视频，但：
- 仅能获取单流格式（画质或音质可能受限）
- 无法使用音频提取功能（MP3/M4A/FLAC）

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | Vue 3 + Element Plus + TypeScript |
| 桌面框架 | Tauri 1.x（Rust） |
| 下载引擎 | Python 3.12（内嵌）+ yt-dlp |
| 音视频处理 | FFmpeg（系统环境变量检测） |

## 开发指南

### 环境要求

| 依赖 | 版本要求 | 用途 |
|------|---------|------|
| Node.js | >= 18 | 前端构建 |
| Rust | >= 1.70（含 cargo） | Tauri 后端编译 |
| Python | >= 3.9 | 开发时运行下载引擎 |
| yt-dlp | >= 2024.12 | 视频解析与下载 |

> 开发模式下使用系统 Python，打包后使用内嵌的 Python 3.12。

### 安装依赖

```bash
# 前端依赖
npm install

# Python 依赖（开发用）
npm run setup:python
```

### 开发模式启动

```bash
npm run tauri:dev
```

启动后会同时运行 Vite 开发服务器和 Tauri 桌面窗口，前端修改后自动热更新。

### 打包发布

**准备内嵌 Python（首次打包前执行一次）：**

```bash
# 下载 Python 3.12 嵌入式包并安装 yt-dlp
# 详见 src-tauri/python-bundle/ 目录
```

**生成安装包：**

```bash
npx tauri build --bundles nsis
```

安装程序位于 `target/release/bundle/nsis/SnapDown_<版本>_x64-setup.exe`。

## 项目结构

```
├── src/                            # Vue 前端
│   ├── App.vue                     # 主界面
│   └── main.ts                     # 入口
├── src-tauri/                      # Tauri + Rust 后端
│   ├── src/
│   │   ├── main.rs                 # 应用入口
│   │   ├── commands.rs             # Tauri 命令
│   │   ├── models.rs               # 数据模型
│   │   └── state.rs                # 状态管理与 Python 路径解析
│   ├── python/
│   │   ├── downloader_service.py   # 下载引擎核心
│   │   └── requirements.txt        # Python 依赖
│   ├── python-bundle/              # 内嵌 Python 运行时（打包用）
│   │   ├── python.exe              # Python 3.12 嵌入式
│   │   ├── downloader_service.py   # 下载脚本副本
│   │   └── Lib/site-packages/      # yt-dlp 等依赖
│   ├── icons/                      # 应用图标
│   ├── Cargo.toml                  # Rust 依赖
│   └── tauri.conf.json             # Tauri 配置
├── package.json
└── vite.config.ts
```

## 数据存储

应用数据保存在 `%APPDATA%/SnapDown/` 目录下：

| 文件 | 内容 |
|------|------|
| `settings.json` | 用户设置（输出目录、Cookie 路径、重试次数） |
| `tasks.json` | 下载任务历史 |

## Cookie 配置

若下载遇到 HTTP 412 错误（被限制/禁止下载）或 YouTube 提示 "Sign in to confirm you're not a bot" 等错误，需要导入 Cookie：

1. **Cookie 格式要求**：必须为 **Netscape** 格式。
2. **获取工具**：可以在 Chrome 或 Edge 浏览器中安装 **Cookie-Editor** 插件（或使用 Chrome 的 **Get cookies.txt LOCALLY** 扩展）。
3. **获取步骤**：
   - 在浏览器中打开并登录对应的网站（如 Bilibili 或 YouTube）。
   - 点击插件，选择导出格式为 **Netscape**，将导出的文本保存为 `.txt` 文件。
4. **导入方式**：在 SnapDown **参数设置 → Cookie 文件** 中选择刚才保存的 `.txt` 文件。

## 许可

仅供个人学习使用。请仅下载你有权访问或已获授权的内容。
