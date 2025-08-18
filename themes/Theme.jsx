import { createContext, useContext, createSignal, createEffect, onMount } from 'solid-js';

const themes = {
  dark: {
    '--background': '15 15 15',
    '--foreground': '245 245 245',
    '--panel': '25 25 25',
    '--panelSecondary': '30 30 30',
    '--surface': '35 35 35',
    '--surfaceHover': '40 40 40',
    '--textPrimary': '245 245 245',
    '--textSecondary': '156 163 175',
    '--textDisabled': '107 114 128',
    '--primary': '59 130 246',
    '--primaryHover': '37 99 235',
    '--border': '45 45 45',
    '--borderSurface': '55 55 55',
  },
  light: {
    '--background': '255 255 255',
    '--foreground': '15 15 15',
    '--panel': '249 250 251',
    '--panelSecondary': '243 244 246',
    '--surface': '248 250 252',
    '--surfaceHover': '241 245 249',
    '--textPrimary': '15 15 15',
    '--textSecondary': '75 85 99',
    '--textDisabled': '156 163 175',
    '--primary': '59 130 246',
    '--primaryHover': '37 99 235',
    '--border': '229 231 235',
    '--borderSurface': '203 213 225',
  }
};

const ThemeContext = createContext();

export function Theme(props) {
  const getSavedTheme = () => {
    if (typeof window !== 'undefined') {
      return localStorage.getItem('renzora-theme') || 'dark';
    }
    return 'dark';
  };
  
  const [currentTheme, setCurrentTheme] = createSignal(getSavedTheme());
  
  createEffect(() => {
    const themeName = currentTheme();
    const theme = themes[themeName];
    
    if (theme && typeof window !== 'undefined') {
      const root = document.documentElement;
      
      Object.entries(theme).forEach(([key, value]) => {
        root.style.setProperty(key, value);
      });
      
      localStorage.setItem('renzora-theme', themeName);
      root.setAttribute('data-theme', themeName);
    }
  });
  
  const toggleTheme = () => {
    setCurrentTheme(currentTheme() === 'dark' ? 'light' : 'dark');
  };
  
  const value = {
    theme: currentTheme,
    toggleTheme,
    setTheme: setCurrentTheme
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
    return {
      theme: () => 'dark',
      toggleTheme: () => {},
      setTheme: () => {}
    };
  }
  return context;
}
