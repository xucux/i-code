import { createFileRoute } from '@tanstack/react-router'
import { GatewaySettingsPanel } from '@/modules/ai-gateway/ui/gateway-settings'

/**
 * 网关设置页面
 *
 * 管理 AI Gateway 的监听配置和认证 API Key。
 */
function GatewaySettingsPage() {
  return (
    <div className="h-full overflow-auto p-6">
      <GatewaySettingsPanel />
    </div>
  )
}

export const Route = createFileRoute('/gateways/settings')({
  component: GatewaySettingsPage,
})
