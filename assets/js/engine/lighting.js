const lighting = {
    lights: [],
    compositeOperation: 'soft-light',
    nightFilter: {
        opacity: 1,
        color: { r: 102, g: 39, b: 255 },
        compositeOperation: 'multiply'
    },
    greyFilter: {
        opacity: 0.5,
        color: { r: 128, g: 128, b: 128 }, // Grey color
        compositeOperation: 'source-over'
    },
    timeBasedUpdatesEnabled: true,
    nightAmbiencePlaying: false,

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

    createLightMask: function() {
        const lightCanvas = document.createElement('canvas');
        lightCanvas.width = game.canvas.width;
        lightCanvas.height = game.canvas.height;
        const lightCtx = lightCanvas.getContext('2d');

        const tempCanvas = document.createElement('canvas');
        tempCanvas.width = game.canvas.width;
        tempCanvas.height = game.canvas.height;
        const tempCtx = tempCanvas.getContext('2d');

        this.lights.forEach(light => {
            if (!isFinite(light.x) || !isFinite(light.y) || !isFinite(light.baseRadius)) {
                console.warn(`Invalid light parameters: x=${light.x}, y=${light.y}, radius=${light.baseRadius}`);
                return;
            }

            tempCtx.clearRect(0, 0, tempCanvas.width, tempCanvas.height);

            const gradient = tempCtx.createRadialGradient(light.x, light.y, 0, light.x, light.y, light.baseRadius);
            gradient.addColorStop(0, `rgba(${light.color.r}, ${light.color.g}, ${light.color.b}, ${light.currentIntensity})`);
            gradient.addColorStop(0.4, `rgba(${light.color.r}, ${light.color.g}, ${light.color.b}, ${light.currentIntensity * 0.7})`);
            gradient.addColorStop(0.7, `rgba(${light.color.r}, ${light.color.g}, ${light.color.b}, ${light.currentIntensity * 0.4})`);
            gradient.addColorStop(0.9, `rgba(${light.color.r}, ${light.color.g}, ${light.color.b}, ${light.currentIntensity * 0.1})`);
            gradient.addColorStop(1, 'rgba(0, 0, 0, 0)');

            tempCtx.fillStyle = gradient;
            tempCtx.beginPath();
            tempCtx.arc(light.x, light.y, light.baseRadius, 0, Math.PI * 2);
            tempCtx.fill();

            tempCtx.filter = 'blur(20px)';
            tempCtx.drawImage(tempCanvas, 0, 0);

            lightCtx.globalCompositeOperation = 'lighter';
            lightCtx.drawImage(tempCanvas, 0, 0);
        });

        lightCtx.filter = 'blur(10px)';
        lightCtx.drawImage(lightCanvas, 0, 0);

        return lightCanvas;
    },

    addLight: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        const existingLight = this.lights.find(light => light.id === id);
        if (!existingLight) {
            const clampedMaxIntensity = Math.min(maxIntensity, maxIntensity);

            const newLight = new this.LightSource(id, x, y, radius, color, clampedMaxIntensity, type, flicker, flickerSpeed, flickerAmount);
            newLight.currentIntensity = clampedMaxIntensity;
            this.lights.push(newLight);
        }
    },

    updateLights: function(deltaTime) {
        this.lights.forEach(light => {
            if (light.maxIntensity > 0) {
                if (light.flicker) {
                    const flickerValue = Math.sin((performance.now() + light.flickerOffset) * light.flickerSpeed) * light.flickerAmount;
                    light.currentIntensity = light.maxIntensity + flickerValue;
                } else {
                    light.currentIntensity = light.maxIntensity;
                }
                light.currentIntensity = Math.max(0, Math.min(light.currentIntensity, light.maxIntensity));
            } else {
                light.currentIntensity = 0;
            }
        });
    },

    updateDayNightCycle: function() {
        if (!this.timeBasedUpdatesEnabled) return;
    
        const hours = game.gameTime.hours;
        const minutes = game.gameTime.minutes;
        const time = hours + minutes / 60;
    
        // Determine if it's nighttime
        const isNightTime = time >= 22 || time < 7;
    
        // Update fireflies state based on nighttime
        weather.fireflysActive = isNightTime;
    
        if (isNightTime) {
            if (!this.nightAmbiencePlaying) {
                audio.playAudio("nightAmbience", assets.load('nightAmbience'), 'ambience', true);
                this.nightAmbiencePlaying = true;
            }
            if (time >= 22 && time < 24) {
                lighting.nightFilter.opacity = Math.min(1, (time - 22) / 2);
            } else if (time >= 0 && time < 6) {
                lighting.nightFilter.opacity = 1;
            } else if (time >= 6 && time < 7) {
                const progress = (time - 6);
                lighting.nightFilter.opacity = 1 - progress;
                lighting.nightFilter.color = {
                    r: Math.round(102 + progress * (255 - 102)),
                    g: Math.round(39 + progress * (140 - 39)),
                    b: Math.round(255 + progress * (0 - 255))
                };
            }
    
            if (!(time >= 6 && time < 7)) {
                const progress = (time >= 22) ? (time - 22) / 2 : (7 - time) / 7;
                lighting.nightFilter.color = {
                    r: Math.round(102 + progress * (102 - 102)),
                    g: Math.round(39 + progress * (39 - 39)),
                    b: Math.round(255 + progress * (255 - 255))
                };
            }
        } else {
            if (this.nightAmbiencePlaying) {
                audio.stopLoopingAudio('nightAmbience', 'ambience', 0.5);
                this.nightAmbiencePlaying = false;
            }
            lighting.nightFilter.opacity = 0;
            lighting.nightFilter.color = { r: 255, g: 255, b: 255 };
        }
    
        if (time >= 22 || time < 6) {
            if (time >= 22 && time < 24) {
                const progress = (time - 22) / 2;
                lighting.updateLightsIntensity(progress);
            } else if (time >= 0 && time < 6) {
                const progress = (6 - time) / 6;
                lighting.updateLightsIntensity(progress);
            }
        } else if (time >= 6 && time < 7) {
            const progress = (time - 6);
            lighting.updateLightsIntensity(1 - progress);
        } else if (time >= 7 && time < 22) {
            lighting.updateLightsIntensity(0);
        }
    },

    drawNightFilter: function() {
        game.ctx.fillStyle = `rgba(${this.nightFilter.color.r}, ${this.nightFilter.color.g}, ${this.nightFilter.color.b}, ${this.nightFilter.opacity})`;
        game.ctx.globalCompositeOperation = this.nightFilter.compositeOperation;
        game.ctx.fillRect(camera.cameraX - 2, camera.cameraY - 2, (window.innerWidth / game.zoomLevel) + 4, (window.innerHeight / game.zoomLevel) + 4);
    },
    
    drawGreyFilter: function() {
        if (!weather.rainActive) return;
    
        const hours = game.gameTime.hours;
        const minutes = game.gameTime.minutes;
        const time = hours + minutes / 60;
    
        if (time >= 7 && time < 22) { // Daytime check
            game.ctx.save();
            game.ctx.fillStyle = `rgba(${this.greyFilter.color.r}, ${this.greyFilter.color.g}, ${this.greyFilter.color.b}, ${this.greyFilter.opacity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
            game.ctx.restore();
        }
    },

    updateLightsIntensity: function(progress) {
        this.lights.forEach(light => {
            const hours = game.gameTime.hours;
            const minutes = game.gameTime.minutes;
            const time = hours + minutes / 60;
            let targetIntensity = 0;

            if (time >= 7 && time < 22) {
                light.maxIntensity = 0;
                light.flicker = false;
            } else if (time >= 6 && time < 7) {
                light.maxIntensity = light.initialMaxIntensity * (1 - (time - 6));
                light.flicker = false;
            } else if (time >= 22 && time < 24) {
                light.maxIntensity = light.initialMaxIntensity * (time - 22) / 2;
                light.flicker = true;
            } else {
                light.maxIntensity = light.initialMaxIntensity;
                light.flicker = true;
            }

            targetIntensity = Math.min(light.maxIntensity * progress, light.maxIntensity);
            light.currentIntensity = targetIntensity;
        });
    }
};
