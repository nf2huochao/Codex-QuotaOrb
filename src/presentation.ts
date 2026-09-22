import { Language, getLanguage } from './i18n'

export function formatTokens(tokens?: number, language: Language = getLanguage()): string {
  if (tokens === undefined || !Number.isFinite(tokens)) return '--'
  if (language === 'en-US') return new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(tokens)
  if (tokens < 10_000) return String(tokens)
  const divisor = tokens >= 100_000_000 ? 100_000_000 : 10_000
  return `${Number((tokens / divisor).toFixed(tokens >= divisor * 100 ? 0 : 1))}${divisor === 10_000 ? '万' : '亿'}`
}

// Keep raw plan identifiers intact in the data model; format only the label.
export function planLabel(plan?: string): string {
  return ({ plus: 'Plus', pro: 'Pro', prolite: 'Pro', free: 'Free', team: 'Team', business: 'Business', enterprise: 'Enterprise' } as Record<string, string>)[plan?.toLowerCase() ?? ''] ?? plan ?? '--'
}

export function errorLabel(error?: string, language: Language = getLanguage()): string {
  if (!error) return ''
  if (error === 'usage_unavailable') return language === 'en-US' ? 'Token usage unavailable; showing the last value' : 'Token 用量暂不可用，保留上次数据'
  if (language !== 'en-US') return error
  return ({
    'Codex 协议版本不兼容': 'This Codex interface is not supported. Check the configured Codex version.',
    'Codex 服务暂时无法读取数据，请稍后重试': 'Codex data is temporarily unavailable. Retrying.',
    'Codex 返回的数据格式异常': 'Codex returned incomplete or unexpected data. Retrying.',
    'Codex 未登录，请先在 Codex 中登录': 'Sign in to Codex to refresh your quota.',
    '无法连接 Codex 服务，请检查网络': 'Cannot reach Codex. Check your network connection.',
    '读取 Codex 数据超时': 'Reading Codex data timed out. Retrying.',
    'Codex app-server 暂时不可用': 'Codex is temporarily unavailable.',
    'Codex app-server 已断开，正在重连…': 'Codex disconnected. Reconnecting…',
    '已恢复历史数据，等待实时状态确认': 'Restored saved data. Waiting for a live update.',
    '等待连接 Codex app-server': 'Waiting for Codex to connect.',
  } as Record<string, string>)[error] ?? 'Unable to refresh Codex data. Check the connection and try again.'
}
