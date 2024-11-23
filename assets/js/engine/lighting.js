const lighting = {
    lights: [],
    compositeOperation: 'soft-light',
nightFilter: {
    opacity: 0.7, // Adjust to control the overall darkness
    color: { r: 20, g: 20, b: 50 }, // A darker blue tint
    compositeOperation: 'multiply'
},
    greyFilter: {
        opacity: 0.5,
        color: { r: 128, g: 128, b: 128 }, // Grey color
        compositeOperation: 'source-out'
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

    clearLightsAndEffects: function() {
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

createLightMask: function() {
    const lightCanvas = document.createElement('canvas');
    lightCanvas.width = game.canvas.width;
    lightCanvas.height = game.canvas.height;
    const lightCtx = lightCanvas.getContext('2d');

    // Clear the canvas
    lightCtx.clearRect(0, 0, lightCanvas.width, lightCanvas.height);

    // Set composite operation to 'source-over' for proper layering
    lightCtx.globalCompositeOperation = 'source-over';

    this.lights.forEach(light => {
        const gradient = lightCtx.createRadialGradient(
            light.x, light.y, 0,
            light.x, light.y, light.baseRadius
        );

        const color = light.color;
        const intensity = light.currentIntensity;

        // Create a smooth gradient for softer lighting
        gradient.addColorStop(0, `rgba(${color.r}, ${color.g}, ${color.b}, ${intensity})`);
        gradient.addColorStop(0.5, `rgba(${color.r}, ${color.g}, ${color.b}, ${intensity * 0.5})`);
        gradient.addColorStop(1, `rgba(${color.r}, ${color.g}, ${color.b}, 0)`);

        // Apply shadow blur for softness
        lightCtx.save();
        lightCtx.shadowBlur = 50; // Increase for softer edges
        lightCtx.shadowColor = `rgba(${color.r}, ${color.g}, ${color.b}, ${intensity})`;
        lightCtx.fillStyle = gradient;
        lightCtx.beginPath();
        lightCtx.arc(light.x, light.y, light.baseRadius, 0, Math.PI * 2);
        lightCtx.fill();
        lightCtx.restore();
    });

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
    const isNightTime = time >= 20 || time < 6;

    // Update fireflies state based on nighttime
    weather.fireflysActive = isNightTime;

    // Play or stop night ambience based on night time
    if (isNightTime) {
        if (!this.nightAmbiencePlaying) {
            audio.playAudio("nightAmbience", assets.use('nightAmbience'), 'ambience', true);
            this.nightAmbiencePlaying = true;
        }
    } else {
        if (this.nightAmbiencePlaying) {
            audio.stopLoopingAudio('nightAmbience', 'ambience', 0.5);
            this.nightAmbiencePlaying = false;
        }
    }

    // Adjust night filter opacity smoothly between times
    if (time >= 18 && time < 20) { // Transition into night from 18:00 to 20:00
        this.nightFilter.opacity = (time - 18) / 2; // Transition from 0 to 1 opacity over 2 hours
    } else if (time >= 20 || time < 5) { // Night time from 20:00 to 5:00
        this.nightFilter.opacity = 1;
    } else if (time >= 5 && time < 7) { // Transition out of night from 5:00 to 7:00
        this.nightFilter.opacity = 1 - ((time - 5) / 2); // Transition from 1 to 0 opacity over 2 hours
    } else {
        this.nightFilter.opacity = 0; // Daytime
    }

    // Ensure opacity stays within [0, 1]
    this.nightFilter.opacity = Math.min(Math.max(this.nightFilter.opacity, 0), 1);

    // Adjust light intensities based on time
    if (time >= 18 && time < 20) { // Transition into night
        const progress = (time - 18) / 2; // Progress from 0 to 1 over 2 hours
        this.updateLightsIntensity(progress);
    } else if (time >= 20 || time < 5) { // Night time
        this.updateLightsIntensity(1);
    } else if (time >= 5 && time < 7) { // Transition out of night
        const progress = 1 - ((time - 5) / 2); // Progress from 1 to 0 over 2 hours
        this.updateLightsIntensity(progress);
    } else {
        this.updateLightsIntensity(0); // Daytime, lights off
    }
},
    

drawNightFilter: function() {
    game.ctx.save();
    game.ctx.globalCompositeOperation = 'multiply';
    game.ctx.fillStyle = `rgba(${this.nightFilter.color.r}, ${this.nightFilter.color.g}, ${this.nightFilter.color.b}, ${this.nightFilter.opacity})`;
    game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
    game.ctx.restore();
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
