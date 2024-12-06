const lighting = {
    lights: [],
    nightFilter: {
        opacity: 0, // Start with day opacity
        dayColor: { r: 255, g: 255, b: 255 }, // Day color (white)
        nightColor: { r: 13, g: 0, b: 101 }, // Night color
        compositeOperation: 'hard-light', // Use 'multiply' blending mode
    },
    timeBasedUpdatesEnabled: true,
    nightAmbiencePlaying: false,
    lightIntensityMultiplier: 0, // Multiplier for light intensities based on time of day

    LightSource: function (id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
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

    addLight: function (id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        const existingLight = this.lights.find(light => light.id === id);
        if (!existingLight) {
            // Ensure maxIntensity is between 0 and 1
            const clampedMaxIntensity = Math.min(Math.max(maxIntensity, 0), 1);
            const newLight = new this.LightSource(id, x, y, radius, color, clampedMaxIntensity, type, flicker, flickerSpeed, flickerAmount);
            newLight.currentIntensity = clampedMaxIntensity;
            this.lights.push(newLight);
        }
    },
    
    updateLights: function (deltaTime) {
        this.lights.forEach(light => {
            // Apply time-based flicker if enabled
            if (light.flicker) {
                const flickerDelta = Math.sin((performance.now() + light.flickerOffset) * light.flickerSpeed) * light.flickerAmount;
                light.currentIntensity = Math.max(0, Math.min(light.initialMaxIntensity + flickerDelta, light.initialMaxIntensity));
            }
        });
    },

    clearLightsAndEffects: function () {
        console.log('Clearing lights and effects, preserving player light.');

        // Preserve the player's light
        const playerLight = lighting.lights.find(light => light.id === game.playerid + '_light');

        this.lights = [];
        particles.activeEffects = {};
        game.particles = [];

        // Re-add the preserved player light
        if (playerLight) {
            this.lights.push(playerLight);
            console.log('Preserved player light:', playerLight);
        }
    },

    updateDayNightCycle: function () {
        if (!this.timeBasedUpdatesEnabled) return;

        const hours = utils.gameTime.hours;
        const minutes = utils.gameTime.minutes;
        let time = hours + minutes / 60;

        // Adjust time to be within 0-24 range
        if (time >= 24) time -= 24;

        // Define time periods
        const dayStart = 7;
        const sunsetStart = 18;
        const nightStart = 20;
        const nightEnd = 5;
        const sunriseEnd = 7;

        // Determine opacity, color, and light intensity based on time
        if (time >= dayStart && time < sunsetStart) {
            // Daytime
            this.nightFilter.opacity = 0;
            this.nightFilter.color = { ...this.nightFilter.dayColor }; // Use dayColor
            this.lightIntensityMultiplier = 0; // Lights are off during the day
        } else if (time >= sunsetStart && time < nightStart) {
            // Sunset: 18:00 to 20:00
            const progress = (time - sunsetStart) / (nightStart - sunsetStart); // 0 to 1
            this.nightFilter.opacity = progress; // Opacity increases from 0 to 1

            // Interpolate color from dayColor to nightColor
            this.nightFilter.color = {
                r: Math.round(this.nightFilter.dayColor.r + (this.nightFilter.nightColor.r - this.nightFilter.dayColor.r) * progress),
                g: Math.round(this.nightFilter.dayColor.g + (this.nightFilter.nightColor.g - this.nightFilter.dayColor.g) * progress),
                b: Math.round(this.nightFilter.dayColor.b + (this.nightFilter.nightColor.b - this.nightFilter.dayColor.b) * progress),
            };

            this.lightIntensityMultiplier = progress; // Lights gradually turn on
        } else if (time >= nightStart || time < nightEnd) {
            // Nighttime
            weather.fireflysActive = true;
            this.nightFilter.opacity = 1;
            this.nightFilter.color = { ...this.nightFilter.nightColor }; // Use nightColor
            this.lightIntensityMultiplier = 1; // Lights are fully on
        } else if (time >= nightEnd && time < sunriseEnd) {
            // Sunrise: 5:00 to 7:00
            const progress = (time - nightEnd) / (sunriseEnd - nightEnd); // 0 to 1
            this.nightFilter.opacity = 1 - progress; // Opacity decreases from 1 to 0

            // Interpolate color from nightColor to dayColor
            this.nightFilter.color = {
                r: Math.round(this.nightFilter.nightColor.r + (this.nightFilter.dayColor.r - this.nightFilter.nightColor.r) * progress),
                g: Math.round(this.nightFilter.nightColor.g + (this.nightFilter.dayColor.g - this.nightFilter.nightColor.g) * progress),
                b: Math.round(this.nightFilter.nightColor.b + (this.nightFilter.dayColor.b - this.nightFilter.nightColor.b) * progress),
            };

            this.lightIntensityMultiplier = 1 - progress; // Lights gradually turn off
        } else {
            // Default to day settings
            this.nightFilter.opacity = 0;
            this.nightFilter.color = { ...this.nightFilter.dayColor }; // Use dayColor
            this.lightIntensityMultiplier = 0; // Lights are off during the day
        }
    },

    renderNightFilter: function () {
        const ctx = game.ctx;
    
        ctx.save();
        ctx.setTransform(1, 0, 0, 1, 0, 0);
    
        const maskCanvas = document.createElement('canvas');
        maskCanvas.width = game.canvas.width;
        maskCanvas.height = game.canvas.height;
        const maskCtx = maskCanvas.getContext('2d');
    
        maskCtx.fillStyle = `rgba(${this.nightFilter.nightColor.r}, ${this.nightFilter.nightColor.g}, ${this.nightFilter.nightColor.b}, ${this.nightFilter.opacity})`;
        maskCtx.fillRect(0, 0, maskCanvas.width, maskCanvas.height);
    
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
    
        // Dark gradient overlay
        const overlayGradient = maskCtx.createRadialGradient(
            maskCanvas.width / 2,
            maskCanvas.height / 2,
            0,
            maskCanvas.width / 2,
            maskCanvas.height / 2,
            Math.max(maskCanvas.width, maskCanvas.height) / 2
        );
    
        overlayGradient.addColorStop(0, 'rgba(0, 0, 0, 0)');
        overlayGradient.addColorStop(0.5, 'rgba(0, 0, 0, 0.5)');
        overlayGradient.addColorStop(0.6, 'rgba(0, 0, 0, 0.6)');
        overlayGradient.addColorStop(1, 'rgba(0, 0, 0, 0.9)');
    
        maskCtx.globalCompositeOperation = 'source-over';
        maskCtx.fillStyle = overlayGradient;
        maskCtx.fillRect(0, 0, maskCanvas.width, maskCanvas.height);
    
        ctx.globalCompositeOperation = this.nightFilter.compositeOperation;
        ctx.drawImage(maskCanvas, 0, 0);
        ctx.restore();
    },
    
    
    
};