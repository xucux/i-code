import { create } from 'zustand'
import type { AppSettings } from './types'

interface SettingsState {
  settings: AppSettings | null
  isLoading: boolean
  setSettings: (settings: AppSettings) => void
  updateSettings: (patch: Partial<AppSettings>) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  isLoading: false,
  setSettings: (settings) => set({ settings }),
  updateSettings: (patch) =>
    set((state) => ({
      settings: state.settings ? { ...state.settings, ...patch } : null,
    })),
}))
