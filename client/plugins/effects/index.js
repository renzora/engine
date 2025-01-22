window[id] = {
    id: id,

    start: function() {

    },

    unmount: function() {

    },

    onRender: function() {
        this.transitions.render();
        this.transitions.update();
        this.letterbox.update();
    },

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
                game.ctx.fillRect(0, game.canvas.height - this.barHeight, game.canvas.width, this.barHeight);
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
    
            for (let i = 0; i < 30; i++) {
                effectInstance.bubbles.push({
                    x: Math.random() * sprite.width - sprite.width / 2,
                    y: sprite.height - 5,
                    radius: Math.random() * 2,
                    opacity: 0.7,
                    riseSpeed: Math.random() * 1 + 0.5,
                });
            }
    
            this.activeEffects.push(effectInstance);
        },
    
        updateAndRender: function (deltaTime) {
            const ctx = game.ctx;
        
            for (let i = this.activeEffects.length - 1; i >= 0; i--) {
                const effect = this.activeEffects[i];
                const sprite = game.sprites[effect.spriteId];
        
                if (!sprite) {
                    this.activeEffects.splice(i, 1);
                    continue;
                }
        
                effect.bubbles.forEach((bubble, index) => {
                    const bubbleX = sprite.x + sprite.width / 2 + bubble.x;
                    const bubbleY = sprite.y + bubble.y;
        
                    const colorWithOpacity = `${effect.colorHex}${Math.floor(bubble.opacity * 255).toString(16).padStart(2, '0')}`;
                    ctx.fillStyle = colorWithOpacity;
        
                    ctx.beginPath();
                    ctx.arc(bubbleX, bubbleY, bubble.radius, 0, Math.PI * 2);
                    ctx.fill();
        
                    bubble.y -= bubble.riseSpeed * deltaTime / 22;
        
                    const fadeHeight = 1;
                    const distanceAboveSprite = Math.max(0, sprite.y - bubbleY);
                    if (distanceAboveSprite > fadeHeight) {
                        bubble.opacity -= 0.04;
                    } else {
                        bubble.opacity -= 0.01;
                    }
        
                    if (bubble.opacity <= 0 || bubbleY < sprite.y - sprite.height - 32) {
                        effect.bubbles.splice(index, 1);
                    }
                });
        
                if (effect.bubbles.length === 0) {
                    this.activeEffects.splice(i, 1);
                }
            }
        }
        
    },

    dirtCloudEffect: {
        activeClouds: [],
        lastCloudTime: 0,
        cloudInterval: 350,
    
        create: function (sprite, colorHex = '#8B4513') {
            const currentTime = performance.now();
    
            if (currentTime - this.lastCloudTime < this.cloudInterval) {
                return;
            }
    
            this.lastCloudTime = currentTime;
    
            const directionOffsets = {
                N: { x: 0, y: -1 },
                NE: { x: -0.5, y: -0.5 },
                E: { x: -0.5, y: 0 },
                SE: { x: -0.5, y: 0.5 },
                S: { x: 0, y: 0.5 },
                SW: { x: 0.5, y: 0.5 },
                W: { x: 0.5, y: 0 },
                NW: { x: 0.5, y: -0.5 },
            };
    
            const dir = sprite.direction || 'S';
            const offset = directionOffsets[dir];
    
            const cloudInstance = {
                spriteId: sprite.id,
                colorHex: colorHex,
                x: sprite.x + sprite.width / 2,
                y: sprite.y + sprite.height - 5,
                dx: offset.x * (Math.random() * 0.02 + 0.01),
                dy: offset.y * (Math.random() * 0.02 + 0.01),
                shapes: this.generateIrregularShape(),
                opacity: 0.2,
                fadeSpeed: Math.random() * 0.001 + 0.0005,
            };
    
            this.activeClouds.push(cloudInstance);
        },
    
        generateIrregularShape: function () {
            const shapePoints = [];
            const numPoints = Math.random() * 5 + 5;
            const maxRadius = Math.random() * 3 + 2;
    
            for (let i = 0; i < numPoints; i++) {
                const angle = (i / numPoints) * Math.PI * 2;
                const radius = maxRadius * (Math.random() * 0.5 + 0.75);
                shapePoints.push({
                    x: Math.cos(angle) * radius,
                    y: Math.sin(angle) * radius,
                });
            }
    
            return shapePoints;
        },
    
        updateAndRender: function (deltaTime) {
            const ctx = game.ctx;
    
            for (let i = this.activeClouds.length - 1; i >= 0; i--) {
                const cloud = this.activeClouds[i];
    
                cloud.x += cloud.dx * deltaTime / 30;
                cloud.y += cloud.dy * deltaTime / 30;
    
                cloud.opacity -= cloud.fadeSpeed * deltaTime;
    
                if (cloud.opacity > 0) {
                    ctx.save();
                    ctx.globalAlpha = cloud.opacity;
                    ctx.fillStyle = cloud.colorHex;
    
                    ctx.beginPath();
                    ctx.moveTo(cloud.x + cloud.shapes[0].x, cloud.y + cloud.shapes[0].y);
                    for (let point of cloud.shapes) {
                        ctx.lineTo(cloud.x + point.x, cloud.y + point.y);
                    }
                    ctx.closePath();
                    ctx.fill();
                    ctx.restore();
                }
    
                if (cloud.opacity <= 0) {
                    this.activeClouds.splice(i, 1);
                }
            }
        },
    }
}
