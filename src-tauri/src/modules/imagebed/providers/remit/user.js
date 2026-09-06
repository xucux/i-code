// i-code 图床助手（img.remit.ee）
// ============================================================================
// 由篡改猴脚本《img.remit.ee 图床助手（去广告 + 粘贴上传 + 一键复制外链）》v1.2.0
// 适配而来，作为 Tauri WebviewWindowBuilder::initialization_script 注入（document-start）。
//
// 适配改动：
// 1. 删除 Tampermonkey metadata 头（==UserScript== / @grant / @match / @run-at），
//    保留 'use strict' 与外层 IIFE。
// 2. GM_setClipboard 分支无需处理：copyText 已做 typeof 判断，Tauri 中自动落到
//    navigator.clipboard.writeText → execCommand fallback，天然兼容。
// 3. 新增 document.title 桥接（reportToBridge）：点击复制按钮成功后，把外链
//    markdown 写入 document.title = "ICODE_IMGBED:<markdown>"，由 i-code 主程序
//    轮询窗口标题解析回传，实现「一键插入社区编辑器」；超长时只写 LONG 标记，
//    完整外链仍走剪贴板兜底。
// ============================================================================

(function () {
  'use strict';

  /* ==============================
   * 〇·零、目标站点守卫（等价于篡改猴 @match）
   * 只有 img.remit.ee 才运行本脚本，其他页面（如新标签、错误页）直接退出，
   * 避免注入到不匹配站点后报错刷屏。
   * ============================== */

  if (!/^https:\/\/img\.remit\.ee([/?#]|$)/.test(location.href)) return;

  /* ==============================
   * 〇、彩色控制台日志
   * ============================== */

  const VERSION = '1.2.0-icode';

  const LOG = (() => {
    const TAG = 'IMG.REMIT.EE 助手';

    // 各片段的 CSS（支持 %c 的控制台都会生效）
    const C = {
      tag: 'background:linear-gradient(90deg,#2563eb,#7c3aed);color:#fff;font-weight:700;padding:2px 6px;border-radius:3px',
      time: 'color:#94a3b8',
      reset: 'color:inherit;font-weight:normal',
      text: 'color:#1f2937;font-weight:600',
      ok: 'background:#d1fae5;color:#047857;font-weight:700;padding:1px 5px;border-radius:3px',
      block: 'background:#fee2e2;color:#b91c1c;font-weight:700;padding:1px 5px;border-radius:3px',
      copy: 'background:#ede9fe;color:#6d28d9;font-weight:700;padding:1px 5px;border-radius:3px',
      warn: 'background:#fef3c7;color:#b45309;font-weight:700;padding:1px 5px;border-radius:3px',
      err: 'background:#fecaca;color:#991b1b;font-weight:700;padding:1px 5px;border-radius:3px',
      info: 'background:#dbeafe;color:#1d4ed8;font-weight:700;padding:1px 5px;border-radius:3px',
      mono: 'font-family:ui-monospace,Consolas,"Courier New",monospace;color:#334155',
      dim: 'color:#94a3b8',
      link: 'color:#0d9488;font-weight:600;text-decoration:underline',
    };

    // 节流：同一种日志在 quietMs 内只打印一次，避免 MutationObserver 刷屏
    const throttles = new Map();
    function throttled(key, quietMs) {
      const now = Date.now();
      const last = throttles.get(key) || 0;
      if (now - last < quietMs) return false;
      throttles.set(key, now);
      return true;
    }

    function stamp() {
      const d = new Date();
      return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}:${String(d.getSeconds()).padStart(2, '0')}.${String(d.getMilliseconds()).padStart(3, '0')}`;
    }

    /**
     * @param {'INFO'|'OK'|'BLOCK'|'COPY'|'WARN'|'ERR'} kind
     * @param {Array<string|{text:string,style:string}>} parts
     */
    function emit(kind, kindStyle, parts) {
      let fmt = `%c${TAG}%c %c${kind}%c %c${stamp()}%c`;
      const args = [C.tag, C.reset, kindStyle, C.reset, C.time, C.reset];
      for (const p of parts) {
        if (p && typeof p === 'object') {
          fmt += ` %c${p.text}%c`;
          args.push(p.style, C.reset);
        } else {
          fmt += ` %c${p}%c`;
          args.push(C.text, C.reset);
        }
      }
      console.log(fmt, ...args);
    }

    const t = (s, style) => ({ text: s, style });

    return {
      banner() {
        console.log(
          `%c\n ██  img.remit.ee 图床助手 v${VERSION}（i-code 内置）  ██ \n`,
          'background:linear-gradient(90deg,#2563eb,#7c3aed,#0d9488);color:#fff;font-weight:800;padding:6px 14px;border-radius:6px;font-size:12px'
        );
        emit('INFO', C.info, [
          '已启动 · 目标站点',
          t(location.origin, C.link),
        ]);
        emit('INFO', C.info, [
          '能力',
          t('屏蔽广告', C.block),
          t('屏蔽 fc- 解锁弹窗', C.block),
          t('粘贴图片预览 + 一键上传', C.copy),
          t('上传后复制 Markdown / HTML / 直链', C.copy),
        ]);
      },
      styleInjected() {
        emit('OK', C.ok, ['屏蔽样式已注入', t(`(${HIDE_CSS.length} 字节)`, C.dim)]);
      },
      blocked(selector, count) {
        if (!throttled(`block:${selector}`, 1000)) {
          STATS.blockedSilently += count;
          return;
        }
        emit('BLOCK', C.block, [
          '已拦截并移除',
          t(`×${count}`, C.mono),
          t(selector, C.mono),
        ]);
      },
      watchStarted(target) {
        emit('OK', C.ok, ['DOM 监听已启动', t(target, C.dim)]);
      },
      buttonsInjected(name, url) {
        emit('OK', C.ok, [
          '已注入复制按钮',
          t(name, C.mono),
          '→',
          t(url, C.link),
        ]);
      },
      cardPending(reason) {
        if (!throttled(`pending:${reason}`, 3000)) return;
        emit('INFO', C.info, ['卡片尚未就绪，等待下次扫描：', t(reason, C.dim)]);
      },
      copied(label, text) {
        emit('COPY', C.copy, [
          `已复制 ${label}`,
          t(text.length > 120 ? text.slice(0, 120) + '…' : text, C.mono),
        ]);
      },
      copyFailed(label) {
        emit('ERR', C.err, ['复制失败，请手动复制：', t(label, C.mono)]);
      },
      pasteCaptured(count) {
        emit('COPY', C.copy, [
          '已捕获剪贴板图片',
          t(`×${count}`, C.mono),
          t('已加入左侧面板', C.dim),
        ]);
      },
      panelToggle(open) {
        emit('INFO', C.info, [open ? '浮动粘贴面板已展开' : '浮动粘贴面板已折叠']);
      },
      panelUploadStart(count) {
        emit('OK', C.ok, [
          '已注入站点上传通道',
          t(`×${count}`, C.mono),
          t('等待站点生成卡片…', C.dim),
        ]);
      },
      panelUploadDone(count) {
        emit('OK', C.ok, ['上传成功', t(`×${count}`, C.mono), t('卡片已生成', C.dim)]);
      },
      panelUploadFail() {
        emit('WARN', C.warn, ['未检测到结果卡片，上传可能失败，可点击重试']);
      },
      bridgeReported(text) {
        emit('COPY', C.copy, [
          '已上报 i-code 桥接',
          t(text.length > 120 ? text.slice(0, 120) + '…' : text, C.mono),
        ]);
      },
      summary() {
        console.groupCollapsed(
          `%c${TAG}%c %c运行统计%c 页面可见期间共执行 ${STATS.sweeps} 次扫描`,
          C.tag, C.reset, C.info, C.reset
        );
        console.table({
          拦截广告或弹窗: STATS.blocked,
          注入复制按钮: STATS.buttons,
          粘贴图片数: STATS.pasteCount,
          上传成功张数: STATS.uploadsDone,
          复制次数: STATS.copies,
          扫描次数: STATS.sweeps,
        });
        console.log('%c提示：可随时在控制台执行 REMIT_HELPER.stats() 查看实时数据', C.dim);
        console.groupEnd();
      },
    };
  })();

  // 运行统计
  const STATS = {
    blocked: 0,
    blockedSilently: 0,
    buttons: 0,
    copies: 0,
    sweeps: 0,
    pasteCount: 0,
    uploadsSubmitted: 0,
    uploadsDone: 0,
  };

  /* ==============================
   * 〇·五、i-code 应用内桥接（document.title 上报）
   * 点击复制按钮成功后，把外链 markdown 写入 document.title，
   * 由 i-code 主程序轮询图床窗口标题解析回传，实现「一键插入社区编辑器」。
   * ============================== */

  const BRIDGE_PREFIX = 'ICODE_IMGBED:';
  // title 上限保护：超过该长度只写确认标记，完整外链仍走剪贴板兜底
  const BRIDGE_MAX_LEN = 900;

  function reportToBridge(text) {
    try {
      const t = String(text);
      document.title = BRIDGE_PREFIX + (t.length <= BRIDGE_MAX_LEN ? t : 'LONG');
      LOG.bridgeReported(t);
    } catch (e) { /* 抑制：标题写入失败不影响剪贴板复制 */ }
  }

  /* ==============================
   * 一、CSS 层面屏蔽广告与弹窗
   * ============================== */

  const HIDE_CSS = `
    /* Google AdSense 贴片/插页广告（含 #google_vignette） */
    #google_vignette,
    #google_image_div,
    #google_ads,
    ins.adsbygoogle,
    ins[id^="aswift_"],
    iframe[id^="aswift_"],
    iframe[id^="google_ads_"],
    iframe[src*="googlesyndication.com"],
    iframe[src*="doubleclick.net"],
    iframe[src*="google.com/ads"],
    [id^="div-gpt-ad"],
    [id^="google_ads_iframe"],
    .adsbygoogle,
    .ad-container,
    .advertisement,
    /* 解锁/付费/反爬弹窗（fc- 前缀的 overlay 全家桶） */
    .fc-monetization-dialog,
    .fc-dialog,
    .fc-dialog-overlay,
    .fc-dialog-container,
    .fc-dialog-content,
    .fc-message-root,
    .fc-message-container,
    .fc-message-overlay,
    .fc-message-header,
    .fc-message-content,
    .fc-container,
    .fc-overlay,
    .fc-cta-root,
    .fc-cta-container,
    .fc-ab-root,
    .fc-ab-container,
    .fc-whitelist-root,
    div[class^="fc-"],
    div[id^="fc-"],
    /* 通用兜底 */
    [role="dialog"][aria-label="解锁更多内容"],
    [role="dialog"][aria-label*="解锁"] {
      display: none !important;
      visibility: hidden !important;
      opacity: 0 !important;
      pointer-events: none !important;
    }

    /* 贴片广告弹出时页面会被锁定滚动，这里强制解锁 */
    html, body {
      overflow: auto !important;
      position: static !important;
    }
  `;

  function injectStyle() {
    // document-start 注入过早时 head/html 尚未建立，由 boot 的 DOMContentLoaded 时机保证重试
    const host = document.head || document.documentElement;
    if (!host) return;
    const style = document.createElement('style');
    style.setAttribute('data-userscript', 'img-remit-helper');
    style.textContent = HIDE_CSS;
    host.appendChild(style);
    LOG.styleInjected();
  }

  /* ==============================
   * 二、JS 层面兜底删除广告节点
   *（有些广告节点是 JS 晚插入的，删除比隐藏更彻底，防止残留遮罩层挡住点击）
   * ============================== */

  const AD_SELECTORS = [
    '#google_vignette',
    'ins.adsbygoogle',
    'iframe[id^="aswift_"]',
    'iframe[id^="google_ads_"]',
    'iframe[src*="googlesyndication.com"]',
    'iframe[src*="doubleclick.net"]',
    '[id^="div-gpt-ad"]',
    '[id^="google_ads_iframe"]',
    '.fc-monetization-dialog',
    '.fc-dialog',
    '.fc-message-root',
    '.fc-message-container',
    '.fc-container',
    '.fc-ab-root',
    'div[class^="fc-"]',
    'div[id^="fc-"]',
    '[role="dialog"][aria-label="解锁更多内容"]',
  ];

  function isAdNode(node) {
    if (node.nodeType !== 1) return false; // 仅元素节点
    return AD_SELECTORS.some((sel) => {
      try { return node.matches(sel); } catch (e) { return false; }
    });
  }

  function purgeAds(root) {
    if (!root || root.nodeType !== 1) return;
    // 根节点本身是广告
    if (isAdNode(root)) {
      root.remove();
      STATS.blocked += 1;
      LOG.blocked(root.tagName.toLowerCase() + (root.id ? '#' + root.id : '.' + String(root.className).split(' ')[0]), 1);
      return;
    }
    for (const sel of AD_SELECTORS) {
      let nodes = [];
      try { nodes = root.querySelectorAll(sel); } catch (e) {}
      if (!nodes.length) continue;
      nodes.forEach((n) => n.remove());
      STATS.blocked += nodes.length;
      LOG.blocked(sel, nodes.length);
    }
  }

  /* ==============================
   * 三、上传结果卡片：注入复制按钮
   * ============================== */

  // 该站使用 CSS Modules（类名带哈希，如 Home-module__g21JLG__fileCard），
  // 构建后哈希可能变化，因此统一用 [class*="xxx"] 模糊匹配，更抗更新。
  const SEL = {
    resultsSection: '[class*="resultsSection"]',
    fileGrid: '[class*="fileGrid"]',
    fileCard: '[class*="fileCard"]',
    imagePreview: '[class*="imagePreview"]',
    fileInfo: '[class*="fileInfo"]',
    fileName: '[class*="fileName"]',
  };

  function copyText(text) {
    if (typeof GM_setClipboard === 'function') {
      GM_setClipboard(text, 'text');
      return Promise.resolve(true);
    }
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text).then(() => true).catch(() => fallbackCopy(text));
    }
    return Promise.resolve(fallbackCopy(text));
  }

  function fallbackCopy(text) {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.cssText = 'position:fixed;left:-9999px;top:0;opacity:0;';
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    let ok = false;
    try { ok = document.execCommand('copy'); } catch (e) {}
    ta.remove();
    return ok;
  }

  function toast(msg, ok) {
    const tip = document.createElement('div');
    tip.textContent = msg;
    tip.style.cssText = [
      'position:fixed', 'top:24px', 'left:50%', 'transform:translateX(-50%)',
      'z-index:2147483647', 'padding:8px 18px', 'border-radius:8px',
      'font-size:13px', 'color:#fff', 'pointer-events:none',
      'box-shadow:0 4px 14px rgba(0,0,0,.25)', 'transition:opacity .3s',
      ok === false ? 'background:#c0392b' : 'background:#1f883d',
    ].join(';');
    document.body.appendChild(tip);
    setTimeout(() => { tip.style.opacity = '0'; }, 1200);
    setTimeout(() => tip.remove(), 1600);
  }

  function buildButton(label, getText, accent) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = label;
    btn.style.cssText = [
      'display:inline-flex', 'align-items:center', 'gap:4px',
      'padding:6px 10px', 'margin:2px 6px 2px 0', 'border:none',
      'border-radius:6px', 'font-size:12px', 'font-weight:600',
      'cursor:pointer', 'color:#fff', 'line-height:1',
      'transition:filter .15s, transform .05s',
      `background:${accent}`,
    ].join(';');
    btn.addEventListener('mouseenter', () => (btn.style.filter = 'brightness(1.1)'));
    btn.addEventListener('mouseleave', () => (btn.style.filter = 'none'));
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      const text = getText();
      copyText(text).then((ok) => {
        const old = btn.textContent;
        btn.textContent = ok ? '已复制 ✓' : '复制失败';
        if (ok) {
          STATS.copies += 1;
          LOG.copied(label, text);
          // i-code 桥接：复制成功同时上报外链，主程序自动插入社区编辑器
          reportToBridge(text);
        } else {
          LOG.copyFailed(label);
        }
        toast(ok ? `已复制：${label}` : '复制失败，请手动复制', ok);
        setTimeout(() => (btn.textContent = old), 1200);
      });
    });
    return btn;
  }

  function injectButtons() {
    const cards = document.querySelectorAll(SEL.fileCard);
    for (const card of cards) {
      if (card.dataset.remitHelperDone === '1') continue;

      const img = card.querySelector(`${SEL.imagePreview} img`);
      // src 还没就绪（上传中/懒加载）就先跳过，等下次扫描再处理
      if (!img || !img.src || !/^https?:\/\//.test(img.src)) {
        LOG.cardPending('图片 src 未就绪');
        continue;
      }

      const info = card.querySelector(SEL.fileInfo);
      if (!info) {
        LOG.cardPending('未找到 fileInfo 容器');
        continue;
      }

      let name = '';
      const nameEl = card.querySelector(SEL.fileName);
      if (nameEl) name = nameEl.textContent.trim();
      if (!name && img.alt) name = img.alt.trim();

      const url = new URL(img.src, location.origin).href;

      const row = document.createElement('div');
      row.className = 'remit-helper-copy-row';
      row.style.cssText = [
        'display:flex', 'flex-wrap:wrap', 'align-items:center',
        'margin-top:6px', 'padding-top:6px',
        'border-top:1px dashed rgba(128,128,128,.35)',
      ].join(';');

      row.appendChild(
        buildButton('复制 Markdown', () => `![${name}](${url})`, '#2563eb')
      );
      row.appendChild(
        buildButton('复制 HTML', () => `<img src="${url}" alt="${name}">`, '#7c3aed')
      );
      row.appendChild(
        buildButton('复制直链', () => url, '#0d9488')
      );

      info.appendChild(row);
      card.dataset.remitHelperDone = '1';
      STATS.buttons += 1;
      LOG.buttonsInjected(name || '(未命名)', url);
    }
  }

  /* ==============================
   * 三·五、浮动粘贴面板：剪贴板图片预览 + 一键上传
   * ============================== */

  const PANEL_CSS = `
    .remit-helper-fab {
      position: fixed; left: 0; top: 46%; z-index: 2147483646;
      width: 44px; height: 92px; border: none; cursor: pointer;
      background: linear-gradient(180deg,#2563eb,#7c3aed);
      color: #fff; font-size: 11px; text-align: center; line-height: 1.25;
      border-radius: 0 12px 12px 0; box-shadow: 0 4px 16px rgba(37,99,235,.35);
      padding: 4px 0; font-weight: 700; opacity: .95;
      display: flex; flex-direction: column; align-items: center; gap: 3px;
    }
    .remit-helper-fab:hover { filter: brightness(1.12); }
    .remit-helper-fab svg { width: 18px; height: 18px; }

    .remit-helper-panel {
      position: fixed; left: 12px; top: 50%; transform: translateY(-50%);
      width: 262px; max-height: min(76vh, 620px); overflow-y: auto;
      background: #fff; border: 1px solid #e2e8f0; border-radius: 12px;
      box-shadow: 0 12px 30px rgba(15,23,42,.22); z-index: 2147483646;
      font-size: 12px; color: #334155; display: none;
    }
    .remit-helper-panel-head {
      display: flex; align-items: center; justify-content: space-between;
      padding: 8px 10px; background: linear-gradient(90deg,#2563eb,#7c3aed);
      color: #fff; font-weight: 700; font-size: 12.5px;
      border-radius: 12px 12px 0 0;
    }
    .remit-helper-panel-head button {
      background: transparent; color: #fff; border: 1px solid rgba(255,255,255,.6);
      border-radius: 6px; padding: 1px 8px; font-size: 11px; cursor: pointer;
    }
    .remit-helper-panel-head button:hover { background: rgba(255,255,255,.22); }

    .remit-helper-paste-hint {
      margin: 8px 10px; border: 1.5px dashed #a5b4fc; border-radius: 10px;
      padding: 10px 8px; text-align: center; color: #475569;
      background: #eef2ff; cursor: pointer; outline: none;
    }
    .remit-helper-paste-hint:hover { background: #e0e7ff; border-color: #6366f1; }
    .remit-helper-paste-hint:focus { outline: 2px solid #6366f1; outline-offset: 1px; }

    .remit-helper-items { margin: 0 10px 2px; }
    .remit-helper-items:empty::after {
      content: '暂无图片 —— 粘贴（Ctrl+V）或点击上方提示区聚焦后粘贴';
      display: block; text-align: center; color: #94a3b8;
      padding: 14px 0; font-size: 11px;
    }
    .remit-helper-item {
      display: flex; gap: 8px; align-items: center;
      border: 1px solid #e2e8f0; border-radius: 8px;
      padding: 6px 8px; margin-bottom: 6px; background: #fafbfc;
    }
    .remit-helper-item.is-uploading { border-color: #93b4fd; background: #edf4ff; }
    .remit-helper-item img {
      width: 44px; height: 44px; object-fit: cover;
      border-radius: 6px; background: #f1f5f9;
    }
    .remit-helper-item-info { flex: 1; min-width: 0; overflow: hidden; }
    .remit-helper-item-name {
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
      font-size: 11.5px; color: #1e293b; font-weight: 600;
    }
    .remit-helper-item-size { font-size: 10.5px; color: #94a3b8; }
    .remit-helper-item-ops { display: flex; gap: 4px; flex-shrink: 0; }
    .remit-helper-item-ops button {
      border: none; border-radius: 6px; cursor: pointer;
      font-size: 11px; padding: 3px 8px; line-height: 1;
    }
    .rh-up { background: #2563eb; color: #fff; }
    .rh-up[disabled] { background: #94a3b8; color: #fff; cursor: default; }
    .rh-del { background: #fff; color: #94a3b8; border: 1px solid #e2e8f0; }
    .rh-del:hover { color: #dc2626; border-color: #fca5a5; }
    .remit-helper-item-ops button:not([disabled]):hover { filter: brightness(1.12); }

    .remit-helper-panel-foot { margin: 8px 10px 12px; text-align: center; }
    .remit-helper-panel-foot button {
      width: 100%; padding: 7px 0; border: none; border-radius: 8px;
      background: linear-gradient(90deg,#2563eb,#7c3aed); color: #fff;
      font-size: 12px; font-weight: 700; cursor: pointer;
    }
    .remit-helper-panel-foot button[disabled] { background: #94a3b8; cursor: default; }
  `;

  // ---- 面板运行状态 ----
  const PANEL = {
    fab: null,
    root: null,
    list: null,
    upAllBtn: null,
    opened: false,
    busy: false,
    curIds: [],
    seq: 0,
    items: [], // { id, file, url, row, upBtn }
  };

  function humanSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / 1048576).toFixed(2) + ' MB';
  }

  // 粘贴目标是否落在文本输入框（此时保留页面默认粘贴行为）
  function isTextyTarget(el) {
    if (!el || el.nodeType !== 1) return false;
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') return true;
    if (el.isContentEditable) return true;
    try {
      return !!el.closest('input, textarea, [contenteditable="true"]');
    } catch (e) {
      return false;
    }
  }

  function setPanelOpen(open) {
    PANEL.opened = open;
    PANEL.root.style.display = open ? 'block' : 'none';
    PANEL.fab.style.display = open ? 'none' : 'flex';
    LOG.panelToggle(open);
  }

  function syncUpAll() {
    PANEL.upAllBtn.textContent = PANEL.busy
      ? '正在上传…'
      : (PANEL.items.length ? `全部上传 (${PANEL.items.length})` : '全部上传 (0)');
    PANEL.upAllBtn.disabled = PANEL.busy || !PANEL.items.length;
    for (const it of PANEL.items) {
      it.upBtn.disabled = PANEL.busy;
      it.upBtn.textContent = PANEL.busy ? '上传中…' : '上传';
    }
  }

  function addPanelItem(file) {
    const id = 'rhp' + (++PANEL.seq);
    const url = URL.createObjectURL(file);

    const row = document.createElement('div');
    row.className = 'remit-helper-item';
    row.dataset.id = id;

    const img = document.createElement('img');
    img.src = url;
    img.alt = file.name || 'clipboard';

    const info = document.createElement('div');
    info.className = 'remit-helper-item-info';
    const nameEl = document.createElement('div');
    nameEl.className = 'remit-helper-item-name';
    const label = file.name || '剪贴板图片';
    nameEl.textContent = label;
    nameEl.title = label;
    const sizeEl = document.createElement('div');
    sizeEl.className = 'remit-helper-item-size';
    sizeEl.textContent = humanSize(file.size);

    const ops = document.createElement('div');
    ops.className = 'remit-helper-item-ops';
    const up = document.createElement('button');
    up.type = 'button'; up.className = 'rh-up'; up.textContent = '上传';
    up.addEventListener('click', () => uploadOne(id));
    const del = document.createElement('button');
    del.type = 'button'; del.className = 'rh-del'; del.textContent = '✕';
    del.addEventListener('click', () => removePanelItem(id));

    info.append(nameEl, sizeEl);
    ops.append(up, del);
    row.append(img, info, ops);
    PANEL.list.prepend(row); // 新粘贴的置顶
    PANEL.items.push({ id, file, url, row, upBtn: up });
    syncUpAll();
  }

  function addPanelItems(files) {
    for (const f of files) addPanelItem(f);
    STATS.pasteCount += files.length;
    LOG.pasteCaptured(files.length);
    toast(`已加入 ${files.length} 张图片，点击「上传」提交`, true);
  }

  function removePanelItem(id) {
    const idx = PANEL.items.findIndex((it) => it.id === id);
    if (idx < 0) return;
    const it = PANEL.items[idx];
    PANEL.items.splice(idx, 1);
    it.row.remove();
    try { URL.revokeObjectURL(it.url); } catch (e) {}
    syncUpAll();
  }

  function clearPanelItems() {
    if (PANEL.busy) { toast('上传进行中，稍候再清空', false); return; }
    for (const it of PANEL.items.splice(0)) {
      it.row.remove();
      try { URL.revokeObjectURL(it.url); } catch (e) {}
    }
    syncUpAll();
  }

  // 把 File 列表注入站点自身上传输入框（#file-upload）并触发 change，
  // 复用站点压缩 / 限速重试 / 进度 UI 等完整上传管道。
  function injectIntoUploadInput(files) {
    const input = document.getElementById('file-upload');
    if (!input) {
      toast('未找到上传控件（#file-upload），请刷新页面', false);
      return false;
    }
    if (typeof DataTransfer === 'undefined') {
      toast('当前浏览器不支持 DataTransfer，无法注入上传', false);
      return false;
    }
    let dt = null;
    try {
      dt = new DataTransfer();
      for (const f of files) dt.items.add(f);
      input.files = dt.files; // Chrome 允许对 file input 赋值 DataTransfer 的 FileList
    } catch (e) {
      try {
        // 兜底：定义读取访问器，站点读 e.target.files 时拿到我们的列表
        const fl = dt ? dt.files : null;
        Object.defineProperty(input, 'files', {
          get: () => fl,
          configurable: true,
        });
      } catch (e2) {
        toast('无法注入文件到上传控件', false);
        return false;
      }
    }
    input.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }

  function setBusy(busy) {
    PANEL.busy = busy;
    for (const it of PANEL.items) {
      const isCur = PANEL.curIds.includes(it.id);
      it.upBtn.disabled = busy;
      it.upBtn.textContent = busy ? (isCur ? '上传中…' : '排队') : '上传';
      it.row.classList.toggle('is-uploading', busy && isCur);
    }
    PANEL.upAllBtn.disabled = busy || !PANEL.items.length;
    PANEL.upAllBtn.textContent = busy ? '正在上传…' : (PANEL.items.length ? `全部上传 (${PANEL.items.length})` : '全部上传 (0)');
  }

  // 上传（同一时间只允许一批，避免卡片计数互相干扰）
  function submitUpload(ids) {
    if (PANEL.busy) return;
    const targets = PANEL.items.filter((it) => ids.includes(it.id));
    if (!targets.length) return;

    PANEL.curIds = ids;
    setBusy(true);
    STATS.uploadsSubmitted += 1;
    LOG.panelUploadStart(targets.length);

    const baseline = document.querySelectorAll(SEL.fileCard).length;
    const expected = baseline + targets.length;
    if (!injectIntoUploadInput(targets.map((it) => it.file))) {
      setBusy(false);
      syncUpAll();
      return;
    }

    const begun = Date.now();
    const timer = setInterval(() => {
      const nowCount = document.querySelectorAll(SEL.fileCard).length;
      const timedOut = Date.now() - begun > 30000;
      if (nowCount < expected && !timedOut) return;
      clearInterval(timer);

      if (nowCount >= expected) {
        STATS.uploadsDone += targets.length;
        LOG.panelUploadDone(targets.length);
        toast(`上传成功 ×${targets.length}，卡片与复制按钮已生成`, true);
        for (const id of ids) removePanelItem(id);
      } else {
        LOG.panelUploadFail();
        toast('未检测到结果卡片，上传可能失败，可点击重试', false);
      }
      PANEL.curIds = [];
      setBusy(false);
      syncUpAll();
    }, 800);
  }

  function uploadOne(id) { submitUpload([id]); }

  function uploadAll() { submitUpload(PANEL.items.map((it) => it.id)); }

  function buildPanel() {
    // 折叠态：左侧悬浮入口按钮
    const fab = document.createElement('button');
    fab.type = 'button';
    fab.className = 'remit-helper-fab';
    fab.title = '剪贴板粘贴上传（点击展开面板）';
    fab.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="7" height="9" rx="1"/><rect x="10.5" y="4" width="7" height="9" rx="1"/><rect x="14.5" y="6.5" width="7" height="9" rx="1"/></svg><span>粘贴<br>上传</span>';
    fab.addEventListener('click', () => setPanelOpen(true));
    document.body.appendChild(fab);

    // 面板样式
    const style = document.createElement('style');
    style.setAttribute('data-userscript', 'img-remit-helper-panel');
    style.textContent = PANEL_CSS;
    (document.head || document.documentElement).appendChild(style);

    // 展开态：左侧浮动面板
    const root = document.createElement('aside');
    root.className = 'remit-helper-panel';

    const head = document.createElement('div');
    head.className = 'remit-helper-panel-head';
    const title = document.createElement('span');
    title.textContent = '📋 剪贴板粘贴上传';
    const headBtns = document.createElement('span');
    headBtns.style.cssText = 'display:flex;gap:6px;align-items:center';
    const clearBtn = document.createElement('button');
    clearBtn.type = 'button';
    clearBtn.textContent = '清空';
    clearBtn.addEventListener('click', clearPanelItems);
    const hideBtn = document.createElement('button');
    hideBtn.type = 'button';
    hideBtn.textContent = '折叠';
    hideBtn.addEventListener('click', () => setPanelOpen(false));
    headBtns.append(clearBtn, hideBtn);
    head.append(title, headBtns);

    const hint = document.createElement('div');
    hint.className = 'remit-helper-paste-hint';
    hint.tabIndex = 0;
    hint.textContent = '点击聚焦后 Ctrl+V 粘贴图片（页面任意空白处粘贴亦可）';
    hint.addEventListener('click', () => hint.focus());

    const list = document.createElement('div');
    list.className = 'remit-helper-items';

    const foot = document.createElement('div');
    foot.className = 'remit-helper-panel-foot';
    const upAll = document.createElement('button');
    upAll.type = 'button';
    upAll.textContent = '全部上传 (0)';
    upAll.addEventListener('click', uploadAll);
    foot.appendChild(upAll);

    root.append(head, hint, list, foot);
    document.body.appendChild(root);

    PANEL.fab = fab;
    PANEL.root = root;
    PANEL.list = list;
    PANEL.upAllBtn = upAll;

    // 粘贴拦截：捕获阶段，先于页面自身逻辑；仅拦截图片粘贴
    document.addEventListener('paste', (e) => {
      const target = e.target;
      const inPanel = target && target.closest
        ? target.closest('.remit-helper-panel, .remit-helper-fab')
        : null;
      if (!inPanel && isTextyTarget(target)) return; // 文本框内粘贴保持默认

      const dt = e.dataTransfer || e.clipboardData;
      if (!dt) return;
      let files = null;
      try {
        files = [...dt.items]
          .filter((it) => it.kind === 'file' && it.type.startsWith('image/'))
          .map((it) => (it.getAsFile && it.getAsFile()) || null)
          .filter(Boolean);
      } catch (err) { /* 粘贴内容非文件（文本/HTML 等）时忽略 */ }
      if (!files || !files.length) return;

      e.preventDefault();
      e.stopPropagation();
      if (!PANEL.opened) setPanelOpen(true);
      addPanelItems(files);
    }, true);
  }

  /* ==============================
   * 四、总调度：MutationObserver + 轮询兜底
   * ============================== */

  function sweep(addedNode) {
    STATS.sweeps += 1;
    purgeAds(addedNode);
    injectButtons();
  }

  function startObserver() {
    const target = document.body || document.documentElement;
    const observer = new MutationObserver((mutations) => {
      for (const m of mutations) {
        for (const node of m.addedNodes) {
          if (node.nodeType === 1) sweep(node);
        }
      }
    });
    observer.observe(target, { childList: true, subtree: true });
    LOG.watchStarted(target === document.body ? 'document.body（含子树）' : 'document.documentElement');

    // 初次执行 + 轮询兜底（防止漏网之鱼与懒加载图片）
    sweep(document.documentElement);
    let ticks = 0;
    const timer = setInterval(() => {
      sweep(document.documentElement);
      if (++ticks > 40) clearInterval(timer); // 约 20 秒后停止轮询，之后全靠 observer
    }, 500);
    // 页面切回前台时再补一次
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) sweep(document.documentElement);
    });
    // 离开页面时打印一次统计
    window.addEventListener('pagehide', LOG.summary);
  }

  function boot() {
    LOG.banner();
    // Tauri initialization_script 在 document-start 注入，DOM 尚未就绪，
    // 统一挂到 DOMContentLoaded 后再注入样式 / 监听 / 面板，避免空 DOM 崩溃。
    const ready = () => {
      injectStyle();
      startObserver();
      buildPanel();
    };
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', ready, { once: true });
    } else {
      ready();
    }
  }

  // 暴露到全局，方便在控制台手动查看统计
  window.REMIT_HELPER = {
    version: VERSION,
    stats: () => ({
      拦截广告或弹窗: STATS.blocked,
      注入复制按钮: STATS.buttons,
      粘贴图片数: STATS.pasteCount,
      提交上传次数: STATS.uploadsSubmitted,
      上传成功张数: STATS.uploadsDone,
      复制次数: STATS.copies,
      扫描次数: STATS.sweeps,
      静默拦截: STATS.blockedSilently,
    }),
    summary: LOG.summary,
    rescan: () => sweep(document.documentElement),
    panel: {
      open: () => setPanelOpen(true),
      close: () => setPanelOpen(false),
      items: () => PANEL.items.map((it) => ({
        name: it.file.name || 'clipboard',
        size: it.file.size,
        state: PANEL.busy && PANEL.curIds.includes(it.id) ? 'uploading' : 'pending',
      })),
    },
  };

  boot();
})();