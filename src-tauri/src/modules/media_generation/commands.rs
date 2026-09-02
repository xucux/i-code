//! # 媒体生成模块 Commands
//!
//! 命令名按 `模块_动作` 规范（snake_case）命名，在 `main.rs` 的 `invoke_handler` 注册。

use tauri::State;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::media_generation::service::MediaGenerationService;
use crate::modules::media_generation::types::{GenerateImageInput, MediaGeneration};

use base64::Engine as _;

/// 生成图像（供应商直连）
///
/// 流程：校验视觉生成供应商 → 解析认证 → 调用上游 images/generations →
/// 产物下载到本地 → 写入生成历史与调用统计。
#[tauri::command]
pub async fn media_generate_image(
    ai_gateway: State<'_, AiGatewayServiceHandle>,
    input: GenerateImageInput,
) -> IcodeResult<MediaGeneration> {
    MediaGenerationService::new()
        .generate_image(ai_gateway.service(), input)
        .await
}

/// 列出图像生成历史（按创建时间倒序）
///
/// `limit` 限制返回条数（缺省 200）。
#[tauri::command]
pub async fn media_history_list(
    limit: Option<i64>,
) -> IcodeResult<Vec<MediaGeneration>> {
    MediaGenerationService::new().list_history(limit)
}

/// 删除一条图像生成历史（同时清理本地产物文件）
#[tauri::command]
pub async fn media_history_delete(id: String) -> IcodeResult<()> {
    MediaGenerationService::new().delete_history(&id)
}

/// 读取媒体产物内容
///
/// 返回 Base64 编码的图片字节，前端拼装为 data URL 展示。
/// 使用 Base64 而非 asset protocol，避免额外配置资产协议作用域。
#[tauri::command]
pub async fn media_asset_read(relative_path: String) -> IcodeResult<String> {
    let bytes = super::asset_store::read_asset(&relative_path)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// 复制媒体产物图片到系统剪贴板（位图）
///
/// 平台策略：
/// - Windows：绕开 arboard，直接调 Win32 API（OleInitialize + 临时 message-only 窗口
///   作为 owner + PeekMessage 让出消息队列），避免 spawn_blocking 线程上 arboard
///   路径偶发的 `SetClipboardData` `ERROR_CLIPBOARD_NOT_OPEN` (os error 1418)。
/// - 其他平台：沿用 arboard。
///
/// 历史背景：arboard 在 Windows 上以 `OpenClipboard(NULL)` 打开剪贴板，所有者为 NULL；
/// `EmptyClipboard` 后所有者仍为 NULL，`WM_DESTROYCLIPBOARD` 等剪贴板消息没有可靠归宿。
/// spawn_blocking 线程没有消息泵，OLE 剪贴板 hook（Win+V 历史 / PowerToys / 输入法等）
/// 在 `EmptyClipboard` 之后会「借走」当前线程的 `OpenClipboard` 上下文，等
/// `SetClipboardData` 调用时已不是当前线程的打开状态 → 1418。
/// 当前重写方案显式建立 message-only owner 窗口，并在每个 API 调用之间 `PeekMessage`
/// 处理消息队列，让 hook 能稳定完成回调；并 `OleInitialize` 让线程成为 STA，
/// 使 OLE 拦截器在该线程上有正常的 apartment 上下文。
#[tauri::command]
pub async fn media_asset_copy(relative_path: String) -> IcodeResult<()> {
    tauri::async_runtime::spawn_blocking(move || copy_image_to_clipboard(&relative_path))
        .await
        .map_err(|e| IcodeError::internal(format!("剪贴板写入任务异常退出：{e}")))?
}

/// 图片解码 + RGBA 提取（共享于各平台的剪贴板写入路径）
fn decode_rgba(relative_path: &str) -> IcodeResult<(Vec<u8>, u32, u32)> {
    let bytes = super::asset_store::read_asset(relative_path)?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| IcodeError::internal(format!("图片解码失败：{e}")))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width, height))
}

#[cfg(target_os = "windows")]
fn copy_image_to_clipboard(relative_path: &str) -> IcodeResult<()> {
    let (rgba, width, height) = decode_rgba(relative_path)?;
    windows_clipboard::write_image(&rgba, width, height)
}

#[cfg(not(target_os = "windows"))]
fn copy_image_to_clipboard(relative_path: &str) -> IcodeResult<()> {
    use std::borrow::Cow;

    let (rgba, width, height) = decode_rgba(relative_path)?;
    let image_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(rgba),
    };

    // arboard 只在「打开剪贴板」失败（ClipboardOccupied）时内部重试，
    // SetClipboardData 阶段的瞬时失败需要外层对整个「打开 → 写入 → 关闭」周期重试。
    const MAX_ATTEMPTS: usize = 5;
    let mut last_err: Option<arboard::Error> = None;
    for attempt in 0..MAX_ATTEMPTS {
        let result = arboard::Clipboard::new()
            .and_then(|mut cb| cb.set_image(image_data.clone()));
        match result {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(
                    25u64 * (attempt as u64 + 1),
                ));
            }
        }
    }
    Err(IcodeError::internal(format!(
        "写入剪贴板失败：{}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "未知错误".to_string())
    )))
}

/// 导出媒体产物图片：弹出系统「另存为」对话框后复制文件到所选位置
///
/// 同步 Command（主线程执行）：rfd 原生对话框要求在主线程调用。
/// 返回保存路径；用户取消对话框时返回 None（前端静默处理，不提示错误）。
#[tauri::command]
pub fn media_asset_export(
    relative_path: String,
    suggested_name: Option<String>,
) -> IcodeResult<Option<String>> {
    let abs = super::asset_store::absolute_path(&relative_path)?;
    let default_name = suggested_name.unwrap_or_else(|| {
        relative_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("image.png")
            .to_string()
    });

    let target = rfd::FileDialog::new()
        .set_title("保存图片")
        .set_file_name(&default_name)
        .save_file();

    match target {
        Some(path) => {
            std::fs::copy(&abs, &path)
                .map_err(|e| IcodeError::internal(format!("保存图片失败：{e}")))?;
            Ok(Some(path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

#[cfg(target_os = "windows")]
mod windows_clipboard {
    use crate::error::{IcodeError, IcodeResult};
    use std::ptr;
    use windows_sys::Win32::Foundation::{
        GetLastError, GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM,
    };
    use windows_sys::Win32::Graphics::Gdi::{BITMAPV5HEADER, BI_BITFIELDS, LCS_GM_IMAGES};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::{CF_DIBV5, OleInitialize, OleUninitialize};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
        RegisterClassExW, TranslateMessage, HWND_MESSAGE, MSG, PM_REMOVE, WNDCLASSEXW,
    };

    /// windows-sys 未暴露的 `LCS_sRGB` 常量
    /// <https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header>
    const LCS_SRGB: u32 = 0x7352_4742;

    /// 全局内存分配标志：可移动、可丢弃（Windows 剪贴板要求 `GMEM_MOVEABLE`）
    const GMEM_MOVEABLE: u32 = 0x0002;

    /// 默认窗口过程：仅转交 `DefWindowProcW`，处理 `WM_DESTROYCLIPBOARD` 等剪贴板
    /// 系统消息，让 OLE hook 能在当前线程的 message-only owner 窗口上完成回调。
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn io_error(code: u32) -> std::io::Error {
        std::io::Error::from_raw_os_error(code as i32)
    }

    /// 在当前线程上把 RGBA 图片写入剪贴板。
    ///
    /// 完整流程：
    /// 1. `OleInitialize` 让当前 spawn_blocking worker 线程成为 OLE STA；
    /// 2. 注册临时窗口类并 `CreateWindowExW(HWND_MESSAGE)` 创建 message-only 窗口
    ///    作为剪贴板 owner（让 `EmptyClipboard` 触发的 `WM_DESTROYCLIPBOARD` 等消息有
    ///    归宿，而不是依赖系统为 `OpenClipboard(NULL)` 走临时窗口路径）；
    /// 3. 重试循环：每次写入前 `PeekMessage` 处理累积消息 → `OpenClipboard(hwnd)` →
    ///    `EmptyClipboard` → `SetClipboardData(PNG)` → `SetClipboardData(CF_DIBV5)` →
    ///    `CloseClipboard`，每个 API 调用失败即跳出本轮重试；
    /// 4. `DestroyWindow` + `OleUninitialize` 清理。
    pub fn write_image(rgba: &[u8], width: u32, height: u32) -> IcodeResult<()> {
        unsafe {
            // S_OK(0) / S_FALSE(1) 都算成功（S_FALSE 表示已初始化）；HRESULT < 0 失败
            let hr = OleInitialize(ptr::null());
            if hr < 0 {
                return Err(IcodeError::internal(format!(
                    "OleInitialize 失败：HRESULT=0x{:08X}",
                    hr as u32
                )));
            }

            // 注册临时窗口类（重复注册会失败但不致命）
            let mut class_name_buf: [u16; 16] = [0; 16];
            let class_name = b"i-code-clip\0";
            for (i, &b) in class_name.iter().take(class_name_buf.len()).enumerate() {
                class_name_buf[i] = b as u16;
            }
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: ptr::null_mut(),
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: class_name_buf.as_ptr(),
                hIconSm: ptr::null_mut(),
            };
            let _ = RegisterClassExW(&wc);

            // 创建 message-only 窗口
            let hwnd = CreateWindowExW(
                0,
                class_name_buf.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            );

            let result = if hwnd.is_null() {
                Err(IcodeError::internal(format!(
                    "CreateWindowExW 失败：{}",
                    io_error(GetLastError())
                )))
            } else {
                write_with_retry(hwnd, rgba, width, height)
            };

            if !hwnd.is_null() {
                let _ = DestroyWindow(hwnd);
            }
            OleUninitialize();
            result
        }
    }

    fn write_with_retry(hwnd: HWND, rgba: &[u8], width: u32, height: u32) -> IcodeResult<()> {
        // 预先准备好 DIBV5 + PNG 数据，避免在重试循环里重复编码
        let dibv5 = build_dibv5(rgba, width, height)?;
        let png = encode_png(rgba, width, height)?;
        let png_format = unsafe { register_png_format() };

        const MAX_ATTEMPTS: usize = 10;
        let mut last_error_msg = String::new();

        for attempt in 0..MAX_ATTEMPTS {
            // 写入前 PeekMessage 让出消息队列，处理累积的剪贴板 hook 消息
            unsafe { drain_messages() };

            match unsafe { write_once(hwnd, &dibv5, &png, png_format) } {
                Ok(()) => return Ok(()),
                Err(msg) => {
                    last_error_msg = msg;
                    // 退避：20 / 40 / 60 / ... ms，总上限 < 1.1s
                    let backoff = 20u64 * (attempt as u64 + 1);
                    std::thread::sleep(std::time::Duration::from_millis(backoff));
                }
            }
        }

        Err(IcodeError::internal(format!(
            "写入剪贴板失败（重试 {} 次后放弃）：{}",
            MAX_ATTEMPTS, last_error_msg
        )))
    }

    /// 单次「打开 → 清空 → 写 PNG → 写 DIBV5 → 关闭」周期
    unsafe fn write_once(
        hwnd: HWND,
        dibv5: &[u8],
        png: &[u8],
        png_format: Option<u32>,
    ) -> Result<(), String> {
        unsafe {
            if OpenClipboard(hwnd) == 0 {
                return Err(format!("OpenClipboard 失败：{}", io_error(GetLastError())));
            }

            if EmptyClipboard() == 0 {
                let code = GetLastError();
                let _ = CloseClipboard();
                return Err(format!("EmptyClipboard 失败：{}", io_error(code)));
            }

            // PNG 在 Windows 上兼容性更好，按 arboard 的顺序优先写
            if let Some(fmt) = png_format {
                if let Err(e) = set_format(fmt, png) {
                    let _ = CloseClipboard();
                    return Err(format!("SetClipboardData(PNG) 失败：{e}"));
                }
            }

            // CF_DIBV5 兼容老式粘贴目标（Office / Word / 画图等）
            if let Err(e) = set_format(CF_DIBV5 as u32, dibv5) {
                let _ = CloseClipboard();
                return Err(format!("SetClipboardData(CF_DIBV5) 失败：{e}"));
            }

            if CloseClipboard() == 0 {
                // 关闭失败不致命（系统最终会关），仅记日志
                log::warn!("CloseClipboard 失败：{}", io_error(GetLastError()));
            }
            Ok(())
        }
    }

    /// `SetClipboardData(format, hglobal)`，失败时回收 `hglobal`
    unsafe fn set_format(format: u32, data: &[u8]) -> Result<(), String> {
        unsafe {
            let h = alloc_global(data)?;
            let ret = SetClipboardData(format, h as HANDLE);
            if ret.is_null() {
                let _ = GlobalFree(h);
                Err(io_error(GetLastError()).to_string())
            } else {
                // 成功，剪贴板接管内存所有权
                Ok(())
            }
        }
    }

    /// `GlobalAlloc(GMEM_MOVEABLE, n)` → `GlobalLock` → memcpy → `GlobalUnlock`
    unsafe fn alloc_global(data: &[u8]) -> Result<HGLOBAL, String> {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, data.len());
            if h.is_null() {
                return Err(format!("GlobalAlloc 失败：{}", io_error(GetLastError())));
            }
            let p = GlobalLock(h);
            if p.is_null() {
                let _ = GlobalFree(h);
                return Err(format!("GlobalLock 失败：{}", io_error(GetLastError())));
            }
            ptr::copy_nonoverlapping(data.as_ptr(), p as *mut u8, data.len());
            let _ = GlobalUnlock(h);
            Ok(h)
        }
    }

    /// 注册 "PNG" 剪贴板格式，失败返回 `None`（PNG 是可选格式）
    unsafe fn register_png_format() -> Option<u32> {
        // "PNG\0" 宽字符
        let name: [u16; 4] = [b'P' as u16, b'N' as u16, b'G' as u16, 0];
        let id = unsafe { RegisterClipboardFormatW(name.as_ptr()) };
        if id == 0 {
            None
        } else {
            Some(id)
        }
    }

    /// 处理当前线程消息队列中的消息，让出 CPU 给剪贴板 hook / OLE apartment
    unsafe fn drain_messages() {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            let mut count = 0u32;
            // 上限 32 条，避免极端情况下消息洪水阻塞写入流程
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 && count < 32 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                count += 1;
            }
        }
    }

    /// 构造 CF_DIBV5 数据：`BITMAPV5HEADER` + 自底向上 BGRA 像素
    ///
    /// 自底向上（最后一行数据在前）+ BGRA 字节序是 Windows DIB 的强制要求；
    /// 使用正 height 表示自底向上布局（Word 等老软件不支持负 height）。
    fn build_dibv5(rgba: &[u8], width: u32, height: u32) -> IcodeResult<Vec<u8>> {
        let header_size = std::mem::size_of::<BITMAPV5HEADER>();
        let pixel_size = (width as usize) * (height as usize) * 4;
        let mut out = Vec::with_capacity(header_size + pixel_size);

        let header = BITMAPV5HEADER {
            bV5Size: header_size as u32,
            bV5Width: width as i32,
            bV5Height: height as i32,
            bV5Planes: 1,
            bV5BitCount: 32,
            bV5Compression: BI_BITFIELDS,
            bV5SizeImage: (4 * width * height) as u32,
            bV5XPelsPerMeter: 0,
            bV5YPelsPerMeter: 0,
            bV5ClrUsed: 0,
            bV5ClrImportant: 0,
            bV5RedMask: 0x00ff0000,
            bV5GreenMask: 0x0000ff00,
            bV5BlueMask: 0x000000ff,
            bV5AlphaMask: 0xff000000,
            bV5CSType: LCS_SRGB,
            // SAFETY: 当 bV5CSType != LCS_CALIBRATED_RGB 时 Windows 忽略此字段
            bV5Endpoints: unsafe { std::mem::zeroed() },
            bV5GammaRed: 0,
            bV5GammaGreen: 0,
            bV5GammaBlue: 0,
            bV5Intent: LCS_GM_IMAGES as u32,
            bV5ProfileData: 0,
            bV5ProfileSize: 0,
            bV5Reserved: 0,
        };

        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const BITMAPV5HEADER as *const u8,
                header_size,
            );
            out.extend_from_slice(header_bytes);
        }

        // 自底向上 + BGRA 字节序
        let row_size = width as usize * 4;
        let height_us = height as usize;
        for row in (0..height_us).rev() {
            let start = row * row_size;
            let row_data = &rgba[start..start + row_size];
            for chunk in row_data.chunks_exact(4) {
                out.push(chunk[2]); // B
                out.push(chunk[1]); // G
                out.push(chunk[0]); // R
                out.push(chunk[3]); // A
            }
        }

        Ok(out)
    }

    /// PNG 编码（兼容性更好的剪贴板格式，部分粘贴目标仅认 PNG）
    fn encode_png(rgba: &[u8], width: u32, height: u32) -> IcodeResult<Vec<u8>> {
        use image::codecs::png::PngEncoder;
        use image::ExtendedColorType;
        use image::ImageEncoder;
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(rgba, width, height, ExtendedColorType::Rgba8)
            .map_err(|e| IcodeError::internal(format!("PNG 编码失败：{e}")))?;
        Ok(buf)
    }
}
