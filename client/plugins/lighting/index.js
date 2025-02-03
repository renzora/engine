lighting = {
  lights: [],
  lightsActive: true,
  nightFilterActive: true,
  nightFilter: {
    dayColor: { r: 255, g: 255, b: 255 },
    nightColor: { r: 0, g: 0, b: 155 },
    compositeOperation: 'multiply',
    brightness: 2.0,
    saturation: 2.0,
    manualColor: { r: 0, g: 0, b: 155 }
  },
  timeBasedUpdatesEnabled: true,
  useManualRGB: true,
  lightIntensityMultiplier: 0,
  lastBaseNightFilterColor: null,
  lastProcessedNightFilterColor: null,

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
      console.log('Preserved player light:', playerLight);
    }
  },

  unmount() {
  },

  onRender() {
    this.updateDayNightCycle();
    this.updateLights();
    this.renderLights();

    if (!this.nightFilterActive) return;
    if (this.timeBasedUpdatesEnabled && this.lightIntensityMultiplier === 0) {
      return;
    }

    const { maskCanvas, maskCtx } = this.createBaseNightFilter();
    this.renderLightsOnFilter(maskCtx);
    this.renderFinalOverlay(game.ctx, maskCanvas);
  },

  renderLights() {
    if (!game.roomData || !game.roomData.items) return;

    game.roomData.items.forEach(roomItem => {
      if (!roomItem || !roomItem.layer_id) return;

      const tileData = game.objectData[roomItem.id];
      if (!tileData || !tileData[0]) return;

      if (roomItem.visible === false && tileData[0].l && Array.isArray(tileData[0].l)) {
        tileData[0].l.forEach((lightConfig, lightIndex) => {
          const lId = `${roomItem.layer_id}_light_${lightIndex}`;
          this.lights = this.lights.filter(l => l.id !== lId);
        });
        return;
      }

      if (tileData[0].l && Array.isArray(tileData[0].l)) {
        tileData[0].l.forEach((lightConfig, lightIndex) => {
          const offsetX = lightConfig.x || 0;
          const offsetY = lightConfig.y || 0;
          const baseX = Math.min(...roomItem.x) * 16;
          const baseY = Math.min(...roomItem.y) * 16;
          const px = baseX + offsetX;
          const py = baseY + offsetY;
          const lId = `${roomItem.layer_id}_light_${lightIndex}`;
          const dh = plugin.time.hours + plugin.time.minutes / 60;
          const isNight = (dh >= 22 || dh < 7);
          const inViewport = (
            px + 200 >= game.viewportXStart * 16 &&
            px - 200 < game.viewportXEnd * 16 &&
            py + 200 >= game.viewportYStart * 16 &&
            py - 200 < game.viewportYEnd * 16
          );

          if (inViewport && isNight) {
            let existingLight = this.lights.find(l => l.id === lId);
            if (!existingLight) {
              const col = tileData[0].lc || { r: 255, g: 255, b: 255 };
              const intens = tileData[0].li || 1;
              const rad = tileData[0].lr || 200;
              const flickerSpeed = tileData[0].lfs || 0.03;
              const flickerAmount = tileData[0].lfa || 0.04;
              const lt = tileData[0].lt || 'lamp';
              const shp = lightConfig.shape || null;
              
              this.addLight(lId, px, py, rad, col, intens, lt, true,
                flickerSpeed, flickerAmount, shp);
            } else {
              existingLight.x = px;
              existingLight.y = py;
            }
          } else {
            this.lights = this.lights.filter(l => l.id !== lId);
          }
        });
      }
    });
  },

  updateLights() {
    if (!this.lightsActive) return;
    const applyFlicker = this.timeBasedUpdatesEnabled || !this.useManualRGB;

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
    const dayStart = 7;
    const sunsetStart = 19.5;
    const nightStart = 22;
    const nightEnd = 5;
    const sunriseEnd = 7;
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
    if (this.timeBasedUpdatesEnabled && this.lightIntensityMultiplier === 0) {
      const maskCanvas = document.createElement('canvas');
      maskCanvas.width = game.canvas.width;
      maskCanvas.height = game.canvas.height;
      const maskCtx = maskCanvas.getContext('2d');
      maskCtx.clearRect(0, 0, maskCanvas.width, maskCanvas.height);
      return { maskCanvas, maskCtx };
    }

    const { dayColor, nightColor, brightness, saturation, manualColor } = this.nightFilter;
    let newColor;

    if (this.timeBasedUpdatesEnabled) {
      const t = this.lightIntensityMultiplier;
      newColor = this.lerpColor(dayColor, nightColor, t);
    } else {
      if (this.useManualRGB) {
        newColor = { ...manualColor };
      } else {
        const t = this.lightIntensityMultiplier;
        newColor = this.lerpColor(dayColor, nightColor, t);
      }
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
      finalColor = this.applyBrightnessSaturation(newColor, brightness, saturation);
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