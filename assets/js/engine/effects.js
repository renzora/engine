const effects = {

    shakeMap: function(duration, intensity) {
        const originalCameraX = camera.cameraX;
        const originalCameraY = camera.cameraY;

        let elapsed = 0;
        const shakeInterval = setInterval(() => {
            elapsed += 16;

            if (elapsed < duration) {
                const offsetX = (Math.random() - 0.5) * intensity;
                const offsetY = (Math.random() - 0.5) * intensity;

                camera.cameraX = originalCameraX + offsetX;
                camera.cameraY = originalCameraY + offsetY;
            } else {
                clearInterval(shakeInterval);
                camera.cameraX = originalCameraX;
                camera.cameraY = originalCameraY;
            }
        }, 16);
    },

    transitions: {
        active: false,
        type: 'fadeIn',
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
        }
    },

    letterbox: {
        active: false,
        barHeight: 0,
        maxBarHeight: 130,
        speed: 3,
        start: function() {
            this.active = true;
            this.barHeight = 0;
        },
        stop: function() {
            this.active = false;
        },
        update: function() {
            if (this.active && this.barHeight < this.maxBarHeight) {
                this.barHeight += this.speed;
                if (this.barHeight > this.maxBarHeight) {
                    this.barHeight = this.maxBarHeight;
                }
            }
            if (!this.active && this.barHeight > 0) {
                this.barHeight -= this.speed;
                if (this.barHeight < 0) {
                    this.barHeight = 0;
                }
            }
            this.render();
        },
        render: function() {
            if (this.barHeight > 0) {
                game.ctx.setTransform(1, 0, 0, 1, 0, 0);
                game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
                game.ctx.fillRect(0, 0, game.canvas.width, this.barHeight);
                game.ctx.fillRect(0, game.canvas.height - this.barHeight, game.canvas.width, this.barHeight); // Bottom bar
            }
        }
    },

    bubbleEffect: {
        activeEffects: [],
    
        create: function (sprite, colorHex) {
            const effectInstance = {
                spriteId: sprite.id,
                bubbles: [],
                colorHex: colorHex,
            };
    
            // Generate a burst of bubbles
            for (let i = 0; i < 30; i++) { // Number of bubbles in the burst
                effectInstance.bubbles.push({
                    x: Math.random() * sprite.width - sprite.width / 2, // Random x-offset relative to sprite center
                    y: sprite.height - 5, // Start at the bottom of the sprite
                    radius: Math.random() * 2, // Smaller bubble size
                    opacity: 0.7, // Start fully visible
                    riseSpeed: Math.random() * 1 + 0.5, // Moderate randomized rise speed
                });
            }
    
            this.activeEffects.push(effectInstance);
        },
    
        updateAndRender: function (deltaTime) {
            const ctx = game.ctx;
        
            // Loop through all active effects
            for (let i = this.activeEffects.length - 1; i >= 0; i--) {
                const effect = this.activeEffects[i];
                const sprite = game.sprites[effect.spriteId];
        
                if (!sprite) {
                    // If sprite no longer exists, remove the effect
                    this.activeEffects.splice(i, 1);
                    continue;
                }
        
                // Render and update bubbles
                effect.bubbles.forEach((bubble, index) => {
                    const bubbleX = sprite.x + sprite.width / 2 + bubble.x;
                    const bubbleY = sprite.y + bubble.y;
        
                    // Set the bubble's color with opacity
                    const colorWithOpacity = `${effect.colorHex}${Math.floor(bubble.opacity * 255).toString(16).padStart(2, '0')}`;
                    ctx.fillStyle = colorWithOpacity;
        
                    // Draw the bubble
                    ctx.beginPath();
                    ctx.arc(bubbleX, bubbleY, bubble.radius, 0, Math.PI * 2);
                    ctx.fill();
        
                    // Update bubble properties
                    bubble.y -= bubble.riseSpeed * deltaTime / 22; // Move upwards based on speed
        
                    // Gradually fade out as the bubble rises
                    const fadeHeight = 1; // Start fading faster within half the sprite's height above the top
                    const distanceAboveSprite = Math.max(0, sprite.y - bubbleY);
                    if (distanceAboveSprite > fadeHeight) {
                        bubble.opacity -= 0.04; // Faster fade-out above a certain height
                    } else {
                        bubble.opacity -= 0.01; // Normal fade-out otherwise
                    }
        
                    // Remove bubbles that are fully transparent or just above the sprite's top
                    if (bubble.opacity <= 0 || bubbleY < sprite.y - sprite.height - 32) { // Stop closer above the sprite
                        effect.bubbles.splice(index, 1);
                    }
                });
        
                // Remove the effect when all bubbles are gone
                if (effect.bubbles.length === 0) {
                    this.activeEffects.splice(i, 1);
                }
            }
        }
        
    }
    
    

};
