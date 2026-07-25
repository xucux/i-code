"use client"

import { cn } from "@/lib/utils"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"

/**
 * 日志滚动记录配置
 */
export interface LogRollingConfigData {
  /** 是否启用文件滚动记录 */
  enabled: boolean
  /** 单个日志文件最大大小（MB） */
  maxFileSizeMb: number
  /** 保留的日志文件最大数量 */
  maxFileCount: number
  /** 日志保留天数 */
  retentionDays: number
  /** 缓冲区最大条数 */
  bufferSize: number
}

export interface LogRollingConfigProps {
  /** 当前配置 */
  value: LogRollingConfigData
  /** 配置变化回调 */
  onChange?: (value: LogRollingConfigData) => void
  /** 自定义类名 */
  className?: string
}

/**
 * 日志滚动记录配置组件
 *
 * 用于配置日志文件滚动策略与内存缓冲队列大小。
 * 当前为纯 UI 组件，具体持久化与文件写入由后端 Service 实现。
 */
export function LogRollingConfig({
  value,
  onChange,
  className,
}: LogRollingConfigProps) {
  const update = (patch: Partial<LogRollingConfigData>) => {
    onChange?.({ ...value, ...patch })
  }

  return (
    <Card className={cn(className)}>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">日志记录配置</CardTitle>
        <CardDescription className="text-xs">
          配置内存缓冲队列大小与本地日志文件滚动策略
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* 总开关 */}
        <div className="flex items-center justify-between">
          <Label htmlFor="log-rolling-enabled" className="text-xs">
            启用本地日志文件滚动记录
          </Label>
          <Switch
            id="log-rolling-enabled"
            checked={value.enabled}
            onCheckedChange={(checked) => update({ enabled: checked })}
          />
        </div>

        {/* 缓冲队列大小 */}
        <div className="space-y-1.5">
          <Label htmlFor="log-buffer-size" className="text-xs text-muted-foreground">
            内存缓冲队列条数
          </Label>
          <Input
            id="log-buffer-size"
            type="number"
            min={10}
            max={10000}
            value={value.bufferSize}
            onChange={(e) => update({ bufferSize: Number(e.target.value) })}
          />
          <p className="text-[10px] text-muted-foreground">
            日志面板仅展示缓冲队列中的最新数据
          </p>
        </div>

        {/* 文件滚动策略 */}
        <div
          className={cn(
            "space-y-4 rounded-md border p-3 transition-opacity",
            !value.enabled && "pointer-events-none opacity-50"
          )}
        >
          <p className="text-xs font-medium">滚动策略</p>

          <div className="grid gap-4 sm:grid-cols-3">
            <div className="space-y-1.5">
              <Label htmlFor="log-max-size" className="text-xs text-muted-foreground">
                单文件大小上限（MB）
              </Label>
              <Input
                id="log-max-size"
                type="number"
                min={1}
                value={value.maxFileSizeMb}
                onChange={(e) => update({ maxFileSizeMb: Number(e.target.value) })}
              />
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="log-max-count" className="text-xs text-muted-foreground">
                保留文件数量
              </Label>
              <Input
                id="log-max-count"
                type="number"
                min={1}
                max={100}
                value={value.maxFileCount}
                onChange={(e) => update({ maxFileCount: Number(e.target.value) })}
              />
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="log-retention" className="text-xs text-muted-foreground">
                保留天数
              </Label>
              <Input
                id="log-retention"
                type="number"
                min={1}
                max={365}
                value={value.retentionDays}
                onChange={(e) => update({ retentionDays: Number(e.target.value) })}
              />
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
