// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { errorLabel, formatTokens, planLabel } from './presentation'

describe('readable bilingual status', () => {
  it('formats usage without Chinese units in English and recognizes the Pro plan identifier', () => {
    expect(formatTokens(260_000_000, 'en-US')).toBe('260M')
    expect(formatTokens(260_000_000, 'zh-CN')).toBe('2.6亿')
    expect(planLabel('prolite')).toBe('Pro')
    expect(errorLabel('服务异常', 'en-US')).not.toMatch(/[\u3400-\u9fff]/)
    expect(errorLabel('usage_unavailable', 'en-US')).toContain('last value')
  })
})
