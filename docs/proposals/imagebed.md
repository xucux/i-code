# 社区编辑器内置图床上传（imagebed）整合方案

> 状态：**提案**（已进入 Phase 1 实现）
> 日期：2026-09-06
> 关联模块：新增 `imagebed`（Rust + 前端）、`community`（MarkdownEditor 集成）
> 已接入图床：
> - [img.remit.ee](https://img.remit.ee/)（id=`remit`，油猴脚本适配：去广告 + 粘贴上传面板 + 复制按钮 + 外链自动回插）
> - [ooxx.ooo](https://ooxx.ooo/upload)（id=`ooxx`，精简脚本：粘贴上传面板 + 手动上传，无广告处理 / 无复制按钮 / 无回传）

---

## 1. 背景与目标

社区帖子 / 一级回复正文支持 Markdown 图片（`![alt](url)`），但用户需要外链图床才能贴图：手动打开浏览器 → 上传 → 复制外链 → 切回编辑器中粘贴，链路长、频繁。

第三方图床多为标准 Web 交互（拖拽 / 选文件 / 粘贴），社区对两款图床的诉求不同：

| 图床 | 诉求 |
|------|------|
| img.remit.ee | 去广告 + 粘贴上传面板 + 卡片复制按钮（Markdown/HTML/直链）+ 外链自动回插编辑器 |
| ooxx.ooo | 只需「粘贴图片 → 浮动面板 → 点上传」；广告不用管、复制按钮站点自带、**不做外链回传**（人工复制） |

### 目标

1. **内置浏览器窗口**：编辑器工具栏「图床」下拉 → 选择图床 → 弹出独立窗口加载所选图床。
2. **脚本注入**：窗口创建时向图床页面 document-start 注入篡改猴式增强脚本（因站定制，见 §5）。
3. **外链获取**（remit）：点复制按钮 → 外链自动插入编辑器光标处，剪贴板兜底；ooxx：站点结果区人工复制。
4. **可扩展**：内置 provider 表驱动，新增图床只需加一个目录 + 一条表项（见 §7）。

---

## 2. 架构决策（关键）

| 决策点 | 结论 | 理由 |
|--------|------|------|
| 浏览器形态 | 独立 `WebviewWindow`（label=`imagebed-browser`）+ `WebviewUrl::External`，保留系统边框 | 远程页面无法跑本地 React UI，需要原生标题栏关闭/最小化 |
| 脚本注入 | `WebviewWindowBuilder::initialization_script(...)` | Tauri 内唯一能对远程站点在 document-start 注入脚本的通道（**对 iframe 无效**——WebView2 只对 top-level 文档注入，因此否决 iframe 方案） |
| 数据回传（remit） | **document.title 桥接**：脚本把外链 markdown 写入 `document.title = "ICODE_IMGBED:<markdown>"`；Rust 每 800ms 读 `window.title()` 解析 → `emit("imagebed:link-ready")` | 注入脚本无法调 Tauri IPC（远程页面无 `__TAURI__`）；title 是 WebView 侧唯一能被 Rust 读到的页面状态。ooxx 不走此通道 |
| 兜底通道 | 脚本复制按钮同时写剪贴板（`navigator.clipboard` + execCommand fallback） | 超长 URL / title 解析失败时，用户仍可 Ctrl+V 粘贴到编辑器 |
| 覆盖范围 | 仅改 `community/ui/markdown-editor.tsx` 一处 | 发帖 / 编辑 / 一级回复共用该组件，自动全覆盖 |

---

## 3. 端到端集成链路

```
编辑器工具栏「图床」下拉（DropdownMenu，列表来自 imagebed_list）
  → 选择 provider → invokeCommand("imagebed_open", { providerId })
  → Rust 创建 imagebed-browser 窗口（External URL + initialization_script 注入定制脚本）
  → 用户在窗口内交互（remit：拖拽/选文件/粘贴面板；ooxx：粘贴面板 + 手动上传）
  → 上传成功：
       remit：点「复制 Markdown/直链」→ 写剪贴板 + document.title 桥接
       ooxx：站点 #tab-markdown 生成直链（人工复制，不回传）
  → Rust 轮询线程每 800ms 读窗口标题，识别 ICODE_IMGBED: 前缀
  → app.emit("imagebed:link-ready", { providerId, url, markdown, createdAt })
  → 前端 store（imagebed-listen 单例）push → MarkdownEditor consume
  → 焦点在编辑器 → 光标处插入 markdown；否则 toast 提示「已复制，Ctrl+V」
```

标题桥接 edge case：超长（>900 字符）只写 `LONG` 标记走剪贴板兜底；站点覆盖 title 时下次点按钮重新上报；`last_seen` 去重防止重复 emit；窗口关闭轮询线程自动退出。

---

## 4. 模块与文件清单

### 4.1 后端 Rust（`src-tauri/src/modules/imagebed/`）

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块声明，导出 `ImagebedHandle` |
| `types.rs` | `ImagebedProviderSpec`（内部含脚本）/ `ImagebedProvider`（DTO，不含脚本）/ `ImagebedLinkReady`（事件 payload） |
| `service.rs` | 窗口生命周期（open/close，幂等复用）、标题桥接轮询线程（800ms）、事件发射；常量 `IMAGEBED_LABEL` / `BRIDGE_PREFIX`(ICODE_IMGBED:) / `BRIDGE_LONG_MARKER`(LONG) / `EVENT_LINK_READY`(imagebed:link-ready) |
| `commands.rs` | `imagebed_list` / `imagebed_open` / `imagebed_close`（**均必须 async**，见 §6 坑 1） |
| `providers/mod.rs` | 内置 provider 表 `ALL_PROVIDERS` + `provider_by_id` |
| `providers/remit/mod.rs` | remit provider 定义（`inject_script: include_str!("user.js")`） |
| `providers/remit/user.js` | remit 注入脚本（见 §5.1） |
| `providers/ooxx/mod.rs` | ooxx provider 定义 |
| `providers/ooxx/user.js` | ooxx 注入脚本（见 §5.2） |

注册点：`src-tauri/src/main.rs` —— 三条 command 注册进 `invoke_handler`；setup 中 `app.manage(ImagebedHandle::new(app.handle().clone()))`；模块声明在 `modules/mod.rs`。

### 4.2 前端（`src/modules/imagebed/` + 社区）

| 文件 | 职责 |
|------|------|
| `types.ts` | `ImagebedProvider` / `ImagebedLinkReady`（与后端 DTO 驼峰对齐） |
| `store.ts` | zustand：`providers` 列表懒加载（imagebed_list）、`pending` 外链单槽（push/consume）、`registerImagebedEvents()` 模块级单例 listen |
| `src/core/events.ts` | `BACKEND_EVENTS.IMAGEBED_LINK_READY = 'imagebed:link-ready'` |
| `src/main.tsx` | 入口调用 `registerImagebedEvents()` |
| `src/modules/community/ui/markdown-editor.tsx` | 工具栏末尾「图床」DropdownMenu（懒加载 provider 列表，选择即打开）；`useEffect` 订阅 pending 外链：焦点在本 textarea 时 `apply` 插入 `markdown + '\n'`，否则 toast |
| i18n | 四语言 `community.editor.imagebed*`（imagebed / imagebedOpen / imagebedCopied / imagebedNone） |

---

## 5. 注入脚本适配说明

### 5.1 img.remit.ee（`providers/remit/user.js`）

基于用户提供的篡改猴脚本《img.remit.ee 图床助手》v1.2.0（≈900 行）最小改动：

| 段 | 处理 |
|----|------|
| Tampermonkey metadata（`==UserScript==` / `@grant` / `@match` / `@run-at`） | 删除；在顶部补**代码级站点守卫**（`location.href` 正则，等价 @match） |
| `copyText`（GM_setClipboard 分支） | 不动——已有 `typeof` 判断，Tauri 中自动落 `navigator.clipboard` → `execCommand` fallback |
| 复制按钮成功回调 | 追加 `reportToBridge(text)`：写 `document.title` 桥接（>900 字符写 `LONG`） |
| 去广告 CSS/JS、粘贴上传面板、卡片扫描、`window.REMIT_HELPER` | 全部保留 |
| `boot()` | 调整为 **DOMContentLoaded 后再执行**（document-start 过早 DOM 未就绪会崩溃，见 §6 坑 2） |

站点结构依赖：CSS Modules 类名 `[class*="fileCard"]` 等模糊匹配（抗哈希变更）；上传复用站点 `<input id="file-upload">`（DataTransfer 注入触发 change）。

### 5.2 ooxx.ooo（`providers/ooxx/user.js`）

**浏览器实测调研结论**（浏览器自动化访问该站）：服务端渲染、Cloudflare 前置、无验证码无登录；上传由隐藏 `input#import-file-select[type=file]`（`name="files[]"`、multiple、accept 图片）驱动，表单带 `_xsrf` 防 CSRF；**站点无原生 paste 监听**；上传成功后在 `#tab-markdown` 自动填 `![](url)`（另有 `#tab-url` / `#tab-bbcode` / `#tab-html`）。

脚本能力（自研精简版，约 320 行）：
- 顶部站点守卫（仅 `ooxx.ooo` 域运行）
- 左侧浮动粘贴面板（FAB 折叠 + 面板：预览缩略图 / 文件名 / 大小 / 逐张上传 / 全部上传 / 清空 / 折叠），样式与 remit 面板一致（蓝色主题区分）
- `paste` 捕获（非文本框）→ 剪贴板图片进面板，**点击上传才上传**
- 上传 = `DataTransfer` 注入 `#import-file-select` + `change`，**完整复用站点 `_xsrf`/AJAX 链路**
- 成功判定：轮询 `#tab-markdown` 文本是否变化（60s 超时 toast 失败提示）
- **刻意不做**：去广告、复制按钮（站点自带）、document.title 桥接（人工复制）

---

## 6. 踩坑记录（曾遇到的问题与修复）

### 坑 1：图床窗口白屏 —— 同步 Command 创建 Webview
- **现象**：`imagebed_open` 打开窗口后整窗白屏。
- **根因**：`imagebed_open` 写成**同步 command**；Tauri 官方文档明确 Windows 上在同步 Command / 事件处理器中创建 Webview 会 **deadlock**（对照 `open_mini_panel` 用 async 故正常）。
- **修复**：`imagebed_open` / `imagebed_close` 改为 `async fn`（`pub async fn`），Builder 构建在 async 线程上下文执行。

### 坑 2：注入脚本 document-start 过早崩溃
- **现象**：控制台 `Uncaught TypeError: Cannot read properties of null (reading 'appendChild')`，样式注入失败。
- **根因**：Tauri `initialization_script` 在 document-start 极早期注入，`document.head/html` 可能尚未建立；原篡改猴脚本（`@run-at document-start` 由 Tampermonkey 保证时序）直接 `appendChild` 无守卫。
- **修复**：`injectStyle()` 增加空 DOM 守卫；`boot()` 统一挂到 `DOMContentLoaded` 后再注入样式 / 启动监听 / 建面板。

### 坑 3：脚本跑在不匹配的站点（出现 `目标站点 https://www.google.com`）
- **现象**：控制台日志显示注入脚本在 google.com 等非图床页面运行并报错刷屏。
- **根因**：适配时删除了 `@match`，脚本失去站点限制，导航到任何页面（错误页 / 新标签）都会执行。
- **修复**：两个脚本顶部都补**代码级站点守卫**（`location.href` 正则不匹配直接 `return`）。

### 坑 4：候选风险——图床站点识别 WebView2 UA
- 部分图床会对非主流 UA 返回拦截/空白；**预防措施**：窗口 Builder 显式覆盖 `user_agent` 为桌面 Chrome UA（见 `service.rs::open`），规避 UA 检测。

### 坑 5：标题桥接工程细节
- title 会被浏览器截断 → 超长外链（>900）只写 `LONG` 标记，不构造直链事件，靠剪贴板兜底；
- 站点可能自行覆盖 `document.title` → 幂等上报（每次点按钮重写）+ Rust `last_seen` 去重，变化才 emit，不丢也不重复；
- 连续多张：每点一次按钮上报新 title，逐条 emit，前端逐个消费。

---

## 7. 扩展指南：新增一个图床

1. **调研站点**：用浏览器自动化访问目标站，确认上传控件（`input[type=file]` 的选择器 / 是否有 paste 监听）、上传/结果链路（是否有 `_xsrf` 之类隐藏字段、成功后外链展示位置）、能否刷新 UA 兼容。
2. **写注入脚本**：`src-tauri/src/modules/imagebed/providers/{id}/user.js`（骨架复制 ooxx 或 remit）：顶部站点守卫 + 粘贴面板 + 上传注入 + （可选）title 桥接。
3. **注册 provider**：新建 `providers/{id}/mod.rs` 定义 `XXX_PROVIDER`（`include_str!("user.js")`），并在 `providers/mod.rs` 的 `ALL_PROVIDERS` 追加。
4. **前端**：无需改动——图床下拉自动列出 `imagebed_list` 返回的新 provider。
5. 验证 `node --check` 语法 + `cargo check`。

---

## 8. 实施范围与迭代规划

### Phase 1（本期，已完成）
- 内置图床：img.remit.ee（油猴脚本适配：去广告 + 粘贴上传 + 复制按钮 + title 桥接自动插入）、ooxx.ooo（精简面板：粘贴收集 + 手动上传，人工复制外链）
- 社区 Markdown 编辑器「图床」下拉按钮：列出全部内置图床，选择后打开；发帖 / 编辑 / 一级回复全场景可用
- 事件链路：Rust 轮询 title → emit → 前端 store → 编辑器自动插入（remit）；剪贴板兜底

### Phase 2+（迭代候选）
- 用户自填图床地址 + 每站点脚本管理（脚本编辑 / 启用禁用）
- 图床窗口地址栏 / 前进后退 / 「在系统浏览器打开」兜底按钮
- 上传失败重试、配额 / 错误提示（IcodeError toast 复用）
- provider 表归档 DB（当前为 Rust 常量）