import { createContext, useContext, createSignal, createEffect } from 'solid-js';
import { themes, applyTheme } from './themes';

const ThemeContext = createContext();

export function ThemeProvider(props) {
  const getInitialTheme = () => {
    if (typeof window !== 'undefined') {
      const saved = localStorage.getItem('renzora-theme');
      return saved && themes[saved] ? saved : 'dark';
    }
    return 'dark';
  };
  
  const [currentTheme, setCurrentTheme] = createSignal(getInitialTheme());
  const [themeConfig, setThemeConfig] = createSignal(themes[currentTheme()]);
  
  createEffect(() => {
    const themeName = currentTheme();
    const theme = themes[themeName];
    
    if (theme) {
      setThemeConfig(theme);
      if (theme.colors) {
        applyTheme(theme);
      }
      
      if (typeof window !== 'undefined') {
        localStorage.setItem('renzora-theme', themeName);
      }
    }
  });
  
  const switchTheme = (themeName) => {
    if (themes[themeName]) {
      setCurrentTheme(themeName);
    }
  };
  
  const toggleTheme = () => {
    const current = currentTheme();
    const next = current === 'dark' ? 'light' : 'dark';
    switchTheme(next);
  };
  
  const getColor = (colorName) => {
    const theme = themeConfig();
    return theme?.colors?.[colorName] || '';
  };
  
  const isDark = () => {
    return currentTheme() === 'dark' || currentTheme() === 'engine';
  };
  
  const value = {
    theme: currentTheme,
    themeConfig,
    switchTheme,
    toggleTheme,
    getColor,
    isDark,
    availableThemes: Object.keys(themes)
  };
  
  return (
    <ThemeContext.Provider value={value}>
      {props.children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
