import { useTheme } from '../../../themes/Theme.jsx';
import { getComponentTheme } from '../../../themes/componentThemes.js';

export function useComponentTheme(componentName, variant = 'base') {
  const { theme } = useTheme();
  const themeStyles = getComponentTheme(theme(), componentName, variant);
  
  const getStyles = (state = 'default') => {
    const baseStyles = { ...themeStyles };
    const stateStyles = baseStyles[state] || {};
    Object.keys(baseStyles).forEach(key => {
      if (typeof baseStyles[key] === 'object') {
        delete baseStyles[key];
      }
    });
    
    return state === 'default' ? baseStyles : { ...baseStyles, ...stateStyles };
  };
  
  return {
    styles: getStyles(),
    getStyles,
    hoverStyles: getStyles('hover'),
    focusStyles: getStyles('focus'),
    activeStyles: getStyles('active'),
    style: getStyles(),
    onMouseEnter: (e) => {
      const hoverStyles = getStyles('hover');
      Object.assign(e.target.style, hoverStyles);
    },
    onMouseLeave: (e) => {
      const defaultStyles = getStyles();
      Object.assign(e.target.style, defaultStyles);
    },
    onFocus: (e) => {
      const focusStyles = getStyles('focus');
      Object.assign(e.target.style, focusStyles);
    },
    onBlur: (e) => {
      const defaultStyles = getStyles();
      Object.assign(e.target.style, defaultStyles);
    }
  };
}