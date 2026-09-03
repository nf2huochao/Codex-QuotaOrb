import { Snapshot } from '../domain'
import { Language, getLanguage, t, toggleLanguage } from '../i18n'
import { getTheme, setTheme, THEME_EVENT, THEME_PRESETS, ThemeId } from '../theme'
import { MountedView } from './FloatingIsland'

export interface SettingsActions {
  onClose: () => void
  onCheckUpdates: () => Promise<void> | void
  getAutostart: () => Promise<boolean>
  setAutostart: (enabled: boolean) => Promise<void>
  getAlwaysOnTop: () => Promise<boolean>
  setAlwaysOnTop: (enabled: boolean) => Promise<void>
}

export interface MountedSettingsView extends MountedView {
  setAutostart(value: boolean): void
  setAlwaysOnTop(value: boolean): void
}

const THEME_LABELS: Record<ThemeId, string> = {
  sage: 'themeSage',
  'mist-blue': 'themeBlue',
  'dusty-plum': 'themePlum',
  graphite: 'themeGraphite',
}

function esc(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }

export function mountSettingsPanel(root: HTMLElement, actions: SettingsActions): MountedSettingsView {
  root.innerHTML = `<section class="settings-panel" aria-label="设置">
    <header class="settings-header"><div><small class="settings-kicker">CODEX</small><h1 class="settings-title"></h1></div><div class="settings-actions"><button class="close-button settings-language" type="button"></button><button class="close-button settings-back" type="button"></button></div></header>
    <section class="settings-section"><div class="settings-section-title"><strong class="appearance-label"></strong><small class="choose-theme-label"></small></div><div class="theme-grid"></div><button class="settings-reset-theme" type="button"></button></section>
    <section class="settings-section"><strong class="behavior-label"></strong><label class="settings-switch"><span><b class="autostart-label"></b><small class="autostart-hint"></small></span><input class="autostart-input" type="checkbox"></label><label class="settings-switch"><span><b class="always-on-top-label"></b><small class="always-on-top-hint"></small></span><input class="always-on-top-input" type="checkbox"></label></section>
    <section class="settings-section"><strong class="updates-label"></strong><div class="settings-row"><span><b class="check-updates-label"></b><small class="update-status"></small></span><button class="settings-update-button" type="button"></button></div></section>
    <section class="settings-section settings-about"><strong class="about-label"></strong><a href="https://github.com/nf2huochao/Codex-QuotaOrb" target="_blank" rel="noreferrer"><span class="github-label"></span><small>github.com/nf2huochao/Codex-QuotaOrb</small></a><a href="mailto:huochao1210@gmail.com"><span class="contact-label"></span><small>huochao1210@gmail.com</small></a><small class="license-label"></small><small class="privacy-label"></small></section>
  </section>`

  const panel = root.querySelector<HTMLElement>('.settings-panel')!
  const themeGrid = root.querySelector<HTMLElement>('.theme-grid')!
  const languageButton = root.querySelector<HTMLButtonElement>('.settings-language')!
  const updateButton = root.querySelector<HTMLButtonElement>('.settings-update-button')!
  const updateStatus = root.querySelector<HTMLElement>('.update-status')!
  const autostartInput = root.querySelector<HTMLInputElement>('.autostart-input')!
  const alwaysOnTopInput = root.querySelector<HTMLInputElement>('.always-on-top-input')!
  let language = getLanguage()

  const renderThemes = () => {
    const active = getTheme()
    themeGrid.innerHTML = THEME_PRESETS.map((preset) => `<button class="theme-option${preset.id === active ? ' is-selected' : ''}" type="button" data-theme-id="${preset.id}" aria-pressed="${preset.id === active}"><span class="theme-swatches"><i style="--swatch:${preset.colors[0]}"></i><i style="--swatch:${preset.colors[1]}"></i><i style="--swatch:${preset.colors[2]}"></i></span><span>${esc(t(THEME_LABELS[preset.id], language))}</span></button>`).join('')
    themeGrid.querySelectorAll<HTMLButtonElement>('[data-theme-id]').forEach((button) => button.addEventListener('click', () => {
      setTheme(button.dataset.themeId as ThemeId)
      renderThemes()
    }))
  }

  const renderCopy = () => {
    panel.setAttribute('aria-label', t('settings', language))
    root.querySelector<HTMLElement>('.settings-kicker')!.textContent = 'CODEX'
    root.querySelector<HTMLElement>('.settings-title')!.textContent = t('settings', language)
    root.querySelector<HTMLButtonElement>('.settings-back')!.textContent = t('back', language)
    languageButton.textContent = language === 'zh-CN' ? 'EN' : '中'
    languageButton.setAttribute('aria-label', language === 'zh-CN' ? t('switchToEnglish', language) : t('switchToChinese', language))
    root.querySelector<HTMLElement>('.appearance-label')!.textContent = t('appearance', language)
    root.querySelector<HTMLElement>('.choose-theme-label')!.textContent = t('chooseTheme', language)
    root.querySelector<HTMLButtonElement>('.settings-reset-theme')!.textContent = t('restoreDefaults', language)
    root.querySelector<HTMLElement>('.behavior-label')!.textContent = t('behavior', language)
    root.querySelector<HTMLElement>('.autostart-label')!.textContent = t('autostart', language)
    root.querySelector<HTMLElement>('.autostart-hint')!.textContent = t('autostartHint', language)
    root.querySelector<HTMLElement>('.always-on-top-label')!.textContent = t('alwaysOnTop', language)
    root.querySelector<HTMLElement>('.always-on-top-hint')!.textContent = t('alwaysOnTopHint', language)
    root.querySelector<HTMLElement>('.updates-label')!.textContent = t('updates', language)
    root.querySelector<HTMLElement>('.check-updates-label')!.textContent = t('checkUpdates', language)
    updateButton.textContent = t('checkUpdates', language)
    root.querySelector<HTMLElement>('.about-label')!.textContent = t('about', language)
    root.querySelector<HTMLElement>('.github-label')!.textContent = t('githubRepo', language)
    root.querySelector<HTMLElement>('.contact-label')!.textContent = t('contactMe', language)
    root.querySelector<HTMLElement>('.license-label')!.textContent = t('license', language)
    root.querySelector<HTMLElement>('.privacy-label')!.textContent = t('privacyNote', language)
    renderThemes()
  }

  languageButton.addEventListener('click', () => { toggleLanguage() })
  root.querySelector<HTMLButtonElement>('.settings-back')!.addEventListener('click', actions.onClose)
  root.querySelector<HTMLButtonElement>('.settings-reset-theme')!.addEventListener('click', () => { setTheme('sage'); renderThemes() })
  autostartInput.addEventListener('change', () => { void actions.setAutostart(autostartInput.checked).catch(() => { autostartInput.checked = !autostartInput.checked }) })
  alwaysOnTopInput.addEventListener('change', () => { void actions.setAlwaysOnTop(alwaysOnTopInput.checked).catch(() => { alwaysOnTopInput.checked = !alwaysOnTopInput.checked }) })
  updateButton.addEventListener('click', async () => {
    updateButton.disabled = true
    updateStatus.textContent = t('checkingUpdates', language)
    try { await actions.onCheckUpdates() } finally { updateButton.disabled = false; updateStatus.textContent = '' }
  })
  window.addEventListener(THEME_EVENT, renderThemes)
  renderCopy()
  void actions.getAutostart().then((value) => { autostartInput.checked = value }).catch(() => undefined)
  void actions.getAlwaysOnTop().then((value) => { alwaysOnTopInput.checked = value }).catch(() => undefined)

  return {
    update(_snapshot: Snapshot) {},
    setLanguage(value) { language = value; renderCopy() },
    setAutostart(value) { autostartInput.checked = value },
    setAlwaysOnTop(value) { alwaysOnTopInput.checked = value },
    setRefreshing(_value: boolean) {},
    destroy() { window.removeEventListener(THEME_EVENT, renderThemes); root.replaceChildren() },
  }
}
