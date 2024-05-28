var weather = {
    stars: [],
    rainDrops: [],
    snowflakes: [],
    maxSnowflakes: 1000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    nightColor: localStorage.getItem('nightColor') || '#000032',
    nightOpacity: parseFloat(localStorage.getItem('nightOpacity')) || 0.7,
    vignetteColor: localStorage.getItem('vignetteColor') || '#000000',
    vignetteOpacity: parseFloat(localStorage.getItem('vignetteOpacity')) || 0.6,

    createSnow: function(opacity) {
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
    },

    updateSnow: function() {
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

    createStars: function() {
        for (let i = 0; i < 300; i++) {
            this.stars.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: Math.random() * 0.3 + 0.1,
                twinkle: Math.random() * 0.02 + 0.01,
                speed: Math.random() * 0.2 + 0.1
            });
        }
    },

    updateStars: function() {
        for (let star of this.stars) {
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

    drawStars: function() {
        game.ctx.fillStyle = 'gold';
        for (let star of this.stars) {
            game.ctx.beginPath();
            game.ctx.arc(star.x, star.y, star.radius, 0, Math.PI * 2);
            game.ctx.fill();
        }
    },

    initRain: function(opacity) {
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

    updateRain: function() {
        for (let drop of this.rainDrops) {
            drop.y += drop.speed;
            if (drop.y > game.canvas.height) {
                drop.y = -drop.length;
                drop.x = Math.random() * game.canvas.width;
            }
        }
    },

    drawRain: function() {
        game.ctx.strokeStyle = 'rgba(174, 194, 224, 0.2)';
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

    applyNightColorFilter: function() {
        game.ctx.save();
        game.ctx.globalCompositeOperation = 'source-over';
        game.ctx.fillStyle = this.nightColor;
        game.ctx.globalAlpha = this.nightOpacity;
        game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
        game.ctx.restore();
    },

    applyLightingEffects: function() {
        // Example lighting effect for windows or lanterns
        let lightSources = [
            { x: 100, y: 150, radius: 50, intensity: 0.8 },
            { x: 300, y: 400, radius: 30, intensity: 0.6 }
        ];
        game.ctx.save();
        game.ctx.globalCompositeOperation = 'lighter';
        lightSources.forEach(light => {
            let gradient = game.ctx.createRadialGradient(light.x, light.y, 0, light.x, light.y, light.radius);
            gradient.addColorStop(0, `rgba(255, 255, 150, ${light.intensity})`);
            gradient.addColorStop(1, 'rgba(255, 255, 150, 0)');
            game.ctx.fillStyle = gradient;
            game.ctx.fillRect(light.x - light.radius, light.y - light.radius, light.radius * 2, light.radius * 2);
        });
        game.ctx.restore();
    }
};
