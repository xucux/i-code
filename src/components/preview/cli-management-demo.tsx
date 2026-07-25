import { useState } from 'react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { CliProfile, CliProvider, CliModelMapping } from '@/modules/cli-management/types'

// 模拟 CLI 档案数据
const initialProfiles: CliProfile[] = [
  {
    id: 'profile-1',
    slug: 'claude-code',
    displayName: 'Claude Code',
    cliType: 'claude-code',
    configFilePath: '~/.claude/settings.json',
    isEnabled: true,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'profile-2',
    slug: 'codex',
    displayName: 'OpenAI Codex',
    cliType: 'codex',
    configFilePath: '~/.codex/config.json',
    isEnabled: false,
    createdAt: '2026-07-15T09:00:00Z',
    updatedAt: '2026-07-15T09:00:00Z',
  },
]

// 模拟 CLI 供应商绑定数据
const initialProviders: CliProvider[] = [
  {
    id: 'provider-1',
    cliProfileId: 'profile-1',
    providerId: 'gateway-openai',
    displayName: 'OpenAI 渠道',
    routeMode: 1,
    gatewayBaseUrl: 'http://127.0.0.1:3000',
    sortOrder: 0,
    isDefault: true,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'provider-2',
    cliProfileId: 'profile-1',
    providerId: 'gateway-anthropic',
    displayName: 'Anthropic 直连',
    routeMode: 0,
    directBaseUrl: 'https://api.anthropic.com',
    sortOrder: 1,
    isDefault: false,
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
]

// 模拟 CLI 模型映射数据
const initialMappings: CliModelMapping[] = [
  {
    id: 'mapping-1',
    cliProviderId: 'provider-1',
    cliModelAlias: 'gpt-4o',
    gatewayModelId: 'openai/gpt-4o',
    inputMode: 'select',
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
  {
    id: 'mapping-2',
    cliProviderId: 'provider-2',
    cliModelAlias: 'claude-opus',
    rawModelId: 'claude-3-opus-20240229',
    inputMode: 'manual',
    createdAt: '2026-07-15T08:00:00Z',
    updatedAt: '2026-07-15T08:00:00Z',
  },
]

/**
 * CLI 管理业务组件演示
 *
 * 展示 CLI 档案、供应商绑定、模型映射三类数据的卡片式布局，
 * 用于 preview-page 的「业务组件」Tab，不依赖真实后端数据。
 */
export function CliManagementDemo() {
  const [activeProfileId, setActiveProfileId] = useState<string>(initialProfiles[0].id)

  const profileProviders = initialProviders.filter((p) => p.cliProfileId === activeProfileId)

  return (
    <div className="space-y-4">
      <Tabs value={activeProfileId} onValueChange={setActiveProfileId}>
        <TabsList className="mb-2">
          {initialProfiles.map((profile) => (
            <TabsTrigger key={profile.id} value={profile.id} className="text-xs">
              {profile.displayName}
            </TabsTrigger>
          ))}
        </TabsList>

        {initialProfiles.map((profile) => (
          <TabsContent key={profile.id} value={profile.id}>
            <Card>
              <CardHeader className="pb-2">
                <div className="flex items-center justify-between">
                  <CardTitle className="text-sm font-medium">{profile.displayName}</CardTitle>
                  <Badge variant={profile.isEnabled ? 'default' : 'secondary'} className="text-[10px]">
                    {profile.isEnabled ? '已启用' : '已禁用'}
                  </Badge>
                </div>
                <CardDescription className="text-xs">
                  <span className="font-mono">{profile.slug}</span>
                  <span className="mx-1.5 text-muted-foreground">|</span>
                  <span>{profile.cliType}</span>
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4 text-xs">
                <div className="grid gap-1.5 rounded-md border p-2.5">
                  <p className="text-muted-foreground">配置文件路径</p>
                  <p className="font-mono">{profile.configFilePath ?? '未设置'}</p>
                </div>

                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <p className="font-medium">供应商绑定</p>
                    <Button variant="ghost" size="sm" className="h-6 text-[10px]">
                      <i className={cn('fa-solid fa-plus', 'mr-1')} />
                      新增
                    </Button>
                  </div>
                  {profileProviders.length === 0 ? (
                    <p className="text-muted-foreground py-2">暂无供应商绑定</p>
                  ) : (
                    <div className="space-y-2">
                      {profileProviders.map((provider) => (
                        <div
                          key={provider.id}
                          className="flex items-center justify-between rounded-md border p-2.5"
                        >
                          <div className="space-y-0.5">
                            <p className="font-medium">{provider.displayName}</p>
                            <p className="text-muted-foreground text-[10px]">
                              {provider.routeMode === 1 ? '本地网关' : '直连'}
                              {provider.routeMode === 1
                                ? ` · ${provider.gatewayBaseUrl}`
                                : ` · ${provider.directBaseUrl}`}
                            </p>
                          </div>
                          <Badge variant={provider.isDefault ? 'default' : 'outline'} className="text-[10px]">
                            {provider.isDefault ? '默认' : `优先级 ${provider.sortOrder}`}
                          </Badge>
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                <div className="space-y-2">
                  <p className="font-medium">模型映射</p>
                  <div className="rounded-md border">
                    <div className="grid grid-cols-3 gap-2 border-b px-2.5 py-1.5 text-[10px] text-muted-foreground">
                      <span>CLI 别名</span>
                      <span>输入模式</span>
                      <span>目标模型</span>
                    </div>
                    {initialMappings
                      .filter((m) => profileProviders.some((p) => p.id === m.cliProviderId))
                      .map((mapping) => (
                        <div
                          key={mapping.id}
                          className="grid grid-cols-3 gap-2 px-2.5 py-2 text-[10px]"
                        >
                          <span className="font-mono">{mapping.cliModelAlias}</span>
                          <Badge variant="outline" className="w-fit text-[10px]">
                            {mapping.inputMode === 'select' ? '选择' : '手动'}
                          </Badge>
                          <span className="truncate">
                            {mapping.gatewayModelId ?? mapping.rawModelId ?? '-'}
                          </span>
                        </div>
                      ))}
                  </div>
                </div>
              </CardContent>
            </Card>
          </TabsContent>
        ))}
      </Tabs>
    </div>
  )
}
