import { createSignal } from 'solid-js';

// Camera settings store
const [cameraSettings, setCameraSettings] = createSignal({
  fov: 60,
  vignette: {
    enabled: false,
    amount: 0.5,
    color: [0, 0, 0]
  },
  nightColor: [0.1, 0.1, 0.15]
});

export const cameraActions = {
  setFOV: (fov) => {
    setCameraSettings(prev => ({ ...prev, fov }));
  },
  
  setVignetteEnabled: (enabled) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, enabled }
    }));
  },
  
  setVignetteAmount: (amount) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, amount }
    }));
  },
  
  setVignetteColor: (color) => {
    setCameraSettings(prev => ({
      ...prev,
      vignette: { ...prev.vignette, color }
    }));
  },
  
  setNightColor: (nightColor) => {
    setCameraSettings(prev => ({ ...prev, nightColor }));
  },
  
  resetToDefaults: () => {
    setCameraSettings({
      fov: 60,
      vignette: {
        enabled: false,
        amount: 0.5,
        color: [0, 0, 0]
      },
      nightColor: [0.1, 0.1, 0.15]
    });
  }
};

export { cameraSettings };