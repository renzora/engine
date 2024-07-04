var weather = {
    fireflys: [],
    rainDrops: [],
    snowflakes: [],
    fogs: [],
    maxSnowflakes: 1000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    lightningFlash: null,
    lightningCooldown: 0,
    snowActive: false,
    rainActive: false,
    fogActive: false,
    fireflysActive: false,
    nightActive: false,

    createSnow: function(opacity) {
        if (!this.snowActive) return;
        this.snowflakes = [];
        for (let i = 0; i < this.maxSnowflakes; i++) {
            this.snowflakes.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: this.snowflakeSize,
                speed: 0.8,
                sway: Math.random() * 0.5 + 0.1,
                opacity: opacity
            });
        }
        this.setBodyBackgroundForSnow(true); // Apply white background to body
    },

    stopSnow: function() {
        this.snowflakes = [];
        this.setBodyBackgroundForSnow(false); // Remove white background from body
    },

    updateSnow: function() {
        if (!this.snowActive) return;
        this.snowflakes.forEach(snowflake => {
            snowflake.y += snowflake.speed;
            snowflake.x += Math.sin(snowflake.y * 0.01) * snowflake.sway * this.swayDirection;

            if (snowflake.y > game.canvas.height) {
                snowflake.y = 0;
                snowflake.x = Math.random() * game.canvas.width;
            }
        });
    },

    drawSnow: function() {
        if (!this.snowActive) return;
        game.ctx.save();
        game.ctx.fillStyle = 'rgba(255, 255, 255, 1)';
        game.ctx.globalAlpha = 0.8;
        this.snowflakes.forEach(snowflake => {
            game.ctx.beginPath();
            game.ctx.arc(snowflake.x, snowflake.y, snowflake.radius, 0, Math.PI * 2);
            game.ctx.closePath();
            game.ctx.fill();
        });
        game.ctx.restore();
    },

    setBodyBackgroundForSnow: function(apply) {
        if (apply) {
            document.body.style.background = 'rgba(240, 248, 255, 0.8)'; // Light snow-like color with opacity
        } else {
            document.body.style.background = ''; // Reset to default
        }
    },

    createFireflys: function() {
        if (!this.fireflysActive) return;
        this.fireflys = [];
        for (let i = 0; i < 300; i++) {
            this.fireflys.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: Math.random() * 0.3 + 0.1,
                twinkle: Math.random() * 0.02 + 0.01,
                speed: Math.random() * 0.2 + 0.1
            });
        }
    },

    updateFireflys: function() {
        if (!this.fireflysActive) return;
        for (let star of this.fireflys) {
            star.radius += star.twinkle;
            if (star.radius > 0.2 || star.radius < 0.1) {
                star.twinkle = -star.twinkle;
            }
            star.y += star.speed;
            if (star.y > game.canvas.height) {
                star.y = -star.radius;
                star.x = Math.random() * game.canvas.width;
            }
        }
    },

    drawFireflys: function() {
        if (!this.fireflysActive) return;
        game.ctx.fillStyle = 'gold';
        for (let star of this.fireflys) {
            game.ctx.beginPath();
            game.ctx.arc(star.x, star.y, star.radius, 0, Math.PI * 2);
            game.ctx.fill();
        }
    },

    createRain: function(opacity) {
        if (!this.rainActive) return;
        this.rainDrops = []; // Clear existing rain drops if any
        for (let i = 0; i < 1000; i++) {
            this.rainDrops.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                length: Math.random() * 8,
                opacity: Math.random() * opacity,
                speed: 7
            });
        }
    },

    drawRain: function() {
        if (!this.rainActive) return;
        game.ctx.strokeStyle = 'rgba(174, 194, 224, 0.4)';
        game.ctx.lineWidth = 1;
        game.ctx.lineCap = 'round';
        for (let drop of this.rainDrops) {
            game.ctx.globalAlpha = drop.opacity;
            game.ctx.beginPath();
            game.ctx.moveTo(drop.x, drop.y);
            game.ctx.lineTo(drop.x, drop.y + drop.length);
            game.ctx.stroke();
        }
        game.ctx.globalAlpha = 1;
    },

    updateRain: function() {
        if (!this.rainActive) return;
        for (let drop of this.rainDrops) {
            drop.y += drop.speed;
            if (drop.y > game.canvas.height) {
                drop.y = -drop.length;
                drop.x = Math.random() * game.canvas.width;
            }
        }
    },

    createFog: function(opacity) {
        if (!this.fogActive) return;
        this.fogs = []; // Clear existing fogs if any
        for (let i = 0; i < 20; i++) {
            let fog = {
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height / 2,
                circles: []
            };
            // Create multiple circles for each mist patch
            for (let j = 0; j < 5; j++) {
                fog.circles.push({
                    offsetX: Math.random() * 100 - 50, // Random offset to spread circles
                    offsetY: Math.random() * 100 - 50,
                    size: Math.random() * 100 + 50,
                    opacity: Math.random() * opacity // Use passed opacity
                });
            }
            this.fogs.push(fog);
        }
    },

    updateFog: function() {
        if (!this.fogActive) return;
        for (let fog of this.fogs) {
            fog.x += 0.005; // Extremely slow horizontal movement
            if (fog.x > game.canvas.width + 100) {
                fog.x = -200; // Move it back to the left side with some offset
                fog.y = Math.random() * game.canvas.height / 2;
            }
        }
    },

    drawFog: function() {
        if (!this.fogActive) return;
        // Create an off-screen canvas
        let offScreenCanvas = document.createElement('canvas');
        offScreenCanvas.width = game.canvas.width;
        offScreenCanvas.height = game.canvas.height;
        let offScreenCtx = offScreenCanvas.getContext('2d');

        for (let fog of this.fogs) {
            offScreenCtx.save();
            for (let circle of fog.circles) {
                offScreenCtx.globalAlpha = circle.opacity;
                offScreenCtx.beginPath();
                offScreenCtx.arc(fog.x + circle.offsetX, fog.y + circle.offsetY, circle.size, 0, Math.PI * 2);
                offScreenCtx.closePath();
                offScreenCtx.fillStyle = 'rgba(255, 255, 255, 0.5)'; // Mist color with some transparency
                offScreenCtx.fill();
            }
            offScreenCtx.restore();
        }

        // Apply a blur filter
        offScreenCtx.filter = 'blur(10px)';

        // Draw the blurred off-screen canvas onto the main canvas
        game.ctx.drawImage(offScreenCanvas, 0, 0);
        game.ctx.globalAlpha = 1; // Reset global alpha
    },

    createLightning: function() {
        this.lightningFlash = {
            duration: Math.random() * 100 + 50, // Flash duration between 50ms to 150ms
            intensity: Math.random() * 0.3 + 0.7, // Flash intensity between 0.7 to 1
            timeLeft: 0 // Remaining time for the current flash
        };
    },

    triggerLightning: function() {
        if (this.lightningCooldown <= 0 && Math.random() < 0.005) { // 0.5% chance per frame to trigger lightning
            this.createLightning();
            this.lightningCooldown = Math.random() * 5000 + 3000; // Cooldown between 3 to 8 seconds
        }
    },

    updateLightning: function() {
        if (this.lightningFlash && this.lightningFlash.timeLeft > 0) {
            this.lightningFlash.timeLeft -= game.deltaTime;
            if (this.lightningFlash.timeLeft <= 0) {
                this.lightningFlash = null; // End the lightning flash
            }
        } else if (this.lightningCooldown > 0) {
            this.lightningCooldown -= game.deltaTime;
        }
    },

    drawLightning: function() {
        if (this.lightningFlash) {
            game.ctx.save();
            game.ctx.fillStyle = `rgba(255, 255, 255, ${this.lightningFlash.intensity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
            game.ctx.restore();
        }
    }
};
