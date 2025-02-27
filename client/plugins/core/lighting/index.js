lighting = {
  config: {
    dayStart: 7,
    sunsetStart: 19.5,
    nightStart: 22,
    nightEnd: 5,
    sunriseEnd: 7
  },
  lights: [],
  lightsActive: true,
  nightFilterActive: true,
  nightFilter: {
    dayColor: { r: 255, g: 255, b: 255 },
    nightColor: { r: 0, g: 0, b: 155 },
    compositeOperation: 'multiply',
    brightness: 2.0,
    saturation: 2.0,
    dayBrightness: 1.0,    // Add these new properties
    daySaturation: 1.0,    // for daytime filter strength
    manualColor: { r: 0, g: 0, b: 155 }
  },

  colorSchemes: {
    normal: {
        dayColor: { r: 255, g: 255, b: 255 },
        nightColor: { r: 0, g: 0, b: 155 },
        dayBrightness: 1.0,
        daySaturation: 1.0
    },
    dystopian: {
        dayColor: { r: 140, g: 110, b: 90 },
        nightColor: { r: 0, g: 0, b: 155 },
        dayBrightness: 2,
        daySaturation: 3
    }
},
  currentScheme: 'dystopian',
  timeBasedUpdatesEnabled: true,
  useManualRGB: true,
  lightIntensityMultiplier: 1,
  lastBaseNightFilterColor: null,
  lastProcessedNightFilterColor: null,

  setColorScheme(schemeName) {
    if (this.colorSchemes[schemeName]) {
      this.currentScheme = schemeName;
      const scheme = this.colorSchemes[schemeName];
      this.nightFilter.dayColor = scheme.dayColor;
      this.nightFilter.nightColor = scheme.nightColor;
    }
  },

  addLight(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05, shape = null) {
    if (!this.lightsActive) return;
    const existingLight = this.lights.find(light => light.id === id);
    if (!existingLight) {
      const clampedMaxIntensity = Math.min(Math.max(maxIntensity, 0), 1);
      const newLight = {
        id,
        x,
        y,
        baseRadius: radius,
        color,
        maxIntensity: clampedMaxIntensity,
        initialMaxIntensity: clampedMaxIntensity,
        type,
        currentIntensity: clampedMaxIntensity,
        flicker,
        flickerSpeed,
        flickerAmount,
        flickerOffset: Math.random() * 1000,
        shape
      };
      this.lights.push(newLight);
    }
  },

  clearLightsAndEffects() {
    const playerLight = this.lights.find(light => light.id === game.playerid + '_light');
    this.lights = [];
    if (plugin.exists('particles')) {
      particles.activeEffects = {};
      particles.particles = [];
    }
    if (playerLight) {
      this.lights.push(playerLight);
    }
  },

  unmount() {
  },

  onRender() {
    this.updateDayNightCycle();
    this.updateLights();
    this.renderLights();
    if (!this.nightFilterActive) return;
    const { maskCanvas, maskCtx } = this.createBaseNightFilter();
    this.renderLightsOnFilter(maskCtx);
    this.renderFinalOverlay(game.ctx, maskCanvas);
  },

  renderLights() {
    this.lights.forEach(light => {
      const inViewport = (
        light.x >= game.viewportXStart * 16 &&
        light.x < game.viewportXEnd * 16 &&
        light.y >= game.viewportYStart * 16 &&
        light.y < game.viewportYEnd * 16
      );

      if (!inViewport) {
        this.lights = this.lights.filter(l => l.id !== light.id);
      }
    });
  },

  updateLights() {
    if (!this.lightsActive) return;
    const applyFlicker = true;
    this.lights.forEach(light => {
      if (light.flicker && applyFlicker) {
        const flickerDelta = Math.sin((performance.now() + light.flickerOffset) * light.flickerSpeed) * light.flickerAmount;
        light.currentIntensity = Math.max(0, Math.min(light.initialMaxIntensity + flickerDelta, light.initialMaxIntensity));
      }
    });
  },

  updateDayNightCycle() {
    if (!plugin.exists('time') || !this.timeBasedUpdatesEnabled) return;
    const hours = time.hours;
    const minutes = time.minutes;
    let currentTime = hours + minutes / 60;
    if (currentTime >= 24) currentTime -= 24;
    const { dayStart, sunsetStart, nightStart, nightEnd, sunriseEnd } = this.config;
    let t = 0;
    if (currentTime >= dayStart && currentTime < sunsetStart) {
      t = 0;
    } else if (currentTime >= sunsetStart && currentTime < nightStart) {
      t = (currentTime - sunsetStart) / (nightStart - sunsetStart);
    } else if ((currentTime >= nightStart && currentTime < 24) || currentTime < nightEnd) {
      t = 1;
    } else if (currentTime >= nightEnd && currentTime < sunriseEnd) {
      t = 1 - ((currentTime - nightEnd) / (sunriseEnd - nightEnd));
    } else {
      t = 0;
    }
    this.lightIntensityMultiplier = t;
  },

  createBaseNightFilter() {
    const scheme = this.colorSchemes[this.currentScheme];
    const { brightness, saturation, dayBrightness, daySaturation } = this.nightFilter;
    
    let newColor;
    let useNightSettings = true;

    if (this.timeBasedUpdatesEnabled) {
      const t = this.lightIntensityMultiplier;
      if (t === 0) {
        // It's daytime - use day settings
        useNightSettings = false;
        newColor = {...scheme.dayColor};
      } else {
        newColor = this.lerpColor(scheme.dayColor, scheme.nightColor, t);
      }
    } else {
      newColor = scheme.dayColor;
      useNightSettings = false;
    }

    let finalColor;
    if (
      this.lastBaseNightFilterColor &&
      this.lastBaseNightFilterColor.r === newColor.r &&
      this.lastBaseNightFilterColor.g === newColor.g &&
      this.lastBaseNightFilterColor.b === newColor.b
    ) {
      finalColor = this.lastProcessedNightFilterColor;
    } else {
      // Use appropriate brightness/saturation based on time of day
      const useBrightness = useNightSettings ? brightness : (scheme.dayBrightness || dayBrightness);
      const useSaturation = useNightSettings ? saturation : (scheme.daySaturation || daySaturation);
      
      finalColor = this.applyBrightnessSaturation(newColor, useBrightness, useSaturation);
      this.lastBaseNightFilterColor = { ...newColor };
      this.lastProcessedNightFilterColor = { ...finalColor };
    }

    const maskCanvas = document.createElement('canvas');
    maskCanvas.width = game.canvas.width;
    maskCanvas.height = game.canvas.height;
    const maskCtx = maskCanvas.getContext('2d');
    maskCtx.fillStyle = `rgb(${finalColor.r}, ${finalColor.g}, ${finalColor.b})`;
    maskCtx.fillRect(0, 0, maskCanvas.width, maskCanvas.height);
    return { maskCanvas, maskCtx };
  },

  applyBrightnessSaturation(color, brightness, saturation) {
    let r = color.r / 255;
    let g = color.g / 255;
    let b = color.b / 255;
    let max = Math.max(r, g, b), min = Math.min(r, g, b);
    let h, s, l = (max + min) / 2;
    if (max === min) {
      h = s = 0;
    } else {
      let d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      switch (max) {
        case r: h = (g - b) / d + (g < b ? 6 : 0); break;
        case g: h = (b - r) / d + 2; break;
        case b: h = (r - g) / d + 4; break;
      }
      h /= 6;
    }
    l = l * brightness;
    s = s * saturation;
    function hue2rgb(p, q, t) {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1/6) return p + (q - p) * 6 * t;
      if (t < 1/2) return q;
      if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
      return p;
    }
    let q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    let p = 2 * l - q;
    r = hue2rgb(p, q, h + 1/3);
    g = hue2rgb(p, q, h);
    b = hue2rgb(p, q, h - 1/3);
    return {
      r: Math.round(r * 255),
      g: Math.round(g * 255),
      b: Math.round(b * 255)
    };
  },

  renderLightsOnFilter(maskCtx) {
    this.lights.forEach(light => {
      if (light.currentIntensity > 0) {
        const screenX = (light.x - camera.cameraX) * game.zoomLevel;
        const screenY = (light.y - camera.cameraY) * game.zoomLevel;
        const screenRadius = light.baseRadius * game.zoomLevel;
        const gradient = maskCtx.createRadialGradient(
          screenX, screenY, 0,
          screenX, screenY, screenRadius
        );
        const { r, g, b } = light.color;
        const intensity = light.currentIntensity;
        gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${intensity})`);
        gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);
        maskCtx.globalCompositeOperation = 'lighter';
        maskCtx.beginPath();
        maskCtx.arc(screenX, screenY, screenRadius, 0, Math.PI * 2);
        maskCtx.fillStyle = gradient;
        maskCtx.fill();
      }
    });
  },

  renderFinalOverlay(ctx, maskCanvas) {
    game.ctx.save();
    game.ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = this.nightFilter.compositeOperation;
    ctx.drawImage(maskCanvas, 0, 0);
    game.ctx.restore();
  },

  lerp(a, b, t) {
    return a + (b - a) * t;
  },

  lerpColor(colorA, colorB, t) {
    return {
      r: Math.round(this.lerp(colorA.r, colorB.r, t)),
      g: Math.round(this.lerp(colorA.g, colorB.g, t)),
      b: Math.round(this.lerp(colorA.b, colorB.b, t))
    };
  }
};