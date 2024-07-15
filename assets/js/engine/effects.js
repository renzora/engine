var effects = {
    lights: [],
    compositeOperation: 'soft-light',
    nightFilter: {
        opacity: 0.92,
        color: { r: 14, g: 0, b: 78 },
        compositeOperation: 'hard-light'
    },
    timeBasedUpdatesEnabled: true,
    nightAmbiencePlaying: false,
    activeEffects: {},

    LightSource: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        this.id = id;
        this.x = x;
        this.y = y;
        this.baseRadius = radius;
        this.color = color;
        this.maxIntensity = maxIntensity;
        this.initialMaxIntensity = maxIntensity; // Store the initial max intensity
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
            // Validate light coordinates and radius
            if (!isFinite(light.x) || !isFinite(light.y) || !isFinite(light.baseRadius)) {
                console.warn(`Invalid light parameters: x=${light.x}, y=${light.y}, radius=${light.baseRadius}`);
                return;
            }

            // Clear the temporary canvas
            tempCtx.clearRect(0, 0, tempCanvas.width, tempCanvas.height);

            // Create a radial gradient with smoother falloff
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

            // Apply a subtle Gaussian blur to the temporary canvas
            tempCtx.filter = 'blur(20px)';
            tempCtx.drawImage(tempCanvas, 0, 0);

            // Draw the blurred light onto the main light canvas with 'lighter' blend mode
            lightCtx.globalCompositeOperation = 'lighter';
            lightCtx.drawImage(tempCanvas, 0, 0);
        });

        // Apply an additional blur to the light mask to smooth out transitions
        lightCtx.filter = 'blur(10px)';
        lightCtx.drawImage(lightCanvas, 0, 0);

        return lightCanvas;
    },
    addLight: function(id, x, y, radius, color, maxIntensity, type, flicker = false, flickerSpeed = 0.1, flickerAmount = 0.05) {
        const existingLight = this.lights.find(light => light.id === id);
        if (!existingLight) {
            const clampedMaxIntensity = Math.min(maxIntensity, maxIntensity); // Ensure maxIntensity is within bounds

            const newLight = new this.LightSource(id, x, y, radius, color, clampedMaxIntensity, type, flicker, flickerSpeed, flickerAmount);
            newLight.currentIntensity = clampedMaxIntensity; // Set initial intensity to maxIntensity
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
                light.currentIntensity = Math.max(0, Math.min(light.currentIntensity, light.maxIntensity)); // Clamp the intensity between 0 and maxIntensity
            } else {
                light.currentIntensity = 0; // Ensure light is off when maxIntensity is 0
            }
        });
    },
    updateDayNightCycle: function() {
        if (!this.timeBasedUpdatesEnabled) return; // Check if time-based updates are enabled

        const hours = game.gameTime.hours;
        const minutes = game.gameTime.minutes;
        const time = hours + minutes / 60;

        // Calculate night filter color and opacity
        if (time >= 22 || time < 7) { // From 10 PM to 7 AM
            if (!this.nightAmbiencePlaying) {
                audio.playAudio("nightAmbience", assets.load('nightAmbience'), 'ambience', true);
                this.nightAmbiencePlaying = true;
            }
            if (time >= 22 && time < 24) {
                effects.nightFilter.opacity = Math.min(0.92, (time - 22) / 2 * 0.92);
            } else if (time >= 0 && time < 6) {
                effects.nightFilter.opacity = 0.92;
            } else if (time >= 6 && time < 7) { // Sunrise effect
                const progress = (time - 6);
                effects.nightFilter.opacity = 0.92 - progress * 0.92;
                effects.nightFilter.color = {
                    r: Math.round(14 + progress * (255 - 14)),
                    g: Math.round(0 + progress * (140 - 0)),
                    b: Math.round(78 + progress * (0 - 78))
                };
            }

            if (!(time >= 6 && time < 7)) { // Only update night filter color if it's not sunrise
                const progress = (time >= 22) ? (time - 22) / 2 : (7 - time) / 7;
                effects.nightFilter.color = {
                    r: Math.round(14 + progress * (14 - 14)),
                    g: Math.round(0 + progress * (0 - 0)),
                    b: Math.round(78 + progress * (78 - 78))
                };
            }
        } else {
            if (this.nightAmbiencePlaying) {
                audio.stopLoopingAudio('nightAmbience', 'ambience', 0.5);
                this.nightAmbiencePlaying = false;
            }
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
            let targetIntensity = 0;

            if (time >= 7 && time < 22) {
                light.maxIntensity = 0;
                light.flicker = false; // Turn off flickering when the lights are off
            } else if (time >= 6 && time < 7) { // Sunrise effect
                light.maxIntensity = light.initialMaxIntensity * (1 - (time - 6));
                light.flicker = false; // Turn off flickering during sunrise
            } else if (time >= 22 && time < 24) { // Sunset effect
                light.maxIntensity = light.initialMaxIntensity * (time - 22) / 2;
                light.flicker = true; // Enable flickering during sunset
            } else {
                light.maxIntensity = light.initialMaxIntensity; // Set to the initial max intensity
                light.flicker = true; // Enable flickering during night
            }

            targetIntensity = Math.min(light.maxIntensity * progress, light.maxIntensity); // Ensure current intensity doesn't exceed maxIntensity
            light.currentIntensity = targetIntensity;
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

    createParticles: function(x, y, options, effectId) {
        const particleCount = options.count || 20;
        const baseSpeed = options.speed || 2;
        const baseAngleDeg = options.angle || 0; // Angle in degrees
        const baseAngle = baseAngleDeg * (Math.PI / 180); // Convert degrees to radians
        const spread = options.spread || 360; // Spread in degrees
        const spreadRad = spread * (Math.PI / 180); // Convert spread to radians
        const colors = options.colors || ['rgba(255, 0, 0, 1)'];
        const repeat = options.repeat || false;
    
        for (let i = 0; i < particleCount; i++) {
            const randomSpread = (Math.random() - 0.5) * spreadRad;
            const angle = baseAngle + randomSpread;
            const speed = baseSpeed * (0.5 + Math.random() * 0.5);
            const color = colors[Math.floor(Math.random() * colors.length)];
    
            const particle = {
                x: x,
                y: y,
                initialX: x,
                initialY: y,
                vx: Math.cos(angle) * speed,
                vy: Math.sin(angle) * speed,
                life: options.life || 50,
                maxLife: options.life || 50,
                color: color,
                size: options.size || 2,
                type: options.type || 'default',
                repeat: repeat,
                glow: options.glow || 0,
                initialOpacity: options.opacity || 1,
                blur: options.blur || 0,
                shape: options.shape || 'circle'
            };
    
            if (!this.activeEffects[effectId]) {
                this.activeEffects[effectId] = [];
            }
            this.activeEffects[effectId].push(particle);
            game.particles.push(particle);
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
    
                // Update opacity based on remaining life
                const fadeStart = 0.2; // Fraction of life remaining when fade starts
                const lifeFraction = particle.life / particle.maxLife;
                if (lifeFraction < fadeStart) {
                    particle.opacity = particle.initialOpacity * (lifeFraction / fadeStart);
                } else {
                    particle.opacity = particle.initialOpacity;
                }
            }
    
            if (particle.life <= 0 && particle.repeat) {
                particle.life = particle.maxLife; // Reset life for repeating particles
                particle.x = particle.initialX; // Reset to initial x position
                particle.y = particle.initialY; // Reset to initial y position
            }
        });
    },
    
    renderParticles: function() {
        Object.values(this.activeEffects).forEach(effectParticles => {
            effectParticles.forEach(particle => {
                const ctx = game.ctx;
    
                ctx.save();
                ctx.globalAlpha = particle.opacity;
    
                if (particle.glow > 0) {
                    ctx.shadowColor = particle.color;
                    ctx.shadowBlur = particle.glow;
                }
    
                if (particle.blur > 0) {
                    ctx.filter = `blur(${particle.blur}px)`;
                }
    
                ctx.fillStyle = particle.color;
                ctx.beginPath();
    
                switch (particle.shape) {
                    case 'square':
                        ctx.fillRect(particle.x, particle.y, particle.size, particle.size);
                        break;
                    case 'triangle':
                        ctx.moveTo(particle.x, particle.y - particle.size);
                        ctx.lineTo(particle.x - particle.size, particle.y + particle.size);
                        ctx.lineTo(particle.x + particle.size, particle.y + particle.size);
                        ctx.closePath();
                        break;
                    default:
                        ctx.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2);
                        break;
                }
    
                ctx.fill();
                ctx.restore();
            });
        });
    }
};