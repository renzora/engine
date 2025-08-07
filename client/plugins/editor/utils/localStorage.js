// UI Settings localStorage utility
const UI_SETTINGS_KEY = 'engine-ui-settings';

// Default UI settings
const defaultUISettings = {
  panels: {
    rightPanelWidth: 304, // 256 + 48 for toolbar
    bottomPanelHeight: 256, // Main bottom asset panel height
    scenePropertiesHeight: 300, // Scene panel properties section height
    assetsLibraryWidth: 250,
    rightPropertiesMenuPosition: 'right', // 'right' | 'bottom'
  },
  settings: {
    gridSettings: {
      enabled: true,
      size: 100,
      cellSize: 1,
      cellThickness: 1.0, // Increased from 0.5 for better visibility and less aliasing
      cellColor: '#6B7280',
      sectionSize: 10,
      sectionThickness: 2.0, // Increased from 1 for clearer section lines
      sectionColor: '#9CA3AF',
      position: [0, -1, 0],
      fadeDistance: 50, // Reduced from 100 to keep lines crisp longer
      fadeStrength: 0.5, // Reduced from 1 for more gradual fade
      infiniteGrid: true
    },
    viewportSettings: {
      backgroundColor: '#1a202c'
    }
  },
  bottomTabs: {
    selectedTab: 'assets', // 'assets' | 'console' | 'timeline' | etc
    tabOrder: [
      'assets', 'scripts', 'animation', 'node-editor', 'timeline', 'console',
      'materials', 'terrain', 'lighting', 'physics', 'audio', 'effects'
    ]
  },
  toolbar: {
    selectedTool: 'scene',
    tabOrder: [
      'scene', 'light', 'effects', 'folder', 'star', 'wifi', 'cloud', 'monitor'
    ],
    bottomTabOrder: [
      'add', 'settings', 'fullscreen'
    ]
  },
  topLeftMenu: {
    selectedItem: null // Will store the selected top-left menu item
  }
};

// Load UI settings from localStorage
export const loadUISettings = () => {
  try {
    const stored = localStorage.getItem(UI_SETTINGS_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      // Merge with defaults to ensure all properties exist
      return mergeDeep(defaultUISettings, parsed);
    }
  } catch (error) {
    console.warn('Failed to load UI settings from localStorage:', error);
  }
  return defaultUISettings;
};

// Save UI settings to localStorage
export const saveUISettings = (settings) => {
  try {
    localStorage.setItem(UI_SETTINGS_KEY, JSON.stringify(settings));
  } catch (error) {
    console.warn('Failed to save UI settings to localStorage:', error);
  }
};

// Update specific UI setting
export const updateUISetting = (path, value) => {
  const settings = loadUISettings();
  const updated = setNestedProperty(settings, path, value);
  saveUISettings(updated);
  return updated;
};

// Get specific UI setting
export const getUISetting = (path, fallback = null) => {
  const settings = loadUISettings();
  return getNestedProperty(settings, path) ?? fallback;
};

// Deep merge utility function
const mergeDeep = (target, source) => {
  const output = { ...target };
  
  if (isObject(target) && isObject(source)) {
    Object.keys(source).forEach(key => {
      if (isObject(source[key])) {
        if (!(key in target)) {
          output[key] = source[key];
        } else {
          output[key] = mergeDeep(target[key], source[key]);
        }
      } else {
        output[key] = source[key];
      }
    });
  }
  
  return output;
};

// Check if value is an object
const isObject = (item) => {
  return item && typeof item === 'object' && !Array.isArray(item);
};

// Set nested property using dot notation (e.g., 'panels.rightPanelWidth')
const setNestedProperty = (obj, path, value) => {
  const keys = path.split('.');
  const result = { ...obj };
  let current = result;
  
  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (!(key in current) || !isObject(current[key])) {
      current[key] = {};
    } else {
      current[key] = { ...current[key] };
    }
    current = current[key];
  }
  
  current[keys[keys.length - 1]] = value;
  return result;
};

// Get nested property using dot notation
const getNestedProperty = (obj, path) => {
  return path.split('.').reduce((current, key) => {
    return current && current[key] !== undefined ? current[key] : undefined;
  }, obj);
};

// Clear all UI settings (reset to defaults)
export const clearUISettings = () => {
  try {
    localStorage.removeItem(UI_SETTINGS_KEY);
  } catch (error) {
    console.warn('Failed to clear UI settings from localStorage:', error);
  }
  return defaultUISettings;
};

// Export default settings for reference
export { defaultUISettings };