import { createStore } from 'solid-js/store';

// Weather Store - centralized state management for weather effects
const [weatherStore, setWeatherStore] = createStore({
  rain: {
    enabled: false,
    intensity: 0.5,
    size: 1.0,
    windStrength: 0.2,
    color: [0.8, 0.9, 1.0] // RGB values
  },
  
  stars: {
    enabled: false,
    brightness: 0.8,
    density: 100,
    twinkle: true
  }
});

// Weather Actions
export const weatherActions = {
  // Rain controls
  setRainEnabled: (enabled) => {
    setWeatherStore('rain', 'enabled', enabled);
  },
  
  setRainIntensity: (intensity) => {
    setWeatherStore('rain', 'intensity', Math.max(0, Math.min(1, intensity)));
  },
  
  setRainSize: (size) => {
    setWeatherStore('rain', 'size', Math.max(0.1, Math.min(2, size)));
  },
  
  setRainWindStrength: (strength) => {
    setWeatherStore('rain', 'windStrength', Math.max(0, Math.min(1, strength)));
  },
  
  setRainColor: (color) => {
    setWeatherStore('rain', 'color', color);
  },
  
  resetRain: () => {
    setWeatherStore('rain', {
      enabled: false,
      intensity: 0.5,
      size: 1.0,
      windStrength: 0.2,
      color: [0.8, 0.9, 1.0]
    });
  },
  
  // Stars controls
  setStarsEnabled: (enabled) => {
    setWeatherStore('stars', 'enabled', enabled);
  },
  
  setStarsBrightness: (brightness) => {
    setWeatherStore('stars', 'brightness', Math.max(0, Math.min(1, brightness)));
  },
  
  setStarsDensity: (density) => {
    setWeatherStore('stars', 'density', Math.max(1, Math.min(1000, density)));
  },
  
  setStarsTwinkle: (twinkle) => {
    setWeatherStore('stars', 'twinkle', twinkle);
  }
};

export { weatherStore };