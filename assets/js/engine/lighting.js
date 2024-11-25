// lighting.js
const lighting = {
    lights: [],
    nightFilter: {
        opacity: 0, // Start with day opacity
        color: { r: 255, g: 255, b: 255 }, // Day color (white)
        compositeOperation: 'multiply', // Use 'multiply' blending mode
    },
    timeBasedUpdatesEnabled: true,
    nightAmbiencePlaying: false,
    lightIntensityMultiplier: 0, // Multiplier for light intensities based on time of day

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


    createNightFilterMask: function () {
        const maskCanvas = document.createElement('canvas');
        maskCanvas.width = game.canvas.width;
        maskCanvas.height = game.canvas.height;
        const maskCtx = maskCanvas.getContext('2d');

        // Fill the mask with the night filter color
        maskCtx.fillStyle = `rgba(${this.nightFilter.color.r}, ${this.nightFilter.color.g}, ${this.nightFilter.color.b}, ${this.nightFilter.opacity})`;
        maskCtx.fillRect(0, 0, maskCanvas.width, maskCanvas.height);

        // Adjust the mask under the lights
        this.lights.forEach((light) => {
            if (light.currentIntensity > 0) {
                // Convert world position to screen position
                const screenX = (light.x - camera.cameraX) * game.zoomLevel;
                const screenY = (light.y - camera.cameraY) * game.zoomLevel;
                const screenRadius = light.baseRadius * game.zoomLevel;

                // Prepare the gradient for the light
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

                gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${intensity})`); // Full intensity at center
                gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`); // Fade out at edges

                // Use 'screen' composite operation to add light to the mask
                maskCtx.globalCompositeOperation = 'screen';
                maskCtx.beginPath();
                maskCtx.arc(screenX, screenY, screenRadius, 0, Math.PI * 2);
                maskCtx.fillStyle = gradient;
                maskCtx.fill();
            }
        });

        // Reset composite operation
        maskCtx.globalCompositeOperation = 'source-over';

        return maskCanvas;
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

        // Initialize variables
        let opacity = 0;
        let color = { r: 255, g: 255, b: 255 }; // Day color (white)
        let lightIntensityMultiplier = 0; // Will be used to adjust light intensities

        // Night color
        const nightColor = { r: 19, g: 11, b: 121 };

        // Determine opacity, color, and light intensity based on time
        if (time >= dayStart && time < sunsetStart) {
            // Daytime
            opacity = 0;
            color = { r: 255, g: 255, b: 255 }; // Day color (white)
            lightIntensityMultiplier = 0; // Lights are off during the day
        } else if (time >= sunsetStart && time < nightStart) {
            // Sunset: 18:00 to 20:00
            const progress = (time - sunsetStart) / (nightStart - sunsetStart); // 0 to 1
            opacity = progress * 1; // Opacity increases from 0 to 1

            // Interpolate color from day color to night color
            const dayColor = { r: 255, g: 255, b: 255 }; // Day color (white)

            color = {
                r: Math.round(dayColor.r + (nightColor.r - dayColor.r) * progress),
                g: Math.round(dayColor.g + (nightColor.g - dayColor.g) * progress),
                b: Math.round(dayColor.b + (nightColor.b - dayColor.b) * progress),
            };

            lightIntensityMultiplier = progress; // Lights gradually turn on
        } else if (time >= nightStart || time < nightEnd) {
            // Nighttime
            opacity = 1;
            color = nightColor;
            lightIntensityMultiplier = 1; // Lights are fully on
        } else if (time >= nightEnd && time < sunriseEnd) {
            // Sunrise: 5:00 to 7:00
            const progress = (time - nightEnd) / (sunriseEnd - nightEnd); // 0 to 1
            opacity = 1 - progress; // Opacity decreases from 1 to 0

            // Interpolate color from night color to day color
            const dayColor = { r: 255, g: 255, b: 255 }; // Day color (white)

            color = {
                r: Math.round(nightColor.r + (dayColor.r - nightColor.r) * progress),
                g: Math.round(nightColor.g + (dayColor.g - nightColor.g) * progress),
                b: Math.round(nightColor.b + (dayColor.b - nightColor.b) * progress),
            };

            lightIntensityMultiplier = 1 - progress; // Lights gradually turn off
        } else {
            // Default to day settings
            opacity = 0;
            color = { r: 255, g: 255, b: 255 }; // Day color (white)
            lightIntensityMultiplier = 0; // Lights are off during the day
        }

        // Update nightFilter with calculated opacity and color
        this.nightFilter.opacity = opacity;
        this.nightFilter.color = color;

        // Update light intensity multiplier
        this.lightIntensityMultiplier = lightIntensityMultiplier;
    },

    updateNightTint: function (progress) {
        // Fixed dark blue color for night
        this.nightFilter.color = { r: 0, g: 0, b: 40 };
        // Increase opacity to make it darker
        this.nightFilter.opacity = 0.9 * progress; // From 0 (day) to 0.9 (night)
    },

    // New function to render the night filter
    renderNightFilter: function () {
        const ctx = game.ctx;

        // Save the current context state
        ctx.save();

        // Reset transformation to ensure we're drawing in screen coordinates
        ctx.setTransform(1, 0, 0, 1, 0, 0);

        // Create the night filter mask
        const maskCanvas = this.createNightFilterMask();

        // Apply the night filter mask over the scene using 'multiply'
        ctx.globalCompositeOperation = this.nightFilter.compositeOperation;
        ctx.drawImage(maskCanvas, 0, 0);

        // Restore the context state
        ctx.restore();
    },

    // New function to render light sources
    renderLightSources: function() {
        const ctx = game.ctx;
        ctx.save();

        // Reset transformation to ensure we're drawing in screen coordinates
        ctx.setTransform(1, 0, 0, 1, 0, 0);

        // Use 'lighter' composite operation to add light to the scene
        ctx.globalCompositeOperation = 'lighter';

        this.lights.forEach(light => {
            if (light.currentIntensity > 0) {
                // Convert world position to screen position
                const screenX = (light.x - camera.cameraX) * game.zoomLevel;
                const screenY = (light.y - camera.cameraY) * game.zoomLevel;
                const screenRadius = light.baseRadius * game.zoomLevel;

                const gradient = ctx.createRadialGradient(
                    screenX, screenY, 0,
                    screenX, screenY, screenRadius
                );

                const intensity = light.currentIntensity;
                const { r, g, b } = light.color;

                gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${intensity})`);
                gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

                ctx.fillStyle = gradient;
                ctx.beginPath();
                ctx.arc(screenX, screenY, screenRadius, 0, Math.PI * 2);
                ctx.fill();
            }
        });

        ctx.restore();
    },

    addLight: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
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
        this.lights.forEach((light) => {
            // Update maxIntensity based on time of day
            light.maxIntensity =
                light.initialMaxIntensity * this.lightIntensityMultiplier;

            if (light.maxIntensity > 0) {
                if (light.flicker) {
                    const flickerValue =
                        Math.sin(
                            (performance.now() + light.flickerOffset) *
                                light.flickerSpeed
                        ) * light.flickerAmount;
                    light.currentIntensity = light.maxIntensity + flickerValue;
                } else {
                    light.currentIntensity = light.maxIntensity;
                }
                light.currentIntensity = Math.max(
                    0,
                    Math.min(light.currentIntensity, light.maxIntensity)
                );
            } else {
                light.currentIntensity = 0;
            }
        });
    },

    updateLightsIntensity: function(progress) {
        this.lights.forEach(light => {
            const hours = utils.gameTime.hours;
            const minutes = utils.gameTime.minutes;
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
