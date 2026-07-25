import { invokeCommand } from '@/hooks/use-command'
import type {
  VirtualProvider,
  VirtualModel,
  VirtualModelRoute,
  CreateVirtualProviderInput,
  UpdateVirtualProviderInput,
  CreateVirtualModelInput,
  UpdateVirtualModelInput,
  SaveVirtualModelInput,
} from '@/modules/virtual-provider/types'

/**
 * 虚拟供应商模块写入操作集合
 *
 * 封装对虚拟供应商、虚拟模型、虚拟模型路由的增删改命令调用。
 * 错误会向上抛出，由调用方（UI 组件）捕获并展示。
 */

// ===== 虚拟供应商 =====

export async function createVirtualProvider(
  input: CreateVirtualProviderInput
): Promise<VirtualProvider> {
  return invokeCommand<VirtualProvider>('virtual_provider_create', { input })
}

/**
 * 保存虚拟模型（含子级路由）
 *
 * 新建/更新共用，一次性提交父级虚拟模型与全部子级真实模型路由，
 * 后端在事务中删除旧路由并重新关联提交的路由。
 */
export async function saveVirtualModel(
  input: SaveVirtualModelInput
): Promise<VirtualModel> {
  return invokeCommand<VirtualModel>('virtual_model_save', { input })
}

export async function updateVirtualProvider(
  id: string,
  input: UpdateVirtualProviderInput
): Promise<VirtualProvider> {
  return invokeCommand<VirtualProvider>('virtual_provider_update', { id, input })
}

export async function deleteVirtualProvider(id: string): Promise<void> {
  await invokeCommand<void>('virtual_provider_delete', { id })
}

// ===== 虚拟模型 =====

export async function createVirtualModel(
  input: CreateVirtualModelInput
): Promise<VirtualModel> {
  return invokeCommand<VirtualModel>('virtual_provider_model_create', { input })
}

export async function updateVirtualModel(
  id: string,
  input: UpdateVirtualModelInput
): Promise<VirtualModel> {
  return invokeCommand<VirtualModel>('virtual_provider_model_update', { id, input })
}

export async function deleteVirtualModel(id: string): Promise<void> {
  await invokeCommand<void>('virtual_provider_model_delete', { id })
}

// ===== 虚拟模型路由 =====

export interface CreateVirtualModelRouteInput {
  virtualModelId: string
  targetProviderId: string
  targetModelId: string
  priority?: number
  enabled?: boolean
  maxRetries?: number
  retryIntervalMs?: number
  timeoutMs?: number
}

export interface UpdateVirtualModelRouteInput {
  targetProviderId?: string
  targetModelId?: string
  priority?: number
  enabled?: boolean
  maxRetries?: number
  retryIntervalMs?: number
  timeoutMs?: number
}

export async function createVirtualModelRoute(
  input: CreateVirtualModelRouteInput
): Promise<VirtualModelRoute> {
  return invokeCommand<VirtualModelRoute>('virtual_provider_route_create', { input })
}

export async function updateVirtualModelRoute(
  id: string,
  input: UpdateVirtualModelRouteInput
): Promise<VirtualModelRoute> {
  return invokeCommand<VirtualModelRoute>('virtual_provider_route_update', { id, input })
}

export async function deleteVirtualModelRoute(id: string): Promise<void> {
  await invokeCommand<void>('virtual_provider_route_delete', { id })
}

/**
 * 批量创建虚拟模型路由
 *
 * 用于模型穿梭框一次性添加多个子级模型，失败会抛出首个错误。
 */
export async function createVirtualModelRoutes(
  inputs: CreateVirtualModelRouteInput[]
): Promise<VirtualModelRoute[]> {
  const results: VirtualModelRoute[] = []
  for (const input of inputs) {
    const route = await createVirtualModelRoute(input)
    results.push(route)
  }
  return results
}

/**
 * 批量删除虚拟模型路由
 *
 * 用于模型穿梭框一次性移除多个子级模型，失败会抛出首个错误。
 */
export async function deleteVirtualModelRoutes(ids: string[]): Promise<void> {
  for (const id of ids) {
    await deleteVirtualModelRoute(id)
  }
}
