export type ThemeId = 'sage' | 'mist-blue' | 'dusty-plum' | 'graphite'

export const THEME_EVENT = 'codex-theme-changed'
const STORAGE_KEY = 'codex-theme'

export const THEME_PRESETS: Array<{ id: ThemeId; label: string; colors: [string, string, string] }> = [
  { id: 'sage', label: '松柏鼠尾草', colors: ['#F0EADC', '#B8CBAA', '#D8AE79'] },
  { id: 'mist-blue', label: '雾蓝石板', colors: ['#E8EDF0', '#AFC3CC', '#C9B18D'] },
  { id: 'dusty-plum', label: '灰紫梅子', colors: ['#EEEAF1', '#C2B7CE', '#CFAEAB'] },
  { id: 'graphite', label: '夜幕石墨', colors: ['#202522', '#8FA596', '#C3A36D'] },
]

function readTheme(): ThemeId {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY)
    return THEME_PRESETS.some((theme) => theme.id === value) ? value as ThemeId : 'sage'
  } catch {
    return 'sage'
  }
}

let theme: ThemeId = typeof window === 'undefined' ? 'sage' : readTheme()

export function getTheme(): ThemeId { return theme }

export function setTheme(next: ThemeId): ThemeId {
  theme = THEME_PRESETS.some((preset) => preset.id === next) ? next : 'sage'
  if (typeof document !== 'undefined') document.documentElement.dataset.theme = theme
  try { window.localStorage.setItem(STORAGE_KEY, theme) } catch { /* local storage may be unavailable */ }
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent(THEME_EVENT, { detail: theme }))
  return theme
}

export function applyStoredTheme(): ThemeId { return setTheme(theme) }
