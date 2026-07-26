/**
 * 脚本模板编辑对话框
 *
 * 布局：元数据 + CodeMirror + 右侧文档 + 试运行面板。
 * 支持系统全屏（Fullscreen API）与还原。
 */

import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { CodeEditor } from '@/components/ui/code-editor'
import { createScriptCompletions } from '@/modules/script-template/script-completions'
import { useProviderList } from '@/hooks/use-provider-list'
import { useScriptSnippets } from '@/hooks/use-script-templates'
import { HelpIcon } from '@/components/ui/help-icon'
import {
  createScriptTemplate,
  setScriptTemplateStatus,
  testScriptTemplate,
  updateScriptTemplate,
} from '@/hooks/use-script-template-mutation'
import { toIcodeError } from '@/core/errors'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import type {
  ScriptTemplate,
  ScriptTemplateTestResult,
} from '@/modules/script-template/types'
import { ScriptTemplateStatusBadge } from './script-template-status-badge'
import { ScriptSidebarDocs } from './script-sidebar-docs'
import { ScriptTestPanel } from './script-test-panel'

export interface ScriptTemplateEditorProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** null = 新建 */
  template: ScriptTemplate | null
  onSaved: () => void
}

function slugify(name: string): string {
  return name
    .trim()
    // 保留英文数字与 - _ . @，其余空白/符号折叠为 -
    .replace(/[^A-Za-z0-9\-_.@]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64)
}

export function ScriptTemplateEditor({
  open,
  onOpenChange,
  template,
  onSaved,
}: ScriptTemplateEditorProps) {
  const { t } = useTranslation('scriptTemplate')
  const isEdit = !!template
  const { providers } = useProviderList()
  const { snippets } = useScriptSnippets()

  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [description, setDescription] = useState('')
  const [scriptBody, setScriptBody] = useState('')
  const [timeoutMs, setTimeoutMs] = useState(15000)
  const [status, setStatus] = useState('draft')
  const [saving, setSaving] = useState(false)
  const [slugManual, setSlugManual] = useState(false)

  const [testProviderId, setTestProviderId] = useState('')
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<ScriptTemplateTestResult | null>(null)
  const [savedId, setSavedId] = useState<string | null>(null)
  /** 是否处于系统全屏（Fullscreen API 或 CSS 回退） */
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [cssFullscreen, setCssFullscreen] = useState(false)

  useEffect(() => {
    const onFullscreenChange = () => {
      const active = Boolean(document.fullscreenElement)
      setIsFullscreen(active)
      if (!active) setCssFullscreen(false)
    }
    document.addEventListener('fullscreenchange', onFullscreenChange)
    return () => {
      document.removeEventListener('fullscreenchange', onFullscreenChange)
    }
  }, [])

  useEffect(() => {
    if (open) return
    if (document.fullscreenElement) {
      void document.exitFullscreen().catch(() => undefined)
    }
    setIsFullscreen(false)
    setCssFullscreen(false)
  }, [open])

  useEffect(() => {
    if (!open) return
    if (template) {
      setName(template.name)
      setSlug(template.slug)
      setDescription(template.description ?? '')
      setScriptBody(template.scriptBody)
      setTimeoutMs(template.defaultTimeoutMs)
      setStatus(template.status)
      setSavedId(template.id)
      setSlugManual(true)
    } else {
      setName('')
      setSlug('')
      setDescription('')
      setScriptBody(snippets.find((s) => s.id === 'items-skeleton')?.body ?? '')
      setTimeoutMs(15000)
      setStatus('draft')
      setSavedId(null)
      setSlugManual(false)
    }
    setTestResult(null)
    setTestProviderId('')
  }, [open, template, snippets])

  const canPublish = useMemo(
    () => scriptBody.trim().length > 0 && name.trim().length > 0 && slug.trim().length > 0,
    [scriptBody, name, slug]
  )

  const expanded = isFullscreen || cssFullscreen

  const scriptCompletions = useMemo(() => [createScriptCompletions()], [])

  const handleNameChange = (v: string) => {
    setName(v)
    if (!slugManual) setSlug(slugify(v))
  }

  const handleSave = async () => {
    if (!name.trim() || !slug.trim()) {
      toast.error(t('validation.nameSlugRequired'))
      return
    }
    setSaving(true)
    try {
      if (savedId) {
        await updateScriptTemplate(savedId, {
          name: name.trim(),
          slug: slug.trim(),
          description: description.trim() || undefined,
          scriptBody,
          defaultTimeoutMs: timeoutMs,
        })
        toast.success(t('saveSuccess'))
      } else {
        const created = await createScriptTemplate({
          name: name.trim(),
          slug: slug.trim(),
          kind: 'balance',
          description: description.trim() || undefined,
          scriptBody,
          defaultTimeoutMs: timeoutMs,
        })
        setSavedId(created.id)
        setStatus(created.status)
        toast.success(t('createSuccess'))
      }
      onSaved()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSaving(false)
    }
  }

  const handleStatus = async (action: 'publish' | 'disable' | 'revert_to_draft') => {
    if (!savedId) {
      toast.error(t('validation.saveFirst'))
      return
    }
    if (action === 'publish' && !scriptBody.trim()) {
      toast.error(t('validation.scriptRequired'))
      return
    }
    setSaving(true)
    try {
      await updateScriptTemplate(savedId, {
        name: name.trim(),
        slug: slug.trim(),
        description: description.trim() || undefined,
        scriptBody,
        defaultTimeoutMs: timeoutMs,
      })
      const updated = await setScriptTemplateStatus(savedId, action)
      setStatus(updated.status)
      toast.success(t('statusUpdated'))
      onSaved()
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setSaving(false)
    }
  }

  const handleTest = async () => {
    if (!savedId) {
      toast.error(t('validation.saveFirst'))
      return
    }
    if (!testProviderId) {
      toast.error(t('validation.providerRequired'))
      return
    }
    setTesting(true)
    try {
      const result = await testScriptTemplate({
        templateId: savedId,
        providerId: testProviderId,
        scriptBodyOverride: scriptBody,
        timeoutMs,
      })
      setTestResult(result)
      if (result.ok) toast.success(t('testSuccess'))
      else toast.error(result.error ?? t('testFailed'))
    } catch (err) {
      toast.error(toIcodeError(err).message)
    } finally {
      setTesting(false)
    }
  }

  const handleInsert = (text: string) => {
    setScriptBody((prev) => (prev ? `${prev}\n${text}` : text))
  }

  /** 系统全屏切换
   *
   * 全屏整个文档（document.documentElement）而非 DialogContent 本身，
   * 避免 Radix UI 的 aria-hidden 与 Select 弹出层（portal 到 document.body）冲突。
   * 失败时回退为 CSS 铺满视口。
   */
  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen()
        return
      }
      if (cssFullscreen) {
        setCssFullscreen(false)
        return
      }
      // 全屏整个文档，DialogContent 的 Select 弹出层随文档可见
      await document.documentElement.requestFullscreen()
      setCssFullscreen(true)
    } catch {
      // Fullscreen API 不可用（如非用户手势触发），回退到 CSS 铺满
      setCssFullscreen((v) => !v)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          'flex flex-col gap-3 overflow-hidden bg-background p-4',
          expanded
            ? '!fixed !inset-0 !left-0 !top-0 !h-screen !w-screen !max-h-none !max-w-none !translate-x-0 !translate-y-0 !rounded-none border-0'
            : 'max-h-[min(640px,90vh)] w-[min(920px,96vw)] max-w-5xl'
        )}
      >
        {/* 与 Dialog 关闭按钮同规格：absolute + h-4 w-4 图标 */}
        <button
          type="button"
          className="absolute right-11 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 data-[state=open]:bg-accent data-[state=open]:text-muted-foreground"
          title={expanded ? t('restore') : t('expand')}
          aria-label={expanded ? t('restore') : t('expand')}
          onClick={toggleFullscreen}
        >
          <i
            className={cn(
              'fa-solid h-4 w-4',
              expanded ? 'fa-compress' : 'fa-expand'
            )}
          />
        </button>

        <DialogHeader className="shrink-0 space-y-2">
          <div className="flex items-center gap-2 pr-14">
            <DialogTitle className="text-sm">
              {isEdit ? t('editTitle') : t('createTitle')}
            </DialogTitle>
            <ScriptTemplateStatusBadge
              status={status}
              labels={{
                draft: t('status.draft'),
                active: t('status.active'),
                disabled: t('status.disabled'),
              }}
            />
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              className="h-7 text-xs"
              disabled={saving}
              onClick={() => void handleSave()}
            >
              {saving ? t('saving') : t('save')}
            </Button>
            {status !== 'active' && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="h-7 text-xs"
                disabled={saving || !canPublish}
                onClick={() => void handleStatus('publish')}
              >
                {t('publish')}
              </Button>
            )}
            {status === 'active' && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="h-7 text-xs"
                disabled={saving}
                onClick={() => void handleStatus('disable')}
              >
                {t('disable')}
              </Button>
            )}
            {status !== 'draft' && (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                className="h-7 text-xs"
                disabled={saving}
                onClick={() => void handleStatus('revert_to_draft')}
              >
                {t('revertDraft')}
              </Button>
            )}
          </div>
        </DialogHeader>

        <div className="grid shrink-0 grid-cols-2 gap-2 sm:grid-cols-4">
          <div className="space-y-1">
            <Label className="text-[11px]">{t('fields.name')}</Label>
            <Input
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              className="h-7 text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">{t('fields.slug')}</Label>
            <Input
              value={slug}
              onChange={(e) => {
                setSlugManual(true)
                setSlug(e.target.value)
              }}
              className="h-7 font-mono text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">{t('fields.timeout')}</Label>
            <Input
              type="number"
              value={timeoutMs}
              onChange={(e) => setTimeoutMs(Number(e.target.value) || 15000)}
              className="h-7 text-xs tabular-nums"
              min={1000}
              max={30000}
            />
          </div>
          <div className="col-span-2 space-y-1 sm:col-span-1">
            <Label className="text-[11px]">{t('fields.description')}</Label>
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="h-7 text-xs"
            />
          </div>
        </div>

        <div
          className={cn(
            'grid min-h-0 flex-1 grid-cols-1 gap-2 overflow-hidden',
            expanded ? 'md:grid-cols-[1fr_280px]' : 'md:grid-cols-[1fr_240px]'
          )}
        >
          <div className="flex min-h-0 flex-col overflow-hidden">
            <div className='flex flex-row items-center gap-1 align-items'>
              <Label className=" text-[11px]">{t('fields.script')}</Label>
              <HelpIcon  trigger="click" ariaLabel="帮助" side="right"  align="center">
                <div className="max-w-xs text-xs">
                  <p className="font-medium">说明</p>
                  <p className="mt-1 text-muted-foreground">{t('engineHint')}</p>
                </div>
              </HelpIcon>
            </div>
           
            <div className="min-h-0 flex-1 overflow-hidden">
              <CodeEditor
                value={scriptBody}
                onChange={setScriptBody}
                language="javascript"
                minHeight={expanded ? 'calc(100vh - 300px)' : '280px'}
                className="h-full"
                placeholder={t('fields.scriptPlaceholder')}
                extensions={scriptCompletions}
              />
            </div>
            
          </div>
          <ScriptSidebarDocs
            snippets={snippets}
            onInsert={handleInsert}
            labels={{
              variables: t('docs.variables'),
              functions: t('docs.functions'),
              snippets: t('docs.snippets'),
              returnShape: t('docs.returnShape'),
              examples: t('docs.examples'),
              insert: t('docs.insert'),
            }}
          />
        </div>

        <div className="shrink-0">
          <ScriptTestPanel
            providers={providers}
            providerId={testProviderId}
            onProviderChange={setTestProviderId}
            onRun={() => void handleTest()}
            running={testing}
            result={testResult}
            labels={{
              provider: t('test.provider'),
              run: t('test.run'),
              running: t('test.running'),
              result: t('test.result'),
              duration: t('test.duration'),
              noResult: t('test.noResult'),
            }}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}
