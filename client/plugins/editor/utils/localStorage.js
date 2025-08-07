const UI_SETTINGS_KEY = 'engine-ui-settings';

const defaultUISettings = {
  panels: {
    rightPanelWidth: 304,
    bottomPanelHeight: 256,
    scenePropertiesHeight: 300,
    assetsLibraryWidth: 250,
    rightPropertiesMenuPosition: 'right',
  },
  settings: {
    gridSettings: {
      enabled: true,
      size: 100,
      cellSize: 1,
      cellThickness: 1.0,
      cellColor: '#6B7280',
      sectionSize: 10,
      sectionThickness: 2.0,
      sectionColor: '#9CA3AF',
      position: [0, -1, 0],
      fadeDistance: 50,
      fadeStrength: 0.5,
      infiniteGrid: true
    },
    viewportSettings: {
      backgroundColor: '#1a202c'
    }
  },
  bottomTabs: {
    selectedTab: 'assets',
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
    selectedItem: null
  }
};

export const loadUISettings = () => {
  try {
    const stored = localStorage.getItem(UI_SETTINGS_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      return mergeDeep(defaultUISettings, parsed);
    }
  } catch (error) {
    console.warn('Failed to load UI settings from localStorage:', error);
  }
  return defaultUISettings;
};

export const saveUISettings = (settings) => {
  try {
    localStorage.setItem(UI_SETTINGS_KEY, JSON.stringify(settings));
  } catch (error) {
    console.warn('Failed to save UI settings to localStorage:', error);
  }
};

export const updateUISetting = (path, value) => {
  const settings = loadUISettings();
  const updated = setNestedProperty(settings, path, value);
  saveUISettings(updated);
  return updated;
};

export const getUISetting = (path, fallback = null) => {
  const settings = loadUISettings();
  return getNestedProperty(settings, path) ?? fallback;
};

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

const isObject = (item) => {
  return item && typeof item === 'object' && !Array.isArray(item);
};

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

const getNestedProperty = (obj, path) => {
  return path.split('.').reduce((current, key) => {
    return current && current[key] !== undefined ? current[key] : undefined;
  }, obj);
};

export const clearUISettings = () => {
  try {
    localStorage.removeItem(UI_SETTINGS_KEY);
  } catch (error) {
    console.warn('Failed to clear UI settings from localStorage:', error);
  }
  return defaultUISettings;
};

export { defaultUISettings };