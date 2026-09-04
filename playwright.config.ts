import { defineConfig, devices } from '@playwright/test'

const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm'

export default defineConfig({
  testDir: './tests/ui',
  timeout: 15_000,
  use: { baseURL: 'http://127.0.0.1:4173', ...devices['Desktop Chrome'] },
  webServer: { command: `${npmCommand} run dev -- --host 127.0.0.1 --port 4173`, url: 'http://127.0.0.1:4173', reuseExistingServer: true, timeout: 30_000 },
})
