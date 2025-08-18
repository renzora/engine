import { useTheme } from '../../../themes/Theme.jsx';
import { cn } from '../utils/cn.js';

/**
 * Hook that provides themed CSS classes based on the current theme
 * Uses the theme system to provide consistent theming across components
 */
export function useThemedClasses() {
  const { theme } = useTheme();
  
  const getThemedClasses = () => {
    return {
      // Background classes
      bg: {
        primary: `bg-[rgb(var(--background))]`,
        panel: `bg-[rgb(var(--panel))]`,
        panelSecondary: `bg-[rgb(var(--panelSecondary))]`,
        surface: `bg-[rgb(var(--surface))]`,
        surfaceHover: `bg-[rgb(var(--surfaceHover))]`,
      },
      
      // Text classes
      text: {
        primary: `text-[rgb(var(--textPrimary))]`,
        secondary: `text-[rgb(var(--textSecondary))]`,
        disabled: `text-[rgb(var(--textDisabled))]`,
        inverse: `text-[rgb(var(--textInverse))]`,
      },
      
      // Border classes
      border: {
        panel: `border-[rgb(var(--panelBorder))]`,
        surface: `border-[rgb(var(--surfaceBorder))]`,
        input: `border-[rgb(var(--inputBorder))]`,
        inputFocus: `border-[rgb(var(--inputBorderFocus))]`,
      },
      
      // Button classes
      button: {
        primary: `bg-[rgb(var(--primary))] hover:bg-[rgb(var(--primaryHover))] text-[rgb(var(--primaryForeground))]`,
        secondary: `bg-[rgb(var(--secondary))] hover:bg-[rgb(var(--secondaryHover))] text-[rgb(var(--secondaryForeground))]`,
        accent: `bg-[rgb(var(--accent))] hover:bg-[rgb(var(--accentHover))] text-[rgb(var(--accentForeground))]`,
        danger: `bg-[rgb(var(--danger))] hover:bg-[rgb(var(--dangerHover))] text-white`,
        ghost: `text-[rgb(var(--textSecondary))] hover:text-[rgb(var(--textPrimary))] hover:bg-[rgb(var(--surfaceHover))]`,
        outline: `border border-[rgb(var(--surfaceBorder))] hover:border-[rgb(var(--inputBorderFocus))] text-[rgb(var(--textPrimary))] hover:bg-[rgb(var(--surfaceHover))]`,
      },
      
      // Input classes
      input: {
        base: `bg-[rgb(var(--inputBg))] border-[rgb(var(--inputBorder))] text-[rgb(var(--inputText))] placeholder:text-[rgb(var(--inputPlaceholder))] focus:border-[rgb(var(--inputBorderFocus))]`,
      },
      
      // Scrollbar classes
      scrollbar: {
        base: `scrollbar-thumb-[rgb(var(--scrollbar))] scrollbar-track-transparent hover:scrollbar-thumb-[rgb(var(--scrollbarHover))]`,
      },
      
      // Utility classes
      utils: {
        shadow: {
          sm: `shadow-[0_1px_2px_0_rgba(var(--shadowSm))]`,
          md: `shadow-[0_4px_6px_-1px_rgba(var(--shadowMd))]`,
          lg: `shadow-[0_10px_15px_-3px_rgba(var(--shadowLg))]`,
        },
        selection: `selection:bg-[rgb(var(--selection))]`,
        highlight: `bg-[rgb(var(--highlight))]`,
      }
    };
  };
  
  const themed = getThemedClasses();
  
  // Helper function to combine themed classes with additional classes
  const combine = (...classes) => cn(classes);
  
  return {
    themed,
    combine,
    theme: theme()
  };
}

export default useThemedClasses;