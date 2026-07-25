import { useState } from 'react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type {
  Workspace,
  WorkspaceCliConfig,
  WorkspacePrompt,
  WorkspaceMcpServer,
  WorkspaceSkill,
} from '@/modules/workspace/types'

// 模拟工作区数据
const initialWorkspaces: Workspace[] = [
  {
    id: 'workspace-1',
    slug: 'default',
    displayName: '默认工作区',
    rootPath: '~/projects/default',
    isActive: true,
    lastAppliedAt: '2026-07-15T10:00:00Z',
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T10:00:00Z',
  },
  {
    id: 'workspace-2',
    slug: 'ai-research',
    displayName: 'AI 研发',
    rootPath: '~/projects/ai-research',
    isActive: false,
    createdAt: '2026-07-15T09:00:00Z',
    updatedAt: '2026-07-15T09:00:00Z',
  },
]

// 模拟工作区 CLI 配置头数据
const initialCliConfigs: WorkspaceCliConfig[] = [
  {
    id: 'config-1',
    workspaceId: 'workspace-1',
    cliProfileId: 'profile-1',
    isApplied: true,
    pendingApply: false,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T10:00:00Z',
  },
  {
    id: 'config-2',
    workspaceId: 'workspace-1',
    cliProfileId: 'profile-2',
    isApplied: false,
    pendingApply: true,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'config-3',
    workspaceId: 'workspace-2',
    cliProfileId: 'profile-1',
    isApplied: false,
    pendingApply: true,
    createdAt: '2026-07-15T09:00:00Z',
    updatedAt: '2026-07-15T09:00:00Z',
  },
]

// 模拟 Prompts 数据
const initialPrompts: WorkspacePrompt[] = [
  {
    id: 'prompt-1',
    workspaceCliConfigId: 'config-1',
    name: '代码审查',
    content: '请对以下代码进行审查，关注性能、安全和可读性。',
    sortOrder: 0,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'prompt-2',
    workspaceCliConfigId: 'config-1',
    name: '重构建议',
    content: '请给出重构建议，并说明理由。',
    sortOrder: 1,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
]

// 模拟 MCP Servers 数据
const initialMcpServers: WorkspaceMcpServer[] = [
  {
    id: 'mcp-1',
    workspaceCliConfigId: 'config-1',
    name: 'filesystem',
    transport: 'stdio',
    configJson: JSON.stringify({ command: 'npx', args: ['-y', '@modelcontextprotocol/server-filesystem'] }),
    isEnabled: true,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'mcp-2',
    workspaceCliConfigId: 'config-1',
    name: 'github',
    transport: 'sse',
    configJson: JSON.stringify({ url: 'http://localhost:3001/sse' }),
    isEnabled: false,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
]

// 模拟 Skills 数据
const initialSkills: WorkspaceSkill[] = [
  {
    id: 'skill-1',
    workspaceCliConfigId: 'config-1',
    name: 'React 组件开发',
    content: '擅长使用 React + TypeScript 开发可复用组件。',
    isEnabled: true,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
]

/**
 * 工作区业务组件演示
 *
 * 展示工作区列表、激活切换、CLI 配置头状态、以及 Prompts / MCP / Skills 子配置，
 * 用于 preview-page 的「业务组件」Tab，不依赖真实后端数据。
 */
export function WorkspaceDemo() {
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>(initialWorkspaces[0].id)
  const [activeConfigId, setActiveConfigId] = useState<string | null>(initialCliConfigs[0].id)

  const workspaceConfigs = initialCliConfigs.filter((c) => c.workspaceId === activeWorkspaceId)

  return (
    <div className="space-y-4">
      <Tabs value={activeWorkspaceId} onValueChange={setActiveWorkspaceId}>
        <TabsList className="mb-2">
          {initialWorkspaces.map((workspace) => (
            <TabsTrigger key={workspace.id} value={workspace.id} className="text-xs">
              {workspace.isActive && (
                <i className={cn('fa-solid fa-check', 'mr-1 text-[10px]')} />
              )}
              {workspace.displayName}
            </TabsTrigger>
          ))}
        </TabsList>

        {initialWorkspaces.map((workspace) => (
          <TabsContent key={workspace.id} value={workspace.id}>
            <Card>
              <CardHeader className="pb-2">
                <div className="flex items-center justify-between">
                  <CardTitle className="text-sm font-medium">{workspace.displayName}</CardTitle>
                  <Badge variant={workspace.isActive ? 'default' : 'secondary'} className="text-[10px]">
                    {workspace.isActive ? '激活' : '未激活'}
                  </Badge>
                </div>
                <CardDescription className="text-xs">
                  <span className="font-mono">{workspace.slug}</span>
                  <span className="mx-1.5 text-muted-foreground">|</span>
                  <span>{workspace.rootPath}</span>
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4 text-xs">
                <div className="grid gap-1.5 rounded-md border p-2.5">
                  <p className="text-muted-foreground">上次应用时间</p>
                  <p>{workspace.lastAppliedAt ?? '尚未应用'}</p>
                </div>

                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <p className="font-medium">CLI 配置</p>
                    <Button variant="ghost" size="sm" className="h-6 text-[10px]">
                      <i className={cn('fa-solid fa-rotate', 'mr-1')} />
                      应用工作区
                    </Button>
                  </div>
                  {workspaceConfigs.length === 0 ? (
                    <p className="text-muted-foreground py-2">暂无 CLI 配置</p>
                  ) : (
                    <div className="space-y-2">
                      {workspaceConfigs.map((config) => (
                        <button
                          key={config.id}
                          type="button"
                          onClick={() => setActiveConfigId(config.id)}
                          className={cn(
                            'flex w-full items-center justify-between rounded-md border p-2.5 text-left transition-colors',
                            activeConfigId === config.id ? 'border-primary bg-muted/30' : 'hover:bg-muted/50'
                          )}
                        >
                          <div className="space-y-0.5">
                            <p className="font-medium">{config.cliProfileId}</p>
                            <p className="text-muted-foreground text-[10px]">
                              {config.isApplied ? '已应用' : '未应用'}
                            </p>
                          </div>
                          <Badge
                            variant={config.pendingApply ? 'destructive' : 'outline'}
                            className="text-[10px]"
                          >
                            {config.pendingApply ? '待应用' : '已同步'}
                          </Badge>
                        </button>
                      ))}
                    </div>
                  )}
                </div>

                {activeConfigId && workspaceConfigs.some((c) => c.id === activeConfigId) && (
                  <div className="space-y-3 rounded-md border p-3">
                    <p className="font-medium">子配置预览</p>
                    <Tabs defaultValue="prompts">
                      <TabsList className="h-7">
                        <TabsTrigger value="prompts" className="text-[10px]">
                          Prompts
                        </TabsTrigger>
                        <TabsTrigger value="mcp" className="text-[10px]">
                          MCP
                        </TabsTrigger>
                        <TabsTrigger value="skills" className="text-[10px]">
                          Skills
                        </TabsTrigger>
                      </TabsList>

                      <TabsContent value="prompts" className="space-y-2 pt-1">
                        {initialPrompts
                          .filter((p) => p.workspaceCliConfigId === activeConfigId)
                          .map((prompt) => (
                            <div key={prompt.id} className="rounded-md border p-2">
                              <p className="font-medium">{prompt.name}</p>
                              <p className="text-muted-foreground line-clamp-2 text-[10px]">
                                {prompt.content}
                              </p>
                            </div>
                          ))}
                      </TabsContent>

                      <TabsContent value="mcp" className="space-y-2 pt-1">
                        {initialMcpServers
                          .filter((s) => s.workspaceCliConfigId === activeConfigId)
                          .map((server) => (
                            <div
                              key={server.id}
                              className="flex items-center justify-between rounded-md border p-2"
                            >
                              <div>
                                <p className="font-medium">{server.name}</p>
                                <p className="text-muted-foreground text-[10px]">{server.transport}</p>
                              </div>
                              <Badge variant={server.isEnabled ? 'default' : 'secondary'} className="text-[10px]">
                                {server.isEnabled ? '启用' : '禁用'}
                              </Badge>
                            </div>
                          ))}
                      </TabsContent>

                      <TabsContent value="skills" className="space-y-2 pt-1">
                        {initialSkills
                          .filter((s) => s.workspaceCliConfigId === activeConfigId)
                          .map((skill) => (
                            <div key={skill.id} className="rounded-md border p-2">
                              <div className="flex items-center justify-between">
                                <p className="font-medium">{skill.name}</p>
                                <Badge variant={skill.isEnabled ? 'default' : 'secondary'} className="text-[10px]">
                                  {skill.isEnabled ? '启用' : '禁用'}
                                </Badge>
                              </div>
                              <p className="text-muted-foreground line-clamp-2 text-[10px]">
                                {skill.content ?? skill.sourcePath ?? '无内容'}
                              </p>
                            </div>
                          ))}
                      </TabsContent>
                    </Tabs>
                  </div>
                )}
              </CardContent>
            </Card>
          </TabsContent>
        ))}
      </Tabs>
    </div>
  )
}
