import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { Input } from '@/components/ui/input'
import { ScrollPage } from '@/components/ui/scroll-page'
import { ImageLightbox } from '@/components/ui/image-lightbox'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { useProviderList } from '@/hooks/use-provider-list'
import { invokeCommand } from '@/hooks/use-command'
import {
  readMediaAssetDataUrl,
  useMediaHistory,
  notifyMediaHistoryChanged,
} from '@/hooks/use-media-generation'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { toIcodeError } from '@/core/errors'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import type { MediaGeneration } from '@/modules/media-generation/types'

/** SenseNova 图像生成支持的 11 种尺寸（2K 分辨率常量，按 aspect ratio 排列） */
const SIZE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: '2752x1536', label: '2752×1536 · 16:9' },
  { value: '1536x2752', label: '1536×2752 · 9:16' },
  { value: '2496x1664', label: '2496×1664 · 3:2' },
  { value: '1664x2496', label: '1664×2496 · 2:3' },
  { value: '2368x1760', label: '2368×1760 · 4:3' },
  { value: '1760x2368', label: '1760×2368 · 3:4' },
  { value: '2272x1824', label: '2272×1824 · 5:4' },
  { value: '1824x2272', label: '1824×2272 · 4:5' },
  { value: '2048x2048', label: '2048×2048 · 1:1' },
  { value: '3072x1376', label: '3072×1376 · 21:9' },
  { value: '1344x3136', label: '1344×3136 · 9:21' },
]

type VisionTab = 'workbench' | 'gallery'

/** 读取右键时刻的选中文本：优先可编辑目标（textarea/input）的内部选区，其次页面 Selection */
function readSelectionText(): string {
  const active = document.activeElement
  if (active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement) {
    const { selectionStart, selectionEnd, value } = active
    if (selectionStart !== null && selectionEnd !== null && selectionStart !== selectionEnd) {
      return value.slice(selectionStart, selectionEnd)
    }
    return ''
  }
  return window.getSelection()?.toString() ?? ''
}

/** 复制文本到系统剪贴板（语义对齐 Ctrl+C；静默失败） */
function copyTextToClipboard(text: string): void {
  if (!text) return
  void navigator.clipboard.writeText(text).catch(() => undefined)
}

/** 粘贴系统剪贴板文本到当前聚焦的可编辑目标光标处（语义对齐 Ctrl+V） */
async function pasteIntoActive(): Promise<void> {
  let text: string
  try {
    text = await invokeCommand<string>('clipboard_read_text')
  } catch {
    return
  }
  if (!text) return
  const active = document.activeElement
  if (active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement) {
    const start = active.selectionStart ?? active.value.length
    const end = active.selectionEnd ?? start
    const next = active.value.slice(0, start) + text + active.value.slice(end)
    // 用原生 value setter 回填，保证 React 受控组件同步
    const setter = Object.getOwnPropertyDescriptor(active.constructor.prototype, 'value')?.set
    setter?.call(active, next)
    active.setSelectionRange(start + text.length, start + text.length)
    active.dispatchEvent(new Event('input', { bubbles: true }))
  }
}

/** 产物 data URL 模块级缓存：画廊缩略图与灯箱共享，避免同一图片重复读取 Base64 */
const assetUrlCache = new Map<string, string>()

/**
 * 读取媒体产物 data URL（带缓存）
 *
 * 返回 `{ url, failed }`：加载中 url 为 null；读取失败 failed 为 true。
 */
function useAssetDataUrl(path: string): { url: string | null; failed: boolean } {
  const [url, setUrl] = useState<string | null>(assetUrlCache.get(path) ?? null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let cancelled = false
    setFailed(false)
    const cached = assetUrlCache.get(path)
    if (cached) {
      setUrl(cached)
      return
    }
    setUrl(null)
    readMediaAssetDataUrl(path)
      .then((u) => {
        assetUrlCache.set(path, u)
        if (!cancelled) setUrl(u)
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [path])

  return { url, failed }
}

/** 单张产物图片：按需读取 Base64 并缓存展示 */
function AssetImage({
  path,
  className,
}: {
  path: string
  className?: string
}) {
  const { t } = useTranslation('vision')
  const { url, failed } = useAssetDataUrl(path)

  if (failed) {
    return (
      <div
        className={cn(
          'bg-muted text-muted-foreground flex items-center justify-center text-xs',
          className,
        )}
      >
        {t('imageLoadFailed')}
      </div>
    )
  }
  if (!url) {
    return (
      <div className={cn('bg-muted flex items-center justify-center', className)}>
        <i className="fa-solid fa-circle-notch fa-spin text-muted-foreground" />
      </div>
    )
  }
  return <img src={url} alt={path} className={cn('object-contain', className)} />
}

/**
 * 灯箱宿主：加载产物 data URL 后渲染 ImageLightbox
 *
 * 支持 n>1 的多图切换（左右箭头）；缩放由 ImageLightbox 内部管理，切图自动重置。
 */
function LightboxHost({
  item,
  index,
  onClose,
  onNav,
  onReuse,
  onImageContextMenu,
}: {
  item: MediaGeneration
  index: number
  onClose: () => void
  onNav: (nextIndex: number) => void
  /** 「复用提示词」：将历史记录的 prompt 回填到工作台 */
  onReuse: (prompt: string) => void
  onImageContextMenu: (e: React.MouseEvent) => void
}) {
  const { t } = useTranslation('vision')
  const path = item.assetPaths[index]
  const { url } = useAssetDataUrl(path)
  if (!url) return null

  return (
    <ImageLightbox
      key={path}
      src={url}
      alt={item.prompt}
      onClose={onClose}
      caption={item.prompt}
      onPrev={index > 0 ? () => onNav(index - 1) : undefined}
      onNext={index < item.assetPaths.length - 1 ? () => onNav(index + 1) : undefined}
      onImageContextMenu={(e) => onImageContextMenu(e)}
      actions={
        <Button
          variant="ghost"
          size="sm"
          className="h-7 justify-start px-2 text-xs text-white/80 hover:bg-white/10 hover:text-white"
          onClick={() => {
            onReuse(item.prompt)
            onClose()
          }}
        >
          <i className="fa-solid fa-rotate-right mr-1.5 text-[11px]" />
          {t('reusePrompt')}
        </Button>
      }
    />
  )
}

/**
 * 视觉生成页面
 *
 * - 工作台：提示词 + 参数面板（供应商 / 模型 / 尺寸 / 数量 / 水印）+ 画布预览
 * - 画廊：生成历史网格 + 大图预览 + 删除
 * - 数据来源：视觉生成供应商预设仅来自 builtin-*-vision.json（Phase 2 隔离约束）
 */
export function VisionPage() {
  const { t } = useTranslation('vision')
  const { providers } = useProviderList()
  const { history, refresh: refreshHistory } = useMediaHistory()

  // 仅视觉生成供应商可参与
  const mediaProviders = useMemo(
    () => providers.filter((p) => p.isMediaGeneration && p.isEnabled),
    [providers],
  )
  const [providerId, setProviderId] = useState('')
  const [models, setModels] = useState<Array<{ id: string; modelId: string }>>([])
  const [modelKey, setModelKey] = useState('')

  const [tab, setTab] = useState<VisionTab>('workbench')
  const [prompt, setPrompt] = useState('')
  const [size, setSize] = useState<string>('none')
  const [count, setCount] = useState(1)
  const [watermark, setWatermark] = useState(true)
  const [generating, setGenerating] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [current, setCurrent] = useState<MediaGeneration | null>(null)
  const [lightbox, setLightbox] = useState<{ item: MediaGeneration; index: number } | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<MediaGeneration | null>(null)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // 图片右键菜单（接管浏览器默认菜单，提供复制/下载图片与复制/粘贴文本）
  const [imageMenu, setImageMenu] = useState<{
    x: number
    y: number
    /** 右键目标为图片时的产物路径与导出文件名；缺省表示非图片区域 */
    path?: string
    fileName?: string
    /** 右键时刻的选中文本（供「复制」项判断可用性） */
    selection: string
  } | null>(null)

  // 内容区高度：直接测量 wrapper 自身（flex-1 分配后的实际渲染高度），
  // 供画廊 ScrollPage 传入数值高度；工作台为双列自适应布局，无需数值
  // （禁止在测量值上再做手工加减——padding/margin 漏项会导致内容溢出窗口）
  const [contentHeight, contentRef] = useAvailableHeight()

  // 默认选中首个视觉生成供应商
  useEffect(() => {
    if (!providerId && mediaProviders.length > 0) {
      setProviderId(mediaProviders[0].id)
    }
  }, [mediaProviders, providerId])

  // 供应商变化时拉取其模型列表
  useEffect(() => {
    if (!providerId) {
      setModels([])
      setModelKey('')
      return
    }
    let cancelled = false
    invokeCommand<Array<{ id: string; modelId: string; displayName?: string }>>(
      'gateway_model_list_by_provider',
      { providerId },
    )
      .then((list) => {
        if (cancelled) return
        setModels(list)
        setModelKey(list[0]?.modelId ?? '')
      })
      .catch(() => {
        if (!cancelled) {
          setModels([])
          setModelKey('')
        }
      })
    return () => {
      cancelled = true
    }
  }, [providerId])

  // 生成中的耗时计时器
  useEffect(() => {
    if (generating) {
      setElapsed(0)
      timerRef.current = setInterval(() => setElapsed((s) => s + 1), 1000)
    } else if (timerRef.current) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current)
        timerRef.current = null
      }
    }
  }, [generating])

  const handleGenerate = useCallback(async () => {
    if (!providerId || !modelKey || !prompt.trim()) return
    setGenerating(true)
    setCurrent(null)
    try {
      const result = await invokeCommand<MediaGeneration>('media_generate_image', {
        input: {
          providerId,
          modelId: modelKey,
          prompt,
          size: size === 'none' ? undefined : size,
          n: count,
          watermark,
        },
      })
      setCurrent(result)
      void notifyMediaHistoryChanged()
      toast.success(t('generateSuccess'))
    } catch (err) {
      const error = toIcodeError(err)
      toast.error(error.message)
    } finally {
      setGenerating(false)
    }
  }, [providerId, modelKey, prompt, size, count, watermark, t])

  const handleDelete = useCallback(
    async (target: MediaGeneration) => {
      try {
        await invokeCommand('media_history_delete', { id: target.id })
        setDeleteTarget(null)
        setLightbox(null)
        await notifyMediaHistoryChanged()
        void refreshHistory()
        toast.success(t('deleteSuccess'))
      } catch (err) {
        toast.error(toIcodeError(err).message)
      }
    },
    [refreshHistory, t],
  )

  const filteredHistory = useMemo(() => history, [history])
  const selectedProvider = mediaProviders.find((p) => p.id === providerId)

  /** 产物导出文件名：{modelId}-{日期}-{序号}.{ext} */
  const assetFileName = useCallback(
    (gen: MediaGeneration, path: string, index: number) => {
      const ext = path.split('.').pop() || 'png'
      return `${gen.modelId}-${gen.createdAt.slice(0, 10)}-${index + 1}.${ext}`
    },
    [],
  )

  /** 接管图片右键：弹出自定义菜单（复制图片 / 下载图片 / 复制 / 粘贴） */
  const openImageMenu = useCallback((e: React.MouseEvent, path: string, fileName: string) => {
    e.preventDefault()
    e.stopPropagation()
    setImageMenu({ x: e.clientX, y: e.clientY, path, fileName, selection: readSelectionText() })
  }, [])

  /** 非图片区域右键：仅提供 复制 / 粘贴（文本语义） */
  const openTextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    setImageMenu({ x: e.clientX, y: e.clientY, selection: readSelectionText() })
  }, [])

  // 右键菜单：mousedown（捕获）点击菜单外 / 滚动 / 窗口尺寸变化 / Escape 关闭
  // 对齐 AppGlobalMenu 的关闭模式：click 事件在 ScrollArea 内不可靠，
  // 滚动画廊时 fixed 菜单必须随滚动立即关闭
  const menuRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    if (!imageMenu) return
    const onClose = (e: Event) => {
      if (menuRef.current?.contains(e.target as Node)) return
      setImageMenu(null)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setImageMenu(null)
    }
    window.addEventListener('mousedown', onClose, true)
    window.addEventListener('scroll', onClose, true)
    window.addEventListener('resize', onClose)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onClose, true)
      window.removeEventListener('scroll', onClose, true)
      window.removeEventListener('resize', onClose)
      window.removeEventListener('keydown', onKey)
    }
  }, [imageMenu])

  /** 复制图片到系统剪贴板（位图） */
  const handleCopyImage = useCallback(
    async (path: string) => {
      try {
        await invokeCommand('media_asset_copy', { relativePath: path })
        toast.success(t('copySuccess'))
      } catch (err) {
        toast.error(toIcodeError(err).message)
      }
    },
    [t],
  )

  /** 下载图片：弹出系统「另存为」对话框；取消时静默 */
  const handleDownloadImage = useCallback(
    async (path: string, fileName: string) => {
      try {
        const saved = await invokeCommand<string | null>('media_asset_export', {
          relativePath: path,
          suggestedName: fileName,
        })
        if (saved) toast.success(t('downloadSuccess'))
      } catch (err) {
        toast.error(toIcodeError(err).message)
      }
    },
    [t],
  )

  return (
    // data-suppress-global-contextmenu：本页接管全部右键（图片弹专属菜单，其余区域
    // 仅抑制 WebView2 原生菜单），不与全局自定义右键（AppGlobalMenu）叠加
    <div
      data-suppress-global-contextmenu
      onContextMenu={openTextMenu}
      className="flex h-full flex-col overflow-hidden px-4 pt-2 pb-3"
    >
      {/* 工具栏：Tab 切换 */}
      <div className="flex items-center justify-between">
        <div className="flex h-6 w-fit items-center gap-0.5 rounded-md bg-muted/60 p-0.5">
          {(
            [
              { key: 'workbench' as const, label: t('tabWorkbench') },
              { key: 'gallery' as const, label: t('tabGallery') },
            ]
          ).map((item) => (
            <button
              key={item.key}
              type="button"
              onClick={() => setTab(item.key)}
              className={cn(
                'h-5 rounded-[4px] px-2 text-[11px] leading-none transition-colors',
                tab === item.key
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
        {selectedProvider && (
          <Badge variant="outline" className="text-[10px]">
            {selectedProvider.displayName}
          </Badge>
        )}
      </div>

      <div ref={contentRef} className="mt-2 min-h-0 flex-1 overflow-hidden">
        {tab === 'workbench' ? (
          <div className="flex h-full min-h-0 gap-3">
            {/* 参数面板 */}
            <div className="flex w-[260px] shrink-0 flex-col gap-3 overflow-hidden rounded-md border p-3">
              <div className="space-y-1.5">
                <Label className="text-xs">{t('provider')}</Label>
                {mediaProviders.length === 0 ? (
                  <p className="text-muted-foreground text-xs leading-relaxed">
                    {t('emptyProviders')}
                  </p>
                ) : (
                  <Select value={providerId} onValueChange={setProviderId}>
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue placeholder={t('provider')} />
                    </SelectTrigger>
                    <SelectContent>
                      {mediaProviders.map((p) => (
                        <SelectItem key={p.id} value={p.id} className="text-xs">
                          {p.displayName}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">{t('model')}</Label>
                <Select value={modelKey || undefined} onValueChange={setModelKey}>
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder={t('model')} />
                  </SelectTrigger>
                  <SelectContent>
                    {models.map((m) => (
                      <SelectItem key={m.id} value={m.modelId} className="text-xs">
                        {m.modelId}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {providerId && models.length === 0 && (
                  <p className="text-muted-foreground text-xs">{t('emptyModels')}</p>
                )}
              </div>
              <div className="flex min-h-0 flex-1 flex-col gap-1.5">
                <Label className="text-xs">{t('prompt')}</Label>
                <Textarea
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  placeholder={t('promptPlaceholder')}
                  className="min-h-[80px] flex-1 resize-none text-xs"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">{t('size')}</Label>
                <Select value={size} onValueChange={setSize}>
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none" className="text-xs">
                      {t('sizeDefault')}
                    </SelectItem>
                    {SIZE_OPTIONS.map((s) => (
                      <SelectItem key={s.value} value={s.value} className="text-xs">
                        {s.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">{t('count')}</Label>
                <Input
                  type="number"
                  min={1}
                  max={4}
                  value={count}
                  onChange={(e) => {
                    const v = Number(e.target.value)
                    setCount(Number.isFinite(v) ? Math.min(4, Math.max(1, Math.trunc(v))) : 1)
                  }}
                  className="h-8 text-xs tabular-nums"
                />
              </div>
              <div className="flex items-center justify-between">
                <Label className="text-xs">{t('watermark')}</Label>
                <Switch checked={watermark} onCheckedChange={setWatermark} />
              </div>
              <p className="text-muted-foreground text-[10px] leading-snug">
                {t('watermarkHint')}
              </p>
              <Button
                size="sm"
                className="h-8 w-full text-xs"
                disabled={generating || !providerId || !modelKey || !prompt.trim()}
                onClick={() => void handleGenerate()}
              >
                {generating ? (
                  <>
                    <i className="fa-solid fa-circle-notch fa-spin mr-1.5" />
                    {t('generating')} · <span className="tabular-nums">{elapsed}s</span>
                  </>
                ) : (
                  <>
                    <i className="fa-solid fa-wand-magic-sparkles mr-1.5" />
                    {t('generate')}
                  </>
                )}
              </Button>
            </div>

            {/* 画布区 */}
            <div className="flex min-h-0 min-w-0 flex-1 flex-col rounded-md border">
              <div className="relative min-h-0 flex-1">
                {current && current.status === 'succeeded' && current.assetPaths.length > 0 ? (
                  <div
                    className={cn(
                      'grid h-full w-full place-content-center gap-2 p-3',
                      current.assetPaths.length > 1 ? 'grid-cols-2' : 'grid-cols-1',
                    )}
                  >
                    {current.assetPaths.map((p, idx) => (
                      <div
                        key={p}
                        className="flex min-h-0 items-center justify-center"
                        onContextMenu={(e) => openImageMenu(e, p, assetFileName(current, p, idx))}
                      >
                        <AssetImage
                          path={p}
                          className="max-h-full max-w-full cursor-zoom-in rounded"
                        />
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-muted-foreground flex h-full items-center justify-center text-xs">
                    {generating ? (
                      <span className="tabular-nums">
                        {t('generating')} · {elapsed}s
                      </span>
                    ) : (
                      t('canvasEmpty')
                    )}
                  </div>
                )}
              </div>
              {current && current.status === 'succeeded' && (
                <div className="text-muted-foreground flex items-center gap-3 border-t px-3 py-1.5 text-[10px] tabular-nums">
                  <span>{current.modelId}</span>
                  {current.durationMs !== undefined && (
                    <span>
                      {t('duration', { ms: current.durationMs })}
                    </span>
                  )}
                </div>
              )}
            </div>
          </div>
        ) : (
          /* 画廊 */
          <ScrollPage style={{ height: contentHeight }} variant="borderless">
            {filteredHistory.length === 0 ? (
              <p className="text-muted-foreground py-8 text-center text-xs">
                {t('noHistory')}
              </p>
            ) : (
              <div className="grid grid-cols-4 gap-2 p-1">
                {filteredHistory.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() =>
                      item.status === 'succeeded' && setLightbox({ item, index: 0 })
                    }
                    onContextMenu={(e) =>
                      item.status === 'succeeded' &&
                      item.assetPaths.length > 0 &&
                      openImageMenu(e, item.assetPaths[0], assetFileName(item, item.assetPaths[0], 0))
                    }
                    className="group rounded-md border p-1.5 text-left transition-colors hover:bg-muted/50"
                  >
                    {item.status === 'succeeded' && item.assetPaths.length > 0 ? (
                      <AssetImage
                        path={item.assetPaths[0]}
                        className="aspect-square w-full rounded"
                      />
                    ) : (
                      <div className="bg-muted text-muted-foreground flex aspect-square w-full items-center justify-center rounded text-xs">
                        {t('failed')}
                      </div>
                    )}
                    <p className="mt-1 truncate text-[10px] text-muted-foreground">
                      {item.prompt}
                    </p>
                    <div className="mt-0.5 flex items-center justify-between">
                      <span className="truncate text-[10px] text-muted-foreground tabular-nums">
                        {item.modelId}
                      </span>
                      <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                        {item.status === 'succeeded' && item.assetPaths.length > 0 && (
                          <>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-4 w-4 p-0 text-muted-foreground"
                              title={t('copyImage')}
                              onClick={(e) => {
                                e.stopPropagation()
                                void handleCopyImage(item.assetPaths[0])
                              }}
                            >
                              <i className="fa-regular fa-copy text-[10px]" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-4 w-4 p-0 text-muted-foreground"
                              title={t('downloadImage')}
                              onClick={(e) => {
                                e.stopPropagation()
                                void handleDownloadImage(
                                  item.assetPaths[0],
                                  assetFileName(item, item.assetPaths[0], 0),
                                )
                              }}
                            >
                              <i className="fa-solid fa-download text-[10px]" />
                            </Button>
                          </>
                        )}
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-4 w-4 p-0 text-muted-foreground"
                          title={t('delete')}
                          onClick={(e) => {
                            e.stopPropagation()
                            setDeleteTarget(item)
                          }}
                        >
                          <i className="fa-solid fa-trash text-[10px]" />
                        </Button>
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </ScrollPage>
        )}
      </div>

      {/* 大图预览（带缩放 / 多图切换 / 复用提示词） */}
      {lightbox && (
        <LightboxHost
          item={lightbox.item}
          index={lightbox.index}
          onClose={() => setLightbox(null)}
          onNav={(nextIndex) => setLightbox({ item: lightbox.item, index: nextIndex })}
          onReuse={(prompt) => {
            setPrompt(prompt)
            setTab('workbench')
          }}
          onImageContextMenu={(e) =>
            openImageMenu(
              e,
              lightbox.item.assetPaths[lightbox.index],
              assetFileName(lightbox.item, lightbox.item.assetPaths[lightbox.index], lightbox.index),
            )
          }
        />
      )}

      {/* 右键菜单（接管浏览器默认菜单）：图片区域含图片项，全部区域含复制/粘贴 */}
      {imageMenu && (
        <div
          ref={menuRef}
          className="fixed z-[999] w-fit min-w-[132px] rounded-md border bg-popover p-1 shadow-md"
          style={{
            left: Math.min(imageMenu.x, window.innerWidth - 150),
            top: Math.min(imageMenu.y, window.innerHeight - 130),
          }}
          onContextMenu={(e) => e.preventDefault()}
          onMouseDown={(e) => e.stopPropagation()}
          onClick={(e) => e.stopPropagation()}
        >
          {imageMenu.path && (
            <>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-full justify-start px-2 text-xs"
                onClick={() => {
                  setImageMenu(null)
                  void handleCopyImage(imageMenu.path!)
                }}
              >
                <i className="fa-regular fa-image mr-1.5 text-[11px]" />
                {t('copyImage')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-full justify-start px-2 text-xs"
                onClick={() => {
                  setImageMenu(null)
                  void handleDownloadImage(imageMenu.path!, imageMenu.fileName ?? 'image.png')
                }}
              >
                <i className="fa-solid fa-download mr-1.5 text-[11px]" />
                {t('downloadImage')}
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-7 w-full justify-start px-2 text-xs"
            disabled={!imageMenu.selection}
            onClick={() => {
              setImageMenu(null)
              copyTextToClipboard(imageMenu.selection)
            }}
          >
            <i className="fa-regular fa-copy mr-1.5 text-[11px]" />
            {t('copy')}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 w-full justify-start px-2 text-xs"
            onClick={() => {
              setImageMenu(null)
              void pasteIntoActive()
            }}
          >
            <i className="fa-solid fa-paste mr-1.5 text-[11px]" />
            {t('paste')}
          </Button>
        </div>
      )}

      {/* 删除确认 */}
      <AlertDialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className="text-base">
              {t('deleteConfirmTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription className="text-xs">
              {t('deleteConfirmDescription')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="h-8 text-xs">
              {t('cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              className="h-8 text-xs"
              onClick={() => deleteTarget && void handleDelete(deleteTarget)}
            >
              {t('delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
