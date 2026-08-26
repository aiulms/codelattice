// 外观主题：只写 localStorage + documentElement，不进 Tauri / models.json。
export type ThemeName = "light" | "dark";

const THEME_KEY = "codelattice.theme";

export function readTheme(): ThemeName {
  try {
    return localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

export function applyTheme(theme: ThemeName): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* 隐私模式或配额不足时只改本次会话 */
  }
}
