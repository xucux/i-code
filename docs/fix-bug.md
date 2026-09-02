 修复 Radix ScrollArea Viewport 内部 display:table 包裹层问题

```css
 /* 在你的全局样式或组件样式中 */
[data-radix-scroll-area-viewport] > div {
  display: block !important;
}
```

---

## 2026-09-02 视觉生成页复制图片报错 `os error 1418`

**现象**：在 `/vision` 页点「复制图片」，toast 报：

```
写入剪贴板失败：Unknown error while interacting with the clipboard:
SetClipboardData failed with error: 线程没有打开的剪贴板。 (os error 1418)
```

偶发，arboard 内部 5 次重试也无法覆盖。

**根因**：`media_asset_copy` 用 `tauri::async_runtime::spawn_blocking` 把
`arboard::Clipboard::set_image` 挪到 tokio blocking 线程上跑。但 arboard
在 Windows 上的 `Set::image` 流程是：

1. `OpenClipboard(NULL)` —— 所有者为 NULL；
2. `EmptyClipboard()` —— 所有者仍为 NULL；
3. `SetClipboardData(PNG)` / `SetClipboardData(CF_DIBV5)`。

`spawn_blocking` 的 worker 线程没有消息泵且不是 OLE STA，剪贴板 hook
（Win+V 历史 / PowerToys / 输入法 / Office 监听）在 `EmptyClipboard`
之后会借走当前线程的 `OpenClipboard` 上下文；等 `SetClipboardData` 调用时
已不是当前线程的打开状态 → `ERROR_CLIPBOARD_NOT_OPEN` (1418)。

arboard 自身只在「打开失败」时重试，且每次都走「无消息泵 + 无 owner」
路径，所以重试无效。

**修复**（`src-tauri/src/modules/media_generation/commands.rs`）：

Windows 平台绕开 arboard，直接调 Win32 API：

- `OleInitialize` —— 让 spawn_blocking 线程成为 OLE STA，使 OLE 拦截器
  在该线程上有正常 apartment；
- 注册临时窗口类 + `CreateWindowExW(HWND_MESSAGE)` 创建 message-only
  窗口作为 `OpenClipboard` 的 owner，让 `EmptyClipboard` 触发的
  `WM_DESTROYCLIPBOARD` 等消息有归宿；
- 重试循环里：每次写入前 `PeekMessage(PM_REMOVE)` 处理累积消息 →
  `OpenClipboard(hwnd)` → `EmptyClipboard` → `SetClipboardData(PNG)` →
  `SetClipboardData(CF_DIBV5)` → `CloseClipboard`，单步失败即跳出本轮；
- 失败重试 10 次，退避 20/40/60... ms，总上限 < 1.1s；
- `DestroyWindow` + `OleUninitialize` 清理。

非 Windows 平台沿用 arboard 原路径。

`Cargo.toml` 在 `[target.'cfg(windows)'.dependencies]` 下追加
`windows-sys = "0.61"`，features 覆盖 `Win32_Foundation / Win32_System_DataExchange
/ Win32_System_Memory / Win32_System_Ole / Win32_Graphics_Gdi
/ Win32_UI_WindowsAndMessaging`。版本与 arboard 传递依赖一致，
避免重复引入。
