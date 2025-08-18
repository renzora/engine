export const componentThemes = {
  dark: {
    Button: {
      backgroundColor: '#3b82f6',
      color: '#ffffff',
      borderColor: '#3b82f6',
      hover: {
        backgroundColor: '#2563eb',
        borderColor: '#2563eb',
      },
      focus: {
        ringColor: '#3b82f6',
      }
    },

    Card: {
      base: {
        backgroundColor: '#1f2937',
        borderColor: '#374151',
        color: '#f3f4f6',
        shadow: '0 4px 6px -1px rgba(0, 0, 0, 0.1)',
      }
    },

    Input: {
      base: {
        backgroundColor: '#374151',
        borderColor: '#4b5563',
        color: '#f3f4f6',
        placeholderColor: '#9ca3af',
        focus: {
          borderColor: '#3b82f6',
          ringColor: '#3b82f6',
        }
      }
    },

    TreeView: {
      base: {
        backgroundColor: '#1f2937',
        borderColor: '#374151',
      },
      item: {
        backgroundColor: 'transparent',
        color: '#f3f4f6',
        hover: {
          backgroundColor: '#374151',
        },
        selected: {
          backgroundColor: '#3b82f6',
          color: '#ffffff',
        }
      },
      expand: {
        color: '#9ca3af',
        hover: {
          color: '#f3f4f6',
        }
      }
    },

    Panel: {
      base: {
        backgroundColor: '#1f2937',
        borderColor: '#374151',
      },
      header: {
        backgroundColor: '#111827',
        borderColor: '#374151',
        color: '#f3f4f6',
      }
    }
  },

  light: {
    Button: {
      backgroundColor: '#3b82f6',
      color: '#ffffff',
      borderColor: '#3b82f6',
      hover: {
        backgroundColor: '#2563eb',
        borderColor: '#2563eb',
      },
      focus: {
        ringColor: '#3b82f6',
      }
    },

    Card: {
      base: {
        backgroundColor: '#ffffff',
        borderColor: '#e5e7eb',
        color: '#1f2937',
        shadow: '0 4px 6px -1px rgba(0, 0, 0, 0.1)',
      }
    },

    Input: {
      base: {
        backgroundColor: '#ffffff',
        borderColor: '#d1d5db',
        color: '#1f2937',
        placeholderColor: '#6b7280',
        focus: {
          borderColor: '#3b82f6',
          ringColor: '#3b82f6',
        }
      }
    },

    TreeView: {
      base: {
        backgroundColor: '#ffffff',
        borderColor: '#e5e7eb',
      },
      item: {
        backgroundColor: 'transparent',
        color: '#1f2937',
        hover: {
          backgroundColor: '#f3f4f6',
        },
        selected: {
          backgroundColor: '#3b82f6',
          color: '#ffffff',
        }
      },
      expand: {
        color: '#6b7280',
        hover: {
          color: '#1f2937',
        }
      }
    },

    Panel: {
      base: {
        backgroundColor: '#ffffff',
        borderColor: '#e5e7eb',
      },
      header: {
        backgroundColor: '#f9fafb',
        borderColor: '#e5e7eb',
        color: '#1f2937',
      }
    }
  },

  engine: {
    Button: {
      backgroundColor: '#0ea5e9',
      color: '#ffffff',
      borderColor: '#0ea5e9',
      hover: {
        backgroundColor: '#0284c7',
        borderColor: '#0284c7',
      }
    },

    Card: {
      base: {
        backgroundColor: '#171717',
        borderColor: '#404040',
        color: '#e5e5e5',
        shadow: '0 4px 6px -1px rgba(0, 0, 0, 0.3)',
      }
    },

    Input: {
      base: {
        backgroundColor: '#262626',
        borderColor: '#404040',
        color: '#e5e5e5',
        placeholderColor: '#737373',
        focus: {
          borderColor: '#0ea5e9',
          ringColor: '#0ea5e9',
        }
      }
    },

    TreeView: {
      base: {
        backgroundColor: '#171717',
        borderColor: '#404040',
      },
      item: {
        backgroundColor: 'transparent',
        color: '#e5e5e5',
        hover: {
          backgroundColor: '#262626',
        },
        selected: {
          backgroundColor: '#0ea5e9',
          color: '#ffffff',
        }
      }
    }
  }
};

export function getComponentTheme(themeName, componentName, variant = 'base') {
  const theme = componentThemes[themeName] || componentThemes.dark;
  const component = theme[componentName];
  
  if (!component) {
    console.warn(`No theme found for component: ${componentName}`);
    return {};
  }
  
  return component[variant] || component.base || {};
}