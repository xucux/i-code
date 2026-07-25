import { invoke } from '@tauri-apps/api/core'
import { useCallback, useState } from 'react'
import { IcodeError, toIcodeError } from '@/core/errors'

export interface UseCommandOptions<T> {
  command: string
  onSuccess?: (data: T) => void
  onError?: (error: IcodeError) => void
}

export interface UseCommandResult<T, P = unknown> {
  data: T | null
  error: IcodeError | null
  loading: boolean
  execute: (payload?: P) => Promise<T | null>
  reset: () => void
}

export function useCommand<T, P = unknown>(
  command: string,
  options?: Omit<UseCommandOptions<T>, 'command'>
): UseCommandResult<T, P> {
  const [data, setData] = useState<T | null>(null)
  const [error, setError] = useState<IcodeError | null>(null)
  const [loading, setLoading] = useState(false)

  const execute = useCallback(
    async (payload?: P) => {
      setLoading(true)
      setError(null)
      try {
        const result = await invoke<T>(command, payload as Record<string, unknown>)
        setData(result)
        options?.onSuccess?.(result)
        return result
      } catch (e) {
        const err = toIcodeError(e)
        setError(err)
        options?.onError?.(err)
        return null
      } finally {
        setLoading(false)
      }
    },
    [command, options]
  )

  const reset = useCallback(() => {
    setData(null)
    setError(null)
    setLoading(false)
  }, [])

  return { data, error, loading, execute, reset }
}

export async function invokeCommand<T>(command: string, payload?: unknown): Promise<T> {
  if (import.meta.env.DEV) {
    console.log(`[invoke] → ${command}`, payload)
  }
  try {
    const result = await invoke<T>(command, payload as Record<string, unknown>)
    if (import.meta.env.DEV) {
      console.log(`[invoke] ← ${command}`, result)
    }
    return result
  } catch (e) {
    if (import.meta.env.DEV) {
      console.error(`[invoke] ✗ ${command}`, e)
    }
    throw toIcodeError(e)
  }
}
