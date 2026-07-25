import { useMemo, useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { useWorkspaceAggregate } from '@/hooks/use-workspace-aggregate'
import {
  createWorkspace,
  updateWorkspace,
  deleteWorkspace,
  createWorkspacePrompt,
  updateWorkspacePrompt,
  deleteWorkspacePrompt,
  createWorkspaceMcpServer,
  updateWorkspaceMcpServer,
  deleteWorkspaceMcpServer,
  createWorkspaceSkill,
  updateWorkspaceSkill,
  deleteWorkspaceSkill,
  applyWorkspace,
  applyCliConfig,
  previewCliConfig,
} from '@/hooks/use-workspace-mutation'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAvailableHeight } from '@/hooks/use-available-height'
import { ScrollPage } from '@/components/ui/scroll-page'
import { DeleteConfirmDialog } from '@/components/ui/delete-confirm-dialog'
import { WorkspaceSelector } from './workspace-selector'
import { CliOverviewList } from './cli-overview-list'
import { PromptList } from './prompt-list'
import { SkillList } from './skill-list'
import { McpServerList } from './mcp-server-list'
import { WorkspaceEmptyState } from './workspace-empty-state'
import { WorkspaceForm } from './workspace-form'
import { PromptForm } from './prompt-form'
import { McpServerForm } from './mcp-server-form'
import { SkillForm } from './skill-form'
import { WorkspacePreviewDialog } from './workspace-preview-dialog'
import { CliConfigSelectDialog } from './cli-config-select-dialog'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'
import type {
  Workspace,
  WorkspaceCliConfigAggregate,
  WorkspacePrompt,
  WorkspaceMcpServer,
  WorkspaceSkill,
  WorkspacePreviewResult,
} from '@/modules/workspace/types'

type TabType = 'overview' | 'prompts' | 'skills' | 'mcp'

type DeleteTarget =
  | { type: 'workspace'; item: Workspace }
  | { type: 'prompt'; item: WorkspacePrompt }
  | { type: 'mcp'; item: WorkspaceMcpServer }
  | { type: 'skill'; item: WorkspaceSkill }

type SubConfigType = 'prompt' | 'skill' | 'mcp'

interface WorkspaceManagerProps {
  height: number
}

/**
 * 工作区管理器
 *
 * 重构后工作区页面的核心组件：
 * - 顶部左侧为工作区选择器 + 新建工作区按钮
 * - 顶部右侧为 Tab 页签（CLI 概览 / 提示词 / 技能 / MCP）
 * - 下方内容区根据当前 Tab 动态渲染工具栏与列表
 * - 支持工作区及子配置的新建、编辑、删除、应用、预览
 */
export function WorkspaceManager({ height }: WorkspaceManagerProps) {
  const { t } = useTranslation()
  const {
    workspaces,
    selectedWorkspaceId,
    setSelectedWorkspaceId,
    aggregate,
    refetchWorkspaces,
    refetchAggregate,
  } = useWorkspaceAggregate()
  const [activeTab, setActiveTab] = useState<TabType>('overview')

  // ===== 表单弹窗状态 =====
  const [workspaceFormOpen, setWorkspaceFormOpen] = useState(false)
  const [editingWorkspace, setEditingWorkspace] = useState<Workspace | null>(null)

  const [promptFormOpen, setPromptFormOpen] = useState(false)
  const [editingPrompt, setEditingPrompt] = useState<WorkspacePrompt | null>(null)

  const [mcpFormOpen, setMcpFormOpen] = useState(false)
  const [editingMcpServer, setEditingMcpServer] = useState<WorkspaceMcpServer | null>(null)

  const [skillFormOpen, setSkillFormOpen] = useState(false)
  const [editingSkill, setEditingSkill] = useState<WorkspaceSkill | null>(null)

  // ===== 删除确认状态 =====
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null)

  // ===== CLI 选择状态（新建子配置时） =====
  const [cliSelectOpen, setCliSelectOpen] = useState(false)
  const [pendingSubConfig, setPendingSubConfig] = useState<SubConfigType | null>(null)

  // ===== 预览状态 =====
  const [previewOpen, setPreviewOpen] = useState(false)
  const [previewResult, setPreviewResult] = useState<WorkspacePreviewResult | null>(null)
  const [previewLoading, setPreviewLoading] = useState(false)

  // ===== 高度计算 =====
  const [headerHeight, headerRef] = useAvailableHeight()
  // 内容区高度 = 总高度 - 顶部工具栏高度 - 内边距（约 16px）
  const contentHeight = Math.max(0, height - headerHeight - 16)

  // ===== 派生数据 =====
  const aggregatedPrompts = useMemo(() => {
    if (!aggregate) return []
    const result: Array<{ item: WorkspacePrompt; profile: WorkspaceCliConfigAggregate['profile'] }> = []
    for (const cfg of aggregate.cliConfigs) {
      for (const prompt of cfg.prompts) {
        result.push({ item: prompt, profile: cfg.profile })
      }
    }
    return result.sort((a, b) => a.item.name.localeCompare(b.item.name))
  }, [aggregate])

  const aggregatedSkills = useMemo(() => {
    if (!aggregate) return []
    const result: Array<{ item: WorkspaceSkill; profile: WorkspaceCliConfigAggregate['profile'] }> = []
    for (const cfg of aggregate.cliConfigs) {
      for (const skill of cfg.skills) {
        result.push({ item: skill, profile: cfg.profile })
      }
    }
    return result.sort((a, b) => a.item.name.localeCompare(b.item.name))
  }, [aggregate])

  const aggregatedMcpServers = useMemo(() => {
    if (!aggregate) return []
    const result: Array<{ item: WorkspaceMcpServer; profile: WorkspaceCliConfigAggregate['profile'] }> = []
    for (const cfg of aggregate.cliConfigs) {
      for (const server of cfg.mcpServers) {
        result.push({ item: server, profile: cfg.profile })
      }
    }
    return result.sort((a, b) => a.item.name.localeCompare(b.item.name))
  }, [aggregate])

  // ===== 打开表单 =====
  const openCreateWorkspace = () => {
    setEditingWorkspace(null)
    setWorkspaceFormOpen(true)
  }

  const openFormByType = (type: SubConfigType) => {
    switch (type) {
      case 'prompt':
        setEditingPrompt(null)
        setPromptFormOpen(true)
        break
      case 'skill':
        setEditingSkill(null)
        setSkillFormOpen(true)
        break
      case 'mcp':
        setEditingMcpServer(null)
        setMcpFormOpen(true)
        break
    }
  }

  const openCreateSubConfig = (type: SubConfigType) => {
    const configs = aggregate?.cliConfigs ?? []
    if (configs.length === 1) {
      setPendingSelectedConfig(configs[0])
      openFormByType(type)
    } else {
      setPendingSubConfig(type)
      setCliSelectOpen(true)
    }
  }

  const handleCliSelected = (config: WorkspaceCliConfigAggregate) => {
    setCliSelectOpen(false)
    setPendingSelectedConfig(config)
    if (!pendingSubConfig) return
    openFormByType(pendingSubConfig)
  }

  const openEditPrompt = (prompt: WorkspacePrompt) => {
    setEditingPrompt(prompt)
    setPromptFormOpen(true)
  }

  const openEditMcpServer = (server: WorkspaceMcpServer) => {
    setEditingMcpServer(server)
    setMcpFormOpen(true)
  }

  const openEditSkill = (skill: WorkspaceSkill) => {
    setEditingSkill(skill)
    setSkillFormOpen(true)
  }

  const openDelete = (target: DeleteTarget) => {
    setDeleteTarget(target)
    setDeleteOpen(true)
  }

  // ===== 工作区提交 =====
  const handleWorkspaceSubmit = async (values: {
    slug: string
    displayName: string
    rootPath: string
    isActive: boolean
  }) => {
    if (editingWorkspace) {
      const result = await updateWorkspace(editingWorkspace.id, {
        displayName: values.displayName,
        rootPath: values.rootPath,
      })
      if (result) {
        toast.success(t('workspace.messages.workspaceUpdated'))
        setWorkspaceFormOpen(false)
        void refetchWorkspaces()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    } else {
      const result = await createWorkspace({
        slug: values.slug,
        displayName: values.displayName,
        rootPath: values.rootPath,
        isActive: values.isActive,
      })
      if (result) {
        toast.success(t('workspace.messages.workspaceCreated'))
        setWorkspaceFormOpen(false)
        setSelectedWorkspaceId(result.id)
        void refetchWorkspaces()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    }
  }

  // ===== Prompt 提交 =====
  const handlePromptSubmit = async (values: {
    name: string
    content: string
    sortOrder?: number
  }) => {
    if (editingPrompt) {
      const result = await updateWorkspacePrompt(editingPrompt.id, {
        name: values.name,
        content: values.content,
        sortOrder: values.sortOrder,
      })
      if (result) {
        toast.success(t('workspace.messages.promptUpdated'))
        setPromptFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    } else {
      // 新建时从选择器获取目标 CLI 配置头
      const configId = getSelectedCliConfigId()
      if (!configId) return
      const result = await createWorkspacePrompt({
        workspaceCliConfigId: configId,
        name: values.name,
        content: values.content,
        sortOrder: values.sortOrder ?? 0,
      })
      if (result) {
        toast.success(t('workspace.messages.promptCreated'))
        setPromptFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    }
  }

  // ===== MCP Server 提交 =====
  const handleMcpServerSubmit = async (values: {
    name: string
    transport: 'stdio' | 'sse' | 'http'
    configJson: string
    isEnabled: boolean
  }) => {
    if (editingMcpServer) {
      const result = await updateWorkspaceMcpServer(editingMcpServer.id, {
        name: values.name,
        transport: values.transport,
        configJson: values.configJson,
        isEnabled: values.isEnabled,
      })
      if (result) {
        toast.success(t('workspace.messages.mcpUpdated'))
        setMcpFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    } else {
      const configId = getSelectedCliConfigId()
      if (!configId) return
      const result = await createWorkspaceMcpServer({
        workspaceCliConfigId: configId,
        name: values.name,
        transport: values.transport,
        configJson: values.configJson,
        isEnabled: values.isEnabled,
      })
      if (result) {
        toast.success(t('workspace.messages.mcpCreated'))
        setMcpFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    }
  }

  // ===== Skill 提交 =====
  const handleSkillSubmit = async (values: {
    name: string
    sourcePath?: string
    content?: string
    isEnabled: boolean
  }) => {
    if (editingSkill) {
      const result = await updateWorkspaceSkill(editingSkill.id, {
        name: values.name,
        sourcePath: values.sourcePath,
        content: values.content,
        isEnabled: values.isEnabled,
      })
      if (result) {
        toast.success(t('workspace.messages.skillUpdated'))
        setSkillFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    } else {
      const configId = getSelectedCliConfigId()
      if (!configId) return
      const result = await createWorkspaceSkill({
        workspaceCliConfigId: configId,
        name: values.name,
        sourcePath: values.sourcePath,
        content: values.content,
        isEnabled: values.isEnabled,
      })
      if (result) {
        toast.success(t('workspace.messages.skillCreated'))
        setSkillFormOpen(false)
        void refetchAggregate()
      } else {
        toast.error(t('workspace.messages.saveFailed'))
      }
    }
  }

  // 获取当前新建的子配置所属的 CLI 配置头 ID
  // 当只有一个配置头时直接选中；多个时通过弹窗选择，结果暂存在 pendingSelectedConfig
  const [pendingSelectedConfig, setPendingSelectedConfig] = useState<WorkspaceCliConfigAggregate | null>(null)

  function getSelectedCliConfigId(): string | null {
    return pendingSelectedConfig?.config.id ?? aggregate?.cliConfigs[0]?.config.id ?? null
  }

  // ===== 应用与预览 =====
  const handleApplyAll = async () => {
    if (!selectedWorkspaceId) return
    const result = await applyWorkspace(selectedWorkspaceId)
    if (result) {
      toast.success(
        t('workspace.messages.applyAllSuccess', {
          appliedCount: result.appliedCount,
          failedCount: result.failedCount,
        })
      )
      void refetchAggregate()
    } else {
      toast.error(t('workspace.messages.applyFailed'))
    }
  }

  const handleApplyCliConfig = async (configId: string) => {
    const result = await applyCliConfig(configId)
    if (result?.success) {
      toast.success(t('workspace.messages.applyCliSuccess'))
      void refetchAggregate()
    } else {
      toast.error(result?.error ?? t('workspace.messages.applyFailed'))
    }
  }

  const handlePreview = async (configId: string) => {
    setPreviewLoading(true)
    setPreviewOpen(true)
    const result = await previewCliConfig(configId)
    setPreviewResult(result)
    setPreviewLoading(false)
  }

  // ===== 删除确认 =====
  const handleConfirmDelete = async () => {
    if (!deleteTarget) return
    let ok = false

    switch (deleteTarget.type) {
      case 'workspace': {
        ok = await deleteWorkspace(deleteTarget.item.id)
        if (ok) {
          if (selectedWorkspaceId === deleteTarget.item.id) {
            setSelectedWorkspaceId(null)
          }
          void refetchWorkspaces()
        }
        break
      }
      case 'prompt': {
        ok = await deleteWorkspacePrompt(deleteTarget.item.id)
        if (ok) void refetchAggregate()
        break
      }
      case 'mcp': {
        ok = await deleteWorkspaceMcpServer(deleteTarget.item.id)
        if (ok) void refetchAggregate()
        break
      }
      case 'skill': {
        ok = await deleteWorkspaceSkill(deleteTarget.item.id)
        if (ok) void refetchAggregate()
        break
      }
    }

    if (ok) {
      toast.success(t('workspace.messages.deleteSuccess'))
      setDeleteOpen(false)
    } else {
      toast.error(t('workspace.messages.deleteFailed'))
    }
  }

  const deleteTitle = deleteTarget
    ? {
        workspace: t('workspace.delete.workspaceTitle'),
        prompt: t('workspace.delete.promptTitle'),
        mcp: t('workspace.delete.mcpTitle'),
        skill: t('workspace.delete.skillTitle'),
      }[deleteTarget.type]
    : ''

  const deleteDescription = deleteTarget
    ? t('workspace.delete.description', {
        name:
          deleteTarget.type === 'workspace'
            ? deleteTarget.item.displayName
            : deleteTarget.item.name,
      })
    : ''

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as TabType)} className="flex h-full flex-col">
        {/* 顶部工具栏：工作区选择器 + Tab */}
        <div ref={headerRef} className="mb-3 flex items-center justify-between gap-4">
          <WorkspaceSelector
            workspaces={workspaces}
            selectedWorkspaceId={selectedWorkspaceId}
            onSelect={setSelectedWorkspaceId}
            onCreate={openCreateWorkspace}
          />

          <TabsList className="h-8">
            <TabsTrigger value="overview" className="text-xs">
              {t('workspace.tabs.overview')}
            </TabsTrigger>
            <TabsTrigger value="prompts" className="text-xs">
              {t('workspace.tabs.prompts')}
            </TabsTrigger>
            <TabsTrigger value="skills" className="text-xs">
              {t('workspace.tabs.skills')}
            </TabsTrigger>
            <TabsTrigger value="mcp" className="text-xs">
              {t('workspace.tabs.mcp')}
            </TabsTrigger>
          </TabsList>
        </div>

        {/* Tab 内容区 */}
        <div className="flex min-h-0 flex-1 flex-col">
          {workspaces.length === 0 ? (
            <WorkspaceEmptyState onCreate={openCreateWorkspace} />
          ) : (
            <div className="min-h-0 flex-1">
              <TabsContent value="overview" className="mt-0 h-full">
            <div className="flex h-full flex-col">
              <div className="mb-3 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('prompt')}>
                    <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                    {t('workspace.actions.newPrompt')}
                  </Button>
                  <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('skill')}>
                    <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                    {t('workspace.actions.newSkill')}
                  </Button>
                  <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('mcp')}>
                    <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                    {t('workspace.actions.newMcp')}
                  </Button>
                </div>
                <Button size="sm" className="h-7 text-xs" onClick={handleApplyAll}>
                  <i className={cn('fa-solid fa-rotate', 'mr-1.5')} />
                  {t('workspace.actions.applyAll')}
                </Button>
              </div>
              <ScrollPage style={{ height: contentHeight || undefined }} variant="borderless">
                <CliOverviewList
                  configs={aggregate?.cliConfigs ?? []}
                  onApply={handleApplyCliConfig}
                  onPreview={handlePreview}
                />
              </ScrollPage>
            </div>
          </TabsContent>

          <TabsContent value="prompts" className="mt-0 h-full">
            <div className="flex h-full flex-col">
              <div className="mb-3 flex items-center justify-between">
                <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('prompt')}>
                  <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                  {t('workspace.actions.newPrompt')}
                </Button>
              </div>
              <ScrollPage style={{ height: contentHeight || undefined }} variant="borderless">
                <PromptList
                  prompts={aggregatedPrompts}
                  onEdit={openEditPrompt}
                  onDelete={(item) => openDelete({ type: 'prompt', item })}
                  onPreview={(item) => void handlePreview(item.workspaceCliConfigId)}
                />
              </ScrollPage>
            </div>
          </TabsContent>

          <TabsContent value="skills" className="mt-0 h-full">
            <div className="flex h-full flex-col">
              <div className="mb-3 flex items-center justify-between">
                <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('skill')}>
                  <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                  {t('workspace.actions.newSkill')}
                </Button>
              </div>
              <ScrollPage style={{ height: contentHeight || undefined }} variant="borderless">
                <SkillList
                  skills={aggregatedSkills}
                  onEdit={openEditSkill}
                  onDelete={(item) => openDelete({ type: 'skill', item })}
                  onPreview={(item) => void handlePreview(item.workspaceCliConfigId)}
                />
              </ScrollPage>
            </div>
          </TabsContent>

          <TabsContent value="mcp" className="mt-0 h-full">
            <div className="flex h-full flex-col">
              <div className="mb-3 flex items-center justify-between">
                <Button size="sm" className="h-7 text-xs" onClick={() => openCreateSubConfig('mcp')}>
                  <i className={cn('fa-solid fa-plus', 'mr-1.5')} />
                  {t('workspace.actions.newMcp')}
                </Button>
              </div>
              <ScrollPage style={{ height: contentHeight || undefined }} variant="borderless">
                <McpServerList
                  servers={aggregatedMcpServers}
                  onEdit={openEditMcpServer}
                  onDelete={(item) => openDelete({ type: 'mcp', item })}
                  onPreview={(item) => void handlePreview(item.workspaceCliConfigId)}
                />
              </ScrollPage>
            </div>
          </TabsContent>
        </div>
      )}
    </div>
  </Tabs>

      <WorkspaceForm
        open={workspaceFormOpen}
        onOpenChange={setWorkspaceFormOpen}
        workspace={editingWorkspace}
        onSubmit={handleWorkspaceSubmit}
      />

      <PromptForm
        open={promptFormOpen}
        onOpenChange={setPromptFormOpen}
        prompt={editingPrompt}
        onSubmit={handlePromptSubmit}
      />

      <McpServerForm
        open={mcpFormOpen}
        onOpenChange={setMcpFormOpen}
        server={editingMcpServer}
        onSubmit={handleMcpServerSubmit}
      />

      <SkillForm
        open={skillFormOpen}
        onOpenChange={setSkillFormOpen}
        skill={editingSkill}
        onSubmit={handleSkillSubmit}
      />

      <CliConfigSelectDialog
        open={cliSelectOpen}
        onOpenChange={setCliSelectOpen}
        configs={aggregate?.cliConfigs ?? []}
        onSelect={(cfg) => {
          setPendingSelectedConfig(cfg)
          handleCliSelected(cfg)
        }}
      />

      <WorkspacePreviewDialog
        open={previewOpen}
        onOpenChange={setPreviewOpen}
        result={previewResult}
        loading={previewLoading}
      />

      <DeleteConfirmDialog
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
        title={deleteTitle}
        description={deleteDescription}
        onConfirm={handleConfirmDelete}
      />
    </div>
  )
}
