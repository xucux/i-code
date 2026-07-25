import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollPage } from '@/components/ui/scroll-page'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { CodeEditor } from '@/components/ui/code-editor'
import { checkCliClient, inspectCliConfigFile } from '@/hooks/use-cli-profiles'
import {
  readCliConfigFile,
  saveCliConfigFile,
  updateCliProfile,
} from '@/hooks/use-cli-mutation'
import { useTranslation } from '@/modules/i18n/use-translation'
import type {
  CliConfigFileInspection,
  CliConfigParseStatus,
  CliProfile,
  CliType,
} from '@/modules/cli-management/types'

const CLIENTS: Array<{ slug: string; cliType: CliType; labelKey: string; icon: string }> = [
  { slug: 'claude-code', cliType: 'claude-code', labelKey: 'cli.tabs.claude', icon: 'fa-terminal' },
  { slug: 'codex', cliType: 'codex', labelKey: 'cli.tabs.codex', icon: 'fa-code' },
  { slug: 'opencode', cliType: 'opencode', labelKey: 'cli.tabs.opencode', icon: 'fa-code-branch' },
]

export interface CliSettingsPanelProps {
  profiles: CliProfile[]
  height: number
  onProfilesChange: () => void
}

/** CLI 配置文件路径、存在性与语法状态设置。 */
export function CliSettingsPanel({ profiles, height, onProfilesChange }: CliSettingsPanelProps) {
  const { t } = useTranslation()
  const [paths, setPaths] = useState<Record<string, string>>({})
  const [inspections, setInspections] = useState<Record<string, CliConfigFileInspection>>({})
  const [clientAvailable, setClientAvailable] = useState<Record<string, boolean>>({})
  const [checking, setChecking] = useState<Record<string, boolean>>({})
  const [saving, setSaving] = useState<Record<string, boolean>>({})
  const [checkingClient, setCheckingClient] = useState<Record<string, boolean>>({})
  const [editingFile, setEditingFile] = useState<{
    profile: CliProfile
    content: string
    format: string
    loading: boolean
    saving: boolean
  } | null>(null)

  useEffect(() => {
    const nextPaths: Record<string, string> = {}
    for (const profile of profiles) nextPaths[profile.slug] = profile.configFilePath ?? ''
    setPaths(nextPaths)
  }, [profiles])

  useEffect(() => {
    for (const client of CLIENTS) {
      const profile = profiles.find((item) => item.slug === client.slug)
      if (!profile) continue
      void inspect(client.slug, client.cliType, profile.configFilePath)
      void checkClient(client.slug, client.cliType)
    }
    // 初次加载及档案刷新时重新探测；输入过程不自动读取磁盘。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profiles])

  const inspect = async (slug: string, cliType: CliType, configuredPath?: string) => {
    setChecking((current) => ({ ...current, [slug]: true }))
    try {
      const result = await inspectCliConfigFile(cliType, configuredPath)
      setInspections((current) => ({ ...current, [slug]: result }))
    } catch {
      toast.error(t('cli.messages.inspectFailed'))
    } finally {
      setChecking((current) => ({ ...current, [slug]: false }))
    }
  }

  const checkClient = async (slug: string, cliType: CliType) => {
    setCheckingClient((current) => ({ ...current, [slug]: true }))
    try {
      const result = await checkCliClient(cliType)
      setClientAvailable((current) => ({ ...current, [slug]: result }))
    } catch {
      setClientAvailable((current) => ({ ...current, [slug]: false }))
    } finally {
      setCheckingClient((current) => ({ ...current, [slug]: false }))
    }
  }

  const savePath = async (profile: CliProfile) => {
    setSaving((current) => ({ ...current, [profile.slug]: true }))
    const path = paths[profile.slug]?.trim() ?? ''
    const result = await updateCliProfile(profile.id, { configFilePath: path })
    setSaving((current) => ({ ...current, [profile.slug]: false }))
    if (!result) {
      toast.error(t('cli.messages.saveFailed'))
      return
    }
    toast.success(t('cli.messages.pathSaved'))
    await inspect(profile.slug, profile.cliType, path)
    onProfilesChange()
  }

  const toggleEnabled = async (profile: CliProfile, enabled: boolean) => {
    const result = await updateCliProfile(profile.id, { isEnabled: enabled })
    if (!result) {
      toast.error(t('cli.messages.saveFailed'))
      return
    }
    onProfilesChange()
  }

  const openEditFile = async (profile: CliProfile) => {
    setEditingFile({ profile, content: '', format: 'json', loading: true, saving: false })
    try {
      const result = await readCliConfigFile(profile.cliType, profile.configFilePath)
      if (!result) {
        toast.error(t('cli.messages.readFileFailed'))
        setEditingFile(null)
        return
      }
      setEditingFile({
        profile,
        content: result.content,
        format: result.format,
        loading: false,
        saving: false,
      })
    } catch {
      toast.error(t('cli.messages.readFileFailed'))
      setEditingFile(null)
    }
  }

  const saveFile = async () => {
    if (!editingFile) return
    setEditingFile({ ...editingFile, saving: true })
    try {
      const result = await saveCliConfigFile(
        editingFile.profile.cliType,
        editingFile.profile.configFilePath,
        editingFile.content
      )
      if (!result) {
        toast.error(t('cli.messages.saveFailed'))
        setEditingFile({ ...editingFile, saving: false })
        return
      }
      toast.success(t('cli.messages.fileSaved'))
      setEditingFile(null)
      await inspect(editingFile.profile.slug, editingFile.profile.cliType)
    } catch {
      toast.error(t('cli.messages.saveFailed'))
      setEditingFile({ ...editingFile, saving: false })
    }
  }

  const statusVariant = (status?: CliConfigParseStatus) => {
    if (status === 'valid') return 'default' as const
    if (status === 'invalid') return 'destructive' as const
    return 'secondary' as const
  }

  const statusLabel = (status?: CliConfigParseStatus) => {
    if (!status) return t('cli.configStatus.checking')
    return t(`cli.configStatus.${status}`)
  }

  const issueLabel = (inspection?: CliConfigFileInspection) => {
    if (!inspection?.issue) return null
    return t(`cli.configIssue.${inspection.issue}`)
  }

  return (
    <>
      <ScrollPage
        style={{ height: height || undefined }}
        variant="borderless"
        scrollbarVisible="auto"
      >
        <div className="flex flex-col gap-3 pr-3 pb-8">
          {CLIENTS.map((client) => {
            const profile = profiles.find((item) => item.slug === client.slug)
            if (!profile) return null
            const inspection = inspections[client.slug]
            const isChecking = checking[client.slug]
            const isSaving = saving[client.slug]
            const isCheckingClient = checkingClient[client.slug]
            const isClientAvailable = clientAvailable[client.slug]

            return (
              <Card key={client.slug}>
                <CardHeader className="flex flex-row items-center justify-between gap-3 pb-2.5">
                  <CardTitle className="flex items-center gap-2 text-sm">
                    <i className={`fa-solid ${client.icon} text-muted-foreground`} />
                    {t(client.labelKey)}
                    {isCheckingClient ? (
                      <i className="fa-solid fa-spinner fa-spin text-xs text-muted-foreground" />
                    ) : (
                      <Badge
                        variant={isClientAvailable ? 'default' : 'secondary'}
                        className="px-1.5 py-0 text-[9px]"
                      >
                        {isClientAvailable
                          ? t('cli.client.available')
                          : t('cli.client.unavailable')}
                      </Badge>
                    )}
                  </CardTitle>
                  <div className="flex items-center gap-2">
                    <Label
                      htmlFor={`${client.slug}-enabled`}
                      className="text-[11px] text-muted-foreground"
                    >
                      {t('cli.settings.enabled')}
                    </Label>
                    <Switch
                      id={`${client.slug}-enabled`}
                      checked={profile.isEnabled}
                      onCheckedChange={(enabled) => void toggleEnabled(profile, enabled)}
                    />
                  </div>
                </CardHeader>
                <CardContent className="flex flex-col gap-2.5">
                  <div className="flex items-end gap-2">
                    <div className="min-w-0 flex-1">
                      <Label htmlFor={`${client.slug}-path`} className="text-[11px]">
                        {t('cli.settings.configPath')}
                      </Label>
                      <Input
                        id={`${client.slug}-path`}
                        value={paths[client.slug] ?? ''}
                        onChange={(event) =>
                          setPaths((current) => ({
                            ...current,
                            [client.slug]: event.target.value,
                          }))
                        }
                        placeholder={inspection?.suggestedPath}
                        className="mt-1 h-8 font-mono text-[11px]"
                      />
                    </div>
                    <Button
                      variant="outline"
                      size="icon"
                      className="size-8 shrink-0"
                      disabled={isChecking}
                      title={t('cli.settings.inspect')}
                      onClick={() => void inspect(client.slug, client.cliType, paths[client.slug])}
                    >
                      <i
                        className={`fa-solid ${isChecking ? 'fa-spinner fa-spin' : 'fa-rotate'}`}
                      />
                      <span className="sr-only">{t('cli.settings.inspect')}</span>
                    </Button>
                    <Button
                      variant="outline"
                      size="icon"
                      className="size-8 shrink-0"
                      disabled={isSaving}
                      title={t('common.save')}
                      onClick={() => void savePath(profile)}
                    >
                      <i className={`fa-solid ${isSaving ? 'fa-spinner fa-spin' : 'fa-floppy-disk'}`} />
                      <span className="sr-only">{t('common.save')}</span>
                    </Button>
                    <Button
                      variant="outline"
                      size="icon"
                      className="size-8 shrink-0"
                      disabled={!inspection?.readable}
                      title={t('cli.settings.editFile')}
                      onClick={() => void openEditFile(profile)}
                    >
                      <i className="fa-solid fa-pen-to-square" />
                      <span className="sr-only">{t('cli.settings.editFile')}</span>
                    </Button>
                  </div>

                  <div className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-1 rounded-md bg-muted/40 px-2.5 py-1.5 text-[11px]">
                    <span className="text-muted-foreground">{t('cli.settings.status')}</span>
                    <div className="flex min-w-0 items-center gap-1.5">
                      <Badge
                        variant={statusVariant(inspection?.parseStatus)}
                        className="px-1 py-0 text-[9px]"
                      >
                        {isChecking
                          ? t('cli.configStatus.checking')
                          : statusLabel(inspection?.parseStatus)}
                      </Badge>
                      {inspection?.issue && (
                        <span className="truncate text-destructive">
                          {issueLabel(inspection)}
                        </span>
                      )}
                    </div>

                    <span className="text-muted-foreground">{t('cli.settings.resolvedPath')}</span>
                    <span className="truncate font-mono" title={inspection?.resolvedPath}>
                      {inspection?.resolvedPath ?? '-'}
                    </span>

                    <span className="text-muted-foreground">{t('cli.settings.format')}</span>
                    <span className="uppercase">{inspection?.format ?? '-'}</span>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      </ScrollPage>

      <Dialog open={editingFile !== null} onOpenChange={(open) => !open && setEditingFile(null)}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="text-base">
              {editingFile?.profile.displayName} - {t('cli.settings.editFile')}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {t('cli.settings.editFileDescription')}
            </DialogDescription>
          </DialogHeader>
          {editingFile?.loading ? (
            <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
              <i className="fa-solid fa-spinner fa-spin mr-2" />
              {t('common.loading')}
            </div>
          ) : (
            <CodeEditor
              value={editingFile?.content ?? ''}
              onChange={(value) =>
                editingFile && setEditingFile({ ...editingFile, content: value })
              }
              language={editingFile?.format}
              className="min-h-[360px]"
            />
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 text-xs"
              onClick={() => setEditingFile(null)}
            >
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              size="sm"
              className="h-8 text-xs"
              disabled={editingFile?.saving || editingFile?.loading}
              onClick={() => void saveFile()}
            >
              {editingFile?.saving && <i className="fa-solid fa-spinner fa-spin mr-1.5" />}
              {t('common.save')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
