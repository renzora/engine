import { createContext, useContext, useState, useEffect } from 'react';

const ThemeContext = createContext();

const defaultTheme = {
  colors: {
    primary: 'rgb(59 130 246)', // blue-500
    primaryHover: 'rgb(29 78 216)', // blue-700
    primaryRing: 'rgba(59, 130, 246, 0.5)', // blue-500/50
    
    background: {
      primary: 'rgb(30 41 59)', // slate-800
      secondary: 'rgb(15 23 42)', // slate-900
      hover: 'rgb(51 65 85)', // slate-700
      surface: 'rgba(30, 41, 59, 0.8)', // slate-800/80
      panel: 'linear-gradient(to bottom, rgba(30, 41, 59, 0.95), rgba(15, 23, 42, 0.98))',
    },
    
    border: {
      primary: 'rgb(71 85 105)', // slate-600
      secondary: 'rgba(71, 85, 105, 0.5)', // slate-600/50
      focus: 'rgb(59 130 246)', // blue-500
    },
    
    text: {
      primary: 'rgb(255 255 255)', // white
      secondary: 'rgb(203 213 225)', // slate-300
      muted: 'rgb(156 163 175)', // gray-400
      disabled: 'rgb(107 114 128)', // gray-500
    },
    
    semantic: {
      success: 'rgb(34 197 94)', // green-500
      warning: 'rgb(251 191 36)', // amber-400
      error: 'rgb(239 68 68)', // red-500
      info: 'rgb(59 130 246)', // blue-500
    }
  },
  
  spacing: {
    xs: '0.125rem', // 2px
    sm: '0.25rem',  // 4px
    md: '0.5rem',   // 8px
    lg: '0.75rem',  // 12px
    xl: '1rem',     // 16px
    '2xl': '1.5rem', // 24px
    '3xl': '2rem',   // 32px
  },
  
  borderRadius: {
    sm: '0.375rem', // 6px
    md: '0.5rem',   // 8px
    lg: '0.75rem',  // 12px
    xl: '1rem',     // 16px
  },
  
  shadows: {
    sm: '0 1px 2px 0 rgb(0 0 0 / 0.05)',
    md: '0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)',
    lg: '0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)',
    xl: '0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1)',
  },
  
  transitions: {
    fast: '0.15s',
    normal: '0.2s',
    slow: '0.3s',
  }
};

export const ThemeProvider = ({ children, theme = {} }) => {
  const [currentTheme, setCurrentTheme] = useState(() => ({
    ...defaultTheme,
    ...theme
  }));

  const updateTheme = (newTheme) => {
    setCurrentTheme(prev => ({
      ...prev,
      ...newTheme
    }));
  };

  const getCSSVariables = () => {
    const flattenObject = (obj, prefix = '') => {
      let result = {};
      for (const key in obj) {
        if (typeof obj[key] === 'object' && obj[key] !== null) {
          Object.assign(result, flattenObject(obj[key], `${prefix}${key}-`));
        } else {
          result[`--ui-${prefix}${key}`] = obj[key];
        }
      }
      return result;
    };
    
    return flattenObject(currentTheme);
  };

  useEffect(() => {
    const cssVars = getCSSVariables();
    const root = document.documentElement;
    
    Object.entries(cssVars).forEach(([key, value]) => {
      root.style.setProperty(key, value);
    });
    
    return () => {
      Object.keys(cssVars).forEach(key => {
        root.style.removeProperty(key);
      });
    };
  }, [currentTheme]);

  return (
    <ThemeContext.Provider value={{ theme: currentTheme, updateTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
};