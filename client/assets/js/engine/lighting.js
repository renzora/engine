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

    LightSource: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        this.id = id;
        this.x = x;
        this.y = y;
        this.baseRadius = radius;
        this.color = color;
        this.maxIntensity = maxIntensity;
        this.initialMaxIntensity = maxIntensity;
        this.type = type;
        this.currentIntensity = maxIntensity;
        this.flicker = flicker;
        this.flickerSpeed = flickerSpeed;
        this.flickerAmount = flickerAmount;
        this.flickerOffset = Math.random() * 1000;
    },

    addLight: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        if (!this.lightsActive) return;

        const existingLight = this.lights.find(light => light.id === id);
        if (!existingLight) {
            const clampedMaxIntensity = Math.min(Math.max(maxIntensity, 0), 1);
            const newLight = new this.LightSource(id, x, y, radius, color, clampedMaxIntensity, type, flicker, flickerSpeed, flickerAmount);
            newLight.currentIntensity = clampedMaxIntensity;
            this.lights.push(newLight);
        }
    },

    updateLights: function(deltaTime) {
        if (!this.lightsActive) return;
        const applyFlicker = this.timeBasedUpdatesEnabled || !this.useManualRGB;

        this.lights.forEach(light => {
            if (light.flicker && applyFlicker) {
                const flickerDelta = Math.sin((performance.now() + light.flickerOffset) * light.flickerSpeed) * light.flickerAmount;
                light.currentIntensity = Math.max(0, Math.min(light.initialMaxIntensity + flickerDelta, light.initialMaxIntensity));
            }
        });
    },

    clearLightsAndEffects: function() {
        console.log('Clearing lights and effects, preserving player light.');

        const playerLight = lighting.lights.find(light => light.id === game.playerid + '_light');

        this.lights = [];
        particles.activeEffects = {};
        game.particles = [];

        if (playerLight) {
            this.lights.push(playerLight);
            console.log('Preserved player light:', playerLight);
        }
    },

    lerp: function(a, b, t) {
        return a + (b - a) * t;
    },

    lerpColor: function(colorA, colorB, t) {
        return {
            r: Math.round(this.lerp(colorA.r, colorB.r, t)),
            g: Math.round(this.lerp(colorA.g, colorB.g, t)),
            b: Math.round(this.lerp(colorA.b, colorB.b, t)),
        };
    },

    updateDayNightCycle: function() {
        if (!this.timeBasedUpdatesEnabled) return;

        const hours = utils.gameTime.hours;
        const minutes = utils.gameTime.minutes;
        let time = hours + minutes / 60;
        if (time >= 24) time -= 24;

        const dayStart = 7;
        const sunsetStart = 18;
        const nightStart = 20;
        const nightEnd = 5;
        const sunriseEnd = 7;

        let t = 0;

        if (time >= dayStart && time < sunsetStart) {
            t = 0; // Day
            if(utils.pluginExists('weather_plugin')) {
                if(!weather_plugin.fireflys.overrideActive) {
                    weather_plugin.fireflys.active = false;
                }
            }
        } else if (time >= sunsetStart && time < nightStart) {
            t = (time - sunsetStart) / (nightStart - sunsetStart);
            if(utils.pluginExists('weather_plugin')) {
                if(!weather_plugin.fireflys.overrideActive) {
                    weather_plugin.fireflys.active = false;
                }
            }
        } else if (time >= nightStart || time < nightEnd) {
            t = 1; // Night
            if(utils.pluginExists('weather_plugin')) {
                if(!weather_plugin.fireflys.overrideActive) { 
                    weather_plugin.fireflys.active = true;
                }
            }
        } else if (time >= nightEnd && time < sunriseEnd) {
            t = 1 - ((time - nightEnd) / (sunriseEnd - nightEnd));
            if(utils.pluginExists('weather_plugin')) {
                if(!weather_plugin.fireflys.overrideActive) {
                    weather_plugin.fireflys.active = t > 0.5;
                }
            }
        } else {
            t = 0; // Default day
            if(utils.pluginExists('weather_plugin')) {
                if(!weather_plugin.fireflys.overrideActive) {
                    weather_plugin.fireflys.active = false;
                }
            }
        }

        this.lightIntensityMultiplier = t;
    },

    createBaseNightFilter: function() {
        // If time-based updates are enabled and we are at full day (t=0),
        // skip filter entirely: do not apply brightness/saturation or color.
        if (this.timeBasedUpdatesEnabled && this.lightIntensityMultiplier === 0) {
            // Return a transparent canvas
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

        // Check if the base RGB is different from last time
        let finalColor;
        if (
            this.lastBaseNightFilterColor &&
            this.lastBaseNightFilterColor.r === newColor.r &&
            this.lastBaseNightFilterColor.g === newColor.g &&
            this.lastBaseNightFilterColor.b === newColor.b
        ) {
            // The base color hasn't changed, reuse the last processed color
            finalColor = this.lastProcessedNightFilterColor;
        } else {
            // The base color changed, re-run brightness/saturation
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

    applyBrightnessSaturation: function(color, brightness, saturation) {
        console.log("apply brightness saturation");
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

    renderLightsOnFilter: function(maskCtx) {
        this.lights.forEach((light) => {
            if (light.currentIntensity > 0) {
                const screenX = (light.x - camera.cameraX) * game.zoomLevel;
                const screenY = (light.y - camera.cameraY) * game.zoomLevel;
                const screenRadius = light.baseRadius * game.zoomLevel;

                const gradient = maskCtx.createRadialGradient(
                    screenX,
                    screenY,
                    0,
                    screenX,
                    screenY,
                    screenRadius
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

    renderFinalOverlay: function(ctx, maskCanvas, maskCtx) {
        ctx.globalCompositeOperation = this.nightFilter.compositeOperation;
        ctx.drawImage(maskCanvas, 0, 0);
        ctx.restore();
    },

    renderNightFilter: function() {
        if (!this.nightFilterActive) return;
        // If it's full day (and time-based updates are enabled), skip rendering entirely
        if (this.timeBasedUpdatesEnabled && this.lightIntensityMultiplier === 0) {
            return;
        }

        const ctx = game.ctx;
        ctx.save();
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        const { maskCanvas, maskCtx } = this.createBaseNightFilter();
        this.renderLightsOnFilter(maskCtx);
        this.renderFinalOverlay(ctx, maskCanvas, maskCtx);
    }
};