var effects = {
    lights: [],
    compositeOperation: 'soft-light',
    nightFilter: {
        opacity: 0.81,
        color: { r: 7, g: 0, b: 57 },
        compositeOperation: 'hard-light'
    },
    LightSource: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        this.id = id;
        this.x = x;
        this.y = y;
        this.baseRadius = radius;
        this.color = color;
        this.maxIntensity = maxIntensity;
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
        
        // Set blend mode to 'lighter' for additive blending
        lightCtx.globalCompositeOperation = 'soft-light';
        
        this.lights.forEach(light => {
            // Validate light coordinates and radius
            if (!isFinite(light.x) || !isFinite(light.y) || !isFinite(light.baseRadius)) {
                console.warn(`Invalid light parameters: x=${light.x}, y=${light.y}, radius=${light.baseRadius}`);
                return;
            }
        
            const gradient = lightCtx.createRadialGradient(light.x, light.y, 0, light.x, light.y, light.baseRadius);
            gradient.addColorStop(0, `rgba(${light.color.r}, ${light.color.g}, ${light.color.b}, ${light.currentIntensity})`);
            gradient.addColorStop(1, 'rgba(0, 0, 0, 0)');
        
            lightCtx.fillStyle = gradient;
            lightCtx.beginPath();
            lightCtx.arc(light.x, light.y, light.baseRadius, 0, Math.PI * 2);
            lightCtx.fill();
        });
        
        return lightCanvas;
    },

    addLight: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        const existingLight = this.lights.find(light => light.id === id);
        if (!existingLight) {
            const hours = game.gameTime.hours;
            const minutes = game.gameTime.minutes;
            const time = hours + minutes / 60;
            let initialIntensity = 0;

            if (time >= 22 || time < 7) { // From 10pm to 7am
                if (time >= 22 && time < 24) {
                    initialIntensity = Math.min(maxIntensity, (time - 22) / 2 * maxIntensity);
                } else if (time >= 0 && time < 7) {
                    initialIntensity = Math.min(maxIntensity, (7 - time) / 7 * maxIntensity);
                }
            }

            const newLight = new this.LightSource(id, x, y, radius, color, maxIntensity, type, flicker, flickerSpeed, flickerAmount);
            newLight.currentIntensity = initialIntensity; // Set the initial intensity based on time
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
                light.currentIntensity = Math.max(0, Math.min(light.currentIntensity, 1)); // Clamp the intensity between 0 and 1
            } else {
                light.currentIntensity = 0; // Ensure light is off when maxIntensity is 0
            }
        });
    },

    updateDayNightCycle: function() {
        const hours = game.gameTime.hours;
        const minutes = game.gameTime.minutes;
        const time = hours + minutes / 60;

        // Calculate night filter color and opacity
        if (time >= 22 || time < 7) { // From 10pm to 7am
            if (time >= 22 && time < 24) {
                effects.nightFilter.opacity = Math.min(0.9, (time - 22) / 2 * 0.9);
            } else if (time >= 0 && time < 6) {
                effects.nightFilter.opacity = 0.9;
            } else if (time >= 6 && time < 7) { // Sunrise effect
                const progress = (time - 6);
                effects.nightFilter.opacity = 0.9 - progress * 0.9;
                effects.nightFilter.color = {
                    r: Math.round(7 + progress * (255 - 7)),
                    g: Math.round(0 + progress * (140 - 0)),
                    b: Math.round(57 + progress * (0 - 57))
                };
            }

            const progress = (time >= 22) ? (time - 22) / 2 : (7 - time) / 7;
            effects.nightFilter.color = {
                r: Math.round(0 + progress * (7 - 0)),
                g: Math.round(0 + progress * (0 - 0)),
                b: Math.round(0 + progress * (57 - 0))
            };
        } else {
            effects.nightFilter.opacity = 0;
            effects.nightFilter.color = { r: 255, g: 255, b: 255 };
        }

        // Update light sources intensity
        if (time >= 22 || time < 6) {
            if (time >= 22 && time < 24) {
                const progress = (time - 22) / 2;
                effects.updateLightsIntensity(progress);
            } else if (time >= 0 && time < 6) {
                const progress = (6 - time) / 6;
                effects.updateLightsIntensity(progress);
            }
        } else if (time >= 6 && time < 7) { // Sunrise effect
            const progress = (time - 6);
            effects.updateLightsIntensity(1 - progress);
        } else if (time >= 7 && time < 22) {
            effects.updateLightsIntensity(0);
        }
    },

    updateLightsIntensity: function(progress) {
        this.lights.forEach(light => {
            const hours = game.gameTime.hours;
            const minutes = game.gameTime.minutes;
            const time = hours + minutes / 60;
            if (time >= 7 && time < 22) {
                light.maxIntensity = 0;
                light.flicker = false; // Turn off flickering when the lights are off
            } else if (time >= 6 && time < 7) { // Sunrise effect
                light.maxIntensity = 1 - (time - 6);
                light.flicker = false; // Turn off flickering during sunrise
            } else if (time >= 22 && time < 24) { // Sunset effect
                light.maxIntensity = (time - 22) / 2;
                light.flicker = true; // Enable flickering during sunset
            } else {
                light.maxIntensity = 1; // Set to the original max intensity or as needed
                light.flicker = true; // Enable flickering during night
            }
            light.currentIntensity = light.maxIntensity * progress;
        });
    },

    shakeMap: function(duration, intensity) {
        const originalCameraX = game.cameraX;
        const originalCameraY = game.cameraY;
    
        let elapsed = 0;
        const shakeInterval = setInterval(() => {
            elapsed += 16; // Approximate frame duration
    
            if (elapsed < duration) {
                const offsetX = (Math.random() - 0.5) * intensity;
                const offsetY = (Math.random() - 0.5) * intensity;
    
                game.cameraX = originalCameraX + offsetX;
                game.cameraY = originalCameraY + offsetY;
            } else {
                clearInterval(shakeInterval);
                game.cameraX = originalCameraX;
                game.cameraY = originalCameraY;
            }
        }, 16);
    },
    transitions: {
        active: false,
        type: 'fadeIn', // default transition type
        duration: 1000,
        startTime: null,
        progress: 0,

        start: function(type, duration) {
            this.active = true;
            this.type = type || 'fadeIn';
            this.duration = duration || 1000;
            this.startTime = performance.now();
            this.progress = 0;
        },

        update: function() {
            if (!this.active) return;

            const currentTime = performance.now();
            const elapsed = currentTime - this.startTime;
            this.progress = Math.min(elapsed / this.duration, 1);

            if (this.progress >= 1) {
                this.active = false;
            }
        },

        render: function() {
            if (!this.active) return;

            switch (this.type) {
                case 'fadeIn':
                    this.renderFadeIn();
                    break;
                case 'fadeOut':
                    this.renderFadeOut();
                    break;
                case 'swirl':
                    this.renderSwirl();
                    break;
                case 'square':
                    this.renderSquare();
                    break;
                case 'pixelate':
                    this.renderPixelate();
                    break;
                case 'de-pixelate':
                    this.renderDepixelate();
                    break;
            }
        },

        renderFadeIn: function() {
            const opacity = 1 - this.progress;
            game.ctx.fillStyle = `rgba(0, 0, 0, ${opacity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
        },

        renderFadeOut: function() {
            const opacity = this.progress;
            game.ctx.fillStyle = `rgba(0, 0, 0, ${opacity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
        },

        renderSwirl: function() {
            const angle = this.progress * Math.PI * 2;
            const radius = Math.max(game.canvas.width, game.canvas.height);
            game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
            game.ctx.save();
            game.ctx.translate(game.canvas.width / 2, game.canvas.height / 2);
            game.ctx.rotate(angle);
            game.ctx.fillRect(-radius, -radius, radius * 2, radius * 2);
            game.ctx.restore();
        },

        renderSquare: function() {
            const size = this.progress * Math.max(game.canvas.width, game.canvas.height);
            const halfSize = size / 2;
            game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
            game.ctx.fillRect(
                game.canvas.width / 2 - halfSize,
                game.canvas.height / 2 - halfSize,
                size,
                size
            );
        },

        renderPixelate: function() {
            const pixelSize = Math.ceil(this.progress * 20); // Max pixel size of 20
            for (let y = 0; y < game.canvas.height; y += pixelSize) {
                for (let x = 0; x < game.canvas.width; x += pixelSize) {
                    const color = this.getPixelColor(x, y, pixelSize);
                    if (color) {
                        game.ctx.fillStyle = color;
                        game.ctx.fillRect(x, y, pixelSize, pixelSize);
                    }
                }
            }
        },

        renderDepixelate: function() {
            const pixelSize = Math.ceil((1 - this.progress) * 20); // Start with max pixel size of 20
            for (let y = 0; y < game.canvas.height; y += pixelSize) {
                for (let x = 0; x < game.canvas.width; x += pixelSize) {
                    const color = this.getPixelColor(x, y, pixelSize);
                    if (color) {
                        game.ctx.fillStyle = color;
                        game.ctx.fillRect(x, y, pixelSize, pixelSize);
                    }
                }
            }
        },

        getPixelColor: function(x, y, size) {
            // Ensure valid dimensions
            const validWidth = Math.min(size, game.canvas.width - x);
            const validHeight = Math.min(size, game.canvas.height - y);
            if (validWidth <= 0 || validHeight <= 0) return null;

            const data = game.ctx.getImageData(x, y, validWidth, validHeight).data;
            let r = 0, g = 0, b = 0, a = 0;
            for (let i = 0; i < data.length; i += 4) {
                r += data[i];
                g += data[i + 1];
                b += data[i + 2];
                a += data[i + 3];
            }
            const pixelCount = data.length / 4;
            r = Math.floor(r / pixelCount);
            g = Math.floor(g / pixelCount);
            b = Math.floor(b / pixelCount);
            a = Math.floor(a / pixelCount / 255);
            return `rgba(${r},${g},${b},${a})`;
        }
    },

    createParticles: function(x, y, options) {
        const particleCount = options.count || 20;
        const baseSpeed = options.speed || 2;
        const baseAngle = options.angle || 0;
        const spread = options.spread || Math.PI * 2; // Default to 360 degrees
        const colors = options.colors || ['rgba(255, 0, 0, 1)']; // Default color if none provided

        for (let i = 0; i < particleCount; i++) {
            const angle = baseAngle + (Math.random() - 0.5) * spread;
            const speed = baseSpeed * (0.5 + Math.random() * 0.5); // Add some variability to the speed
            const color = colors[Math.floor(Math.random() * colors.length)]; // Select a random color

            game.particles.push({
                x: x,
                y: y,
                vx: Math.cos(angle) * speed,
                vy: Math.sin(angle) * speed,
                life: options.life || 50,
                color: color,
                size: options.size || 2,
                type: options.type || 'default' // Add a type to handle different behaviors
            });
        }
    },

    updateParticles: function(deltaTime) {
        game.particles = game.particles.filter(particle => particle.life > 0);

        game.particles.forEach(particle => {
            if (particle.type === 'ripple') {
                particle.size += 0.1; // Gradually increase the size for ripples
                particle.life -= deltaTime / 16;
                particle.color = `rgba(0, 0, 255, ${particle.life / 50})`; // Fade out the color
            } else {
                particle.x += particle.vx * deltaTime / 16;
                particle.y += particle.vy * deltaTime / 16;
                particle.life -= deltaTime / 16;
            }
        });
    },

    renderParticles: function() {
        game.particles.forEach(particle => {
            game.ctx.fillStyle = particle.color;
            game.ctx.globalAlpha = particle.life / 50; // Match reduced lifetime
            game.ctx.beginPath();
            game.ctx.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2);
            game.ctx.fill();
            game.ctx.globalAlpha = 1; // Reset alpha
        });
    },

    createGrassEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(34, 139, 34, 0.5)', 'rgba(50, 205, 50, 0.5)'], // Subtle green colors
            count: 10,
            speed: 0.5,
            life: 30,
            size: 1,
            spread: Math.PI / 8 // Narrow spread
        });
    },

    createSandEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(194, 178, 128, 0.5)', 'rgba(210, 180, 140, 0.5)'], // Subtle sand colors
            count: 10,
            speed: 1,
            life: 30,
            size: 1,
            spread: Math.PI / 4 // Wider spread
        });
    },

    createMudEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(139, 69, 19, 0.5)', 'rgba(160, 82, 45, 0.5)'], // Subtle brown colors
            count: 10,
            speed: 0.75,
            life: 30,
            size: 1,
            spread: Math.PI / 6 // Medium spread
        });
    }
};
