// i-code 图床助手（ooxx.ooo）
// ============================================================================
// 能力（按用户要求裁剪）：
// - 粘贴收集：Ctrl+V 图片进入左侧浮动面板预览（与 img.remit.ee 体验一致）
// - 手动上传：面板内逐张【上传】或底部【全部上传】，点击后才注入
//   站点隐藏上传控件 #import-file-select 并触发 change，复用站点自带
//   _xsrf / AJAX 全链路（不手拼请求）
// - 成功提示：站点上传成功后 #tab-markdown 会生成 ![](url)，脚本轮询其文本
//   变化给出「上传成功」toast；**不做结果回传**（外链由用户在站点结果区人工复制）
// 刻意不做：去广告 / 快捷复制按钮 / document.title 桥接
// 由 Tauri initialization_script 注入（document-start）。
// ============================================================================

(function () {
  'use strict';

  /* ==============================
   * 〇 · 目标站点守卫（等价于篡改猴 @match）
   * ============================== */
  if (!/^https:\/\/ooxx\.ooo\//.test(location.href)) return;

  /* ==============================
   * 一 · 轻量工具
   * ============================== */

  /** 页面 toast（上传开始 / 成功 / 失败） */
  function toast(msg, ok) {
    try {
      const tip = document.createElement('div');
      tip.textContent = msg;
      tip.style.cssText = [
        'position:fixed', 'top:24px', 'left:50%', 'transform:translateX(-50%)',
        'z-index:2147483647', 'padding:8px 18px', 'border-radius:8px',
        'font-size:13px', 'color:#fff', 'pointer-events:none',
        'box-shadow:0 4px 14px rgba(0,0,0,.25)',
        ok === false ? 'background:#c0392b' : 'background:#1f883d',
      ].join(';');
      document.body.appendChild(tip);
      setTimeout(() => tip.remove(), 2400);
    } catch (e) { /* 忽略 */ }
  }

  /** 文件大小人性化显示 */
  function humanSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / 1048576).toFixed(2) + ' MB';
  }

  /** 焦点是否落在文本输入元素（保留站点默认粘贴行为） */
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

  /* ==============================
   * 二 · 浮动粘贴面板（样式 + 状态）
   * ============================== */

  const PANEL_CSS = `
    .ooxx-helper-fab {
      position: fixed; left: 0; top: 46%; z-index: 2147483646;
      width: 44px; height: 92px; border: none; cursor: pointer;
      background: linear-gradient(180deg,#0ea5e9,#2563eb);
      color: #fff; font-size: 11px; text-align: center; line-height: 1.25;
      border-radius: 0 12px 12px 0; box-shadow: 0 4px 16px rgba(14,165,233,.35);
      padding: 4px 0; font-weight: 700; opacity: .95;
      display: flex; flex-direction: column; align-items: center; gap: 3px;
    }
    .ooxx-helper-fab:hover { filter: brightness(1.12); }

    .ooxx-helper-panel {
      position: fixed; left: 12px; top: 50%; transform: translateY(-50%);
      width: 262px; max-height: min(76vh, 620px); overflow-y: auto;
      background: #fff; border: 1px solid #e2e8f0; border-radius: 12px;
      box-shadow: 0 12px 30px rgba(15,23,42,.22); z-index: 2147483646;
      font-size: 12px; color: #334155; display: none;
    }
    .ooxx-helper-panel-head {
      display: flex; align-items: center; justify-content: space-between;
      padding: 8px 10px; background: linear-gradient(90deg,#0ea5e9,#2563eb);
      color: #fff; font-weight: 700; font-size: 12.5px;
      border-radius: 12px 12px 0 0;
    }
    .ooxx-helper-panel-head button {
      background: transparent; color: #fff; border: 1px solid rgba(255,255,255,.6);
      border-radius: 6px; padding: 1px 8px; font-size: 11px; cursor: pointer;
    }
    .ooxx-helper-panel-head button:hover { background: rgba(255,255,255,.22); }

    .ooxx-helper-paste-hint {
      margin: 8px 10px; border: 1.5px dashed #7dd3fc; border-radius: 10px;
      padding: 10px 8px; text-align: center; color: #475569;
      background: #f0f9ff; cursor: pointer; outline: none;
    }
    .ooxx-helper-paste-hint:hover { background: #e0f2fe; border-color: #0ea5e9; }

    .ooxx-helper-items { margin: 0 10px 2px; }
    .ooxx-helper-items:empty::after {
      content: '暂无图片 —— 页面任意空白处粘贴（Ctrl+V）';
      display: block; text-align: center; color: #94a3b8;
      padding: 14px 0; font-size: 11px;
    }
    .ooxx-helper-item {
      display: flex; gap: 8px; align-items: center;
      border: 1px solid #e2e8f0; border-radius: 8px;
      padding: 6px 8px; margin-bottom: 6px; background: #fafbfc;
    }
    .ooxx-helper-item.is-uploading { border-color: #7dd3fc; background: #f0f9ff; }
    .ooxx-helper-item img {
      width: 44px; height: 44px; object-fit: cover;
      border-radius: 6px; background: #f1f5f9;
    }
    .ooxx-helper-item-info { flex: 1; min-width: 0; overflow: hidden; }
    .ooxx-helper-item-name {
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
      font-size: 11.5px; color: #1e293b; font-weight: 600;
    }
    .ooxx-helper-item-size { font-size: 10.5px; color: #94a3b8; }
    .ooxx-helper-item-ops { display: flex; gap: 4px; flex-shrink: 0; }
    .ooxx-helper-item-ops button {
      border: none; border-radius: 6px; cursor: pointer;
      font-size: 11px; padding: 3px 8px; line-height: 1;
    }
    .oh-up { background: #2563eb; color: #fff; }
    .oh-up[disabled] { background: #94a3b8; color: #fff; cursor: default; }
    .oh-del { background: #fff; color: #94a3b8; border: 1px solid #e2e8f0; }
    .oh-del:hover { color: #dc2626; border-color: #fca5a5; }
    .ooxx-helper-item-ops button:not([disabled]):hover { filter: brightness(1.12); }

    .ooxx-helper-panel-foot { margin: 8px 10px 12px; text-align: center; }
    .ooxx-helper-panel-foot button {
      width: 100%; padding: 7px 0; border: none; border-radius: 8px;
      background: linear-gradient(90deg,#0ea5e9,#2563eb); color: #fff;
      font-size: 12px; font-weight: 700; cursor: pointer;
    }
    .ooxx-helper-panel-foot button[disabled] { background: #94a3b8; cursor: default; }
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

  function setPanelOpen(open) {
    PANEL.opened = open;
    PANEL.root.style.display = open ? 'block' : 'none';
    PANEL.fab.style.display = open ? 'none' : 'flex';
  }

  function syncPanel() {
    PANEL.upAllBtn.textContent = PANEL.busy
      ? '正在上传…'
      : (PANEL.items.length ? `全部上传 (${PANEL.items.length})` : '全部上传 (0)');
    PANEL.upAllBtn.disabled = PANEL.busy || !PANEL.items.length;
    for (const it of PANEL.items) {
      it.upBtn.disabled = PANEL.busy;
      it.upBtn.textContent = PANEL.busy ? (PANEL.curIds.includes(it.id) ? '上传中…' : '排队') : '上传';
    }
  }

  function setBusy(busy) {
    PANEL.busy = busy;
    for (const it of PANEL.items) {
      it.row.classList.toggle('is-uploading', busy && PANEL.curIds.includes(it.id));
    }
    syncPanel();
  }

  function addPanelItem(file) {
    const id = 'ohp' + (++PANEL.seq);
    const url = URL.createObjectURL(file);

    const row = document.createElement('div');
    row.className = 'ooxx-helper-item';
    row.dataset.id = id;

    const img = document.createElement('img');
    img.src = url;
    img.alt = file.name || 'clipboard';

    const info = document.createElement('div');
    info.className = 'ooxx-helper-item-info';
    const nameEl = document.createElement('div');
    nameEl.className = 'ooxx-helper-item-name';
    const label = file.name || '剪贴板图片';
    nameEl.textContent = label;
    nameEl.title = label;
    const sizeEl = document.createElement('div');
    sizeEl.className = 'ooxx-helper-item-size';
    sizeEl.textContent = humanSize(file.size);

    const ops = document.createElement('div');
    ops.className = 'ooxx-helper-item-ops';
    const up = document.createElement('button');
    up.type = 'button'; up.className = 'oh-up'; up.textContent = '上传';
    up.addEventListener('click', () => uploadOne(id));
    const del = document.createElement('button');
    del.type = 'button'; del.className = 'oh-del'; del.textContent = '✕';
    del.addEventListener('click', () => removePanelItem(id));

    info.append(nameEl, sizeEl);
    ops.append(up, del);
    row.append(img, info, ops);
    PANEL.list.prepend(row); // 新粘贴的置顶
    PANEL.items.push({ id, file, url, row, upBtn: up });
    syncPanel();
  }

  function addPanelItems(files) {
    for (const f of files) addPanelItem(f);
    toast(`已加入 ${files.length} 张图片，点击「上传」提交`);
  }

  function removePanelItem(id) {
    const idx = PANEL.items.findIndex((it) => it.id === id);
    if (idx < 0) return;
    const it = PANEL.items[idx];
    PANEL.items.splice(idx, 1);
    it.row.remove();
    try { URL.revokeObjectURL(it.url); } catch (e) {}
    syncPanel();
  }

  function clearPanelItems() {
    if (PANEL.busy) { toast('上传进行中，稍候再清空', false); return; }
    for (const it of PANEL.items.splice(0)) {
      it.row.remove();
      try { URL.revokeObjectURL(it.url); } catch (e) {}
    }
    syncPanel();
  }

  /* ==============================
   * 三 · 上传（手动触发 → 注入站点控件）
   * ============================== */

  /** 把 File 列表注入站点上传控件（#import-file-select）并触发 change */
  function injectIntoUploadInput(files) {
    const input = document.getElementById('import-file-select');
    if (!(input instanceof HTMLInputElement) || typeof DataTransfer === 'undefined') {
      return false;
    }
    let dt = null;
    try {
      dt = new DataTransfer();
      for (const f of files) dt.items.add(f);
      input.files = dt.files;
    } catch (e) {
      try {
        // 兜底：定义读取访问器，站点读 e.target.files 时拿到我们的列表
        const fl = dt ? dt.files : null;
        Object.defineProperty(input, 'files', {
          get: () => fl,
          configurable: true,
        });
      } catch (e2) {
        return false;
      }
    }
    input.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }

  /** 当前站点结果区文本（上传成功后 #tab-markdown 会填入 ![](url)） */
  function currentResultText() {
    const tab = document.getElementById('tab-markdown');
    return tab ? (tab.textContent || '').trim() : '';
  }

  /** 上传（同一时间只允许一批）；成功判定 = #tab-markdown 文本发生变化 */
  function submitUpload(ids) {
    if (PANEL.busy) return;
    const targets = PANEL.items.filter((it) => ids.includes(it.id));
    if (!targets.length) return;

    PANEL.curIds = ids;
    setBusy(true);
    toast(`正在上传 ${targets.length} 张图片…`);

    const before = currentResultText();
    if (!injectIntoUploadInput(targets.map((it) => it.file))) {
      toast('未找到上传控件（#import-file-select），请刷新页面', false);
      PANEL.curIds = [];
      setBusy(false);
      syncPanel();
      return;
    }

    const begun = Date.now();
    const timer = setInterval(() => {
      const changed = currentResultText() !== before;
      const timedOut = Date.now() - begun > 60000;
      if (!changed && !timedOut) return;
      clearInterval(timer);

      if (changed) {
        toast(`上传成功 ×${targets.length}，可到上方结果区复制链接`, true);
        for (const id of ids) removePanelItem(id);
      } else {
        toast('未检测到上传结果，可能失败，可刷新后重试', false);
      }
      PANEL.curIds = [];
      setBusy(false);
      syncPanel();
    }, 800);
  }

  function uploadOne(id) { submitUpload([id]); }
  function uploadAll() { submitUpload(PANEL.items.map((it) => it.id)); }

  /* ==============================
   * 四 · 面板构建 + 粘贴监听
   * ============================== */

  function buildPanel() {
    // 折叠态：左侧悬浮入口
    const fab = document.createElement('button');
    fab.type = 'button';
    fab.className = 'ooxx-helper-fab';
    fab.title = '剪贴板粘贴上传（点击展开面板）';
    fab.textContent = '粘贴\n上传';
    fab.addEventListener('click', () => setPanelOpen(true));
    document.body.appendChild(fab);

    // 面板样式
    const style = document.createElement('style');
    style.setAttribute('data-userscript', 'ooxx-helper-panel');
    style.textContent = PANEL_CSS;
    (document.head || document.documentElement).appendChild(style);

    // 展开态：左侧浮动面板
    const root = document.createElement('aside');
    root.className = 'ooxx-helper-panel';

    const head = document.createElement('div');
    head.className = 'ooxx-helper-panel-head';
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
    hint.className = 'ooxx-helper-paste-hint';
    hint.tabIndex = 0;
    hint.textContent = '页面任意空白处 Ctrl+V 粘贴图片';
    hint.addEventListener('click', () => hint.focus());

    const list = document.createElement('div');
    list.className = 'ooxx-helper-items';

    const foot = document.createElement('div');
    foot.className = 'ooxx-helper-panel-foot';
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
    document.addEventListener(
      'paste',
      (e) => {
        const target = e.target;
        if (isTextyTarget(target)) return; // 文本框内粘贴保持默认

        const dt = e.clipboardData;
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
      },
      true
    );
  }

  function boot() {
    buildPanel();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot, { once: true });
  } else {
    boot();
  }
})();