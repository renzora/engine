var weather = {
    fireflys: [],
    rainDrops: [],
    snowflakes: [],
    fogs: [],
    maxSnowflakes: 1000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    snowActive: false,
    rainActive: false,
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
        utils.tracker('weather.updateSnow');
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
        utils.tracker('weather.updateFireflys');
    },

    drawFireflys: function() {
        if (!this.fireflysActive) return;
        game.ctx.fillStyle = 'gold';
        this.fireflys.forEach(star => {
            game.ctx.beginPath();
            game.ctx.arc(star.x, star.y, star.radius, 0, Math.PI * 2);
            game.ctx.fill();
        });
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
        this.rainDrops.forEach(drop => {
            game.ctx.globalAlpha = drop.opacity;
            game.ctx.beginPath();
            game.ctx.moveTo(drop.x, drop.y);
            game.ctx.lineTo(drop.x, drop.y + drop.length);
            game.ctx.stroke();
        });
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
        utils.tracker('weather.updateRain');
    },

    render: function () {
        this.drawSnow();
        this.drawRain();
        this.drawFireflys();
        lighting.drawGreyFilter();
        render.aimTool();
    },
};
