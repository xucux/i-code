# 社区 Markdown 图片增强（尺寸后缀）设计提案

> 状态：**✅ 已完成（2026-09-06 实现并本地验证通过）**
> 日期：2026-09-06
> 关联模块：`community/ui/community-markdown-content.tsx`（渲染）、`community/ui/markdown-editor.tsx`（工具栏插入）
> 覆盖范围：帖子正文 / 一级评论 / 发帖·回复预览 / 管理端帖子列表，全部复用 `CommunityMarkdownContent`，单点修改全局生效

---

## 1. 背景与需求

### 1.1 需求（用户原话梳理）

社区正文 Markdown 目前图片只能按默认样式渲染（等比缩放不溢出容器），无法控制展示尺寸。参考 Obsidian 语法，期望：

| # | 需求 | 说明 |
|---|------|------|
| R1 | 图片增强显示 | 支持 `![alt|WIDTHxHEIGHT](url)` / `![alt|WIDTH](url)` 语法，按指定像素渲染 |
| R2 | 编辑器工具栏联动 | 插入图片按钮生成带尺寸后缀的占位语法，默认宽度 300 |

### 1.2 现状梳理（改动前确认）

- 社区 Markdown 渲染为**独立隔离实例** `communityMarked`（`marked` v18 + GFM + `breaks`），与全局 `components/ui/markdown-content.tsx` 完全解耦；已自定义 `code` renderer（高亮 + 折叠 + 操作菜单）。
- 图片布局类：普通态 `prose-img:max-w-full`；评论区紧凑态 `compactImages` 时 `prose-img:max-w-[33.333%]`（缩为容器 1/3 宽）。
- 点击图片放大走事件委托 → 读取 `img.src` → `CommunityImageLightbox`（全屏遮罩 + 自由缩放）。
- 编辑器工具栏「图片」按钮当前插入 `![alt](url)`（无尺寸）。

### 1.3 已确认决策（S1 ~ S4）

| # | 决策点 | ✅ 已确认方案 | 对应章节 |
|---|--------|---------------|----------|
| S1 | 尺寸语法 | 采用 Obsidian 风格 `![alt\|WIDTH](url)` 与 `![alt\|WIDTHxHEIGHT](url)`，尺寸以**像素**为单位 | §2 |
| S2 | 实现方式 | 在 `communityMarked.use()` 追加自定义 `image` renderer 解析 alt 后缀，输出内联 `style` 尺寸 | §3 |
| S3 | 紧凑布局交互 | 带尺寸图片标记 `community-img-sized`，`compactImages` 时跳过 1/3 宽钳制，保留设定尺寸 | §4 |
| S4 | 工具栏默认值 | 图片按钮插入 `![图片描述|300](https://)`，用户可改宽或改为 `宽x高` | §5 |

### 1.4 非目标（本期不做）

| 非目标 | 说明 |
|--------|------|
| 单位扩展 | 仅支持像素整数（`\d+`）；不支持 `em` / `%` / 小数 |
| 尺寸 + 放大联动 | Lightbox 仍按原图自然尺寸展示（`max-w-full max-h-full object-contain`），不按设定尺寸放大 |
| 全局渲染器同步 | 仅社区渲染器支持；软件内其他场景（更新日志等）不受影响 |

---

## 2. 语法约定

```
![alt|WIDTH](url)          → 仅设定宽度（px），高度随原图比例自适应
![alt|WIDTHxHEIGHT](url)   → 同时设定宽高（px），按字面值渲染（遵用户原意，不强改比例）
```

- 尺寸后缀仅影响展示，**不参与 alt 文本**（alt 保留描述，供无障碍 / 图片加载失败时显示）。
- 大小写 `x` / `X` 均接受；无后缀 / 非法后缀（非纯数字）回退默认渲染，行为与改动前一致。

## 3. 渲染实现

### 3.1 正则解析

```ts
/^(.+?)\|(\d+)(?:[xX](\d+))?$/.exec(text)
// 例："image|690x462" → alt="image", width="690", height="462"
// 例："图 片|100X80"  → alt="图 片", width="100", height="80"
// 例："image|690"     → alt="image", width="690", height=null
```

### 3.2 渲染输出

- 命中尺寸：输出 `<img src="..." alt="..." class="community-img-sized" style="width:690px;height:462px" />`；
  仅宽度时 `style="width:690px"`（`prose-img:h-auto` 保证高度自适应）。
- 未命中：保持默认输出，不额外加类。
- 安全：`href` / `alt` / `title` 全部过 `escapeHtml`；宽高仅允许 `\d+` 无注入面。

## 4. 布局与交互

| 场景 | 行为 |
|------|------|
| 帖子正文 / 预览（`compactImages=false`） | 内联尺寸生效；`prose-img:max-w-full` 兜底防溢出（容器窄于设定宽度时钳到 100%） |
| 评论区（`compactImages=true`） | 普通图片仍钳 1/3 宽；`community-img-sized` 图片改为 `max-w-full`，**保持设定尺寸** |
| 点击放大 | 事件委托读 `img.src` → Lightbox，不受内联尺寸影响（原图自然尺寸） |

## 5. 编辑器工具栏

「图片」按钮插入语法由 `![alt](url)` 改为 `![图片描述|300](https://)`：

```ts
const part = '![' + tEd('imageAlt') + '|300](' + tEd('imageUrl') + ')'
```

- `imageAlt` / `imageUrl` 走既有 i18n（zh-CN「图片描述」/「https://」），四语言无需新增键。
- 默认宽度 300 为常量，用户可自行改为 `|690x462` 等。

## 6. 变更清单（已落地）

| 文件 | 变更 |
|------|------|
| `src/modules/community/ui/community-markdown-content.tsx` | `communityMarked.use()` 新增自定义 `image` renderer（§3）；图片布局类区分 `community-img-sized`（§4） |
| `src/modules/community/ui/markdown-editor.tsx` | 工具栏「图片」动作插入 `![图片描述|300](https://)`（§5） |

## 7. 验证

- `pnpm type-check` 通过（`image` renderer 签名对齐 marked v18 `Tokens.Image`：`title` 为 `string \| null`）。
- 正则用例（node 验证）：`image|690x462` / `image|690` / `图 片|100X80` 均正确解析；`image` / `photo|x` / `img|abc` 回退默认渲染。
- 布局验证标准：900×700 窗口内帖子正文与评论区图片均不溢出；显式尺寸在评论区不缩为 1/3。

## 8. 演进方向（后续可选）

- 尺寸单位扩展（`em` / `%`）、拖拽调尺寸。
- 设定尺寸参与 Lightbox 初始缩放。
- 全局渲染器同步该能力（若其他场景有同样诉求）。
