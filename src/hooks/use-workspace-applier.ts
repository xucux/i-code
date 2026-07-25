import { useState } from 'react'

export interface WorkspaceApplyState {
  pending: boolean
  applying: boolean
  error?: string
}

export function useWorkspaceApplier(): WorkspaceApplyState {
  const [state] = useState<WorkspaceApplyState>({ pending: false, applying: false })
  return state
}
