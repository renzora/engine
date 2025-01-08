window[id] = {
    id: id,

start: function() {
    this.rain.create(0.7);
    this.fireflys.create();
    this.snow.create(0.6);
},

unmount: function() {
    console.log(`Weather Plugin unmounted: ${this.id}`);
},

snow: {
    snowflakes: [],
    maxSnowflakes: 3000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    snowActive: null,
    overrideActive: false,

    create: function (opacity) {
        console.log('creating snow');
        this.snowflakes = [];
        for (let i = 0; i < this.maxSnowflakes; i++) {
            this.snowflakes.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: this.snowflakeSize,
                speed: 0.8,
                sway: Math.random() * 0.5 + 0.1,
                opacity: opacity,
            });
        }
    },

    stop: function () {
        this.snowflakes = [];
    },

    update: function () {
        if (!this.snowActive) return;
        this.snowflakes.forEach((snowflake) => {
            snowflake.y += snowflake.speed;
            snowflake.x += Math.sin(snowflake.y * 0.01) * snowflake.sway * this.swayDirection;

            if (snowflake.y > game.canvas.height) {
                snowflake.y = 0;
                snowflake.x = Math.random() * game.canvas.width;
            }
        });
        utils.tracker('weather.snow.update()');
    },

    draw: function () {
        if (!this.snowActive) return;
        game.ctx.save();
        game.ctx.fillStyle = 'rgba(255, 255, 255, 1)';
        game.ctx.globalAlpha = 0.8;
        this.snowflakes.forEach((snowflake) => {
            game.ctx.beginPath();
            game.ctx.arc(snowflake.x, snowflake.y, snowflake.radius, 0, Math.PI * 2);
            game.ctx.closePath();
            game.ctx.fill();
        });
        game.ctx.restore();
        utils.tracker('weather.snow.draw()');
    },
},

rain: {
    rainDrops: [],
    rainActive: null,
    overrideActive: false,

    create: function (opacity) {
        this.rainDrops = [];
        for (let i = 0; i < 1000; i++) {
            this.rainDrops.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                length: Math.random() * 8,
                opacity: Math.random() * opacity,
                speed: 7,
            });
        }
    },

    update: function () {
        if (!this.rainActive) return;
        for (let drop of this.rainDrops) {
            drop.y += drop.speed;
            if (drop.y > game.canvas.height) {
                drop.y = -drop.length;
                drop.x = Math.random() * game.canvas.width;
            }
        }
        utils.tracker('weather.rain.update()');
    },

    draw: function () {
        if (!this.rainActive) return;
        game.ctx.strokeStyle = 'rgba(174, 194, 224, 0.4)';
        game.ctx.lineWidth = 1;
        game.ctx.lineCap = 'round';
        this.rainDrops.forEach((drop) => {
            game.ctx.globalAlpha = drop.opacity;
            game.ctx.beginPath();
            game.ctx.moveTo(drop.x, drop.y);
            game.ctx.lineTo(drop.x, drop.y + drop.length);
            game.ctx.stroke();
        });
        game.ctx.globalAlpha = 1;
    },
},

fireflys: {
    fireflys: [],
    fireflyLights: {},
    fireflysActive: null,
    overrideActive: false,

    create: function () {
        this.fireflys = [];
        this.fireflyLights = {};
        for (let i = 0; i < 20; i++) {
            const firefly = {
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: Math.random() * 0.3 + 0.1,
                twinkle: Math.random() * 0.02 + 0.01,
                speed: Math.random() * 0.1 + 0.05,
                direction: Math.random() * Math.PI * 2,
            };
            this.fireflys.push(firefly);

            const lightId = `firefly_${i}`;
            this.fireflyLights[lightId] = {
                id: lightId,
                x: firefly.x,
                y: firefly.y,
                lr: 48,
                color: { r: 223, g: 152, b: 48 },
                li: 0.2,
                flicker: false,
                lfs: 0,
                lfa: 0,
            };
        }
    },

    update: function (deltaTime) {
    if (!this.fireflysActive) {
        lighting.lights = lighting.lights.filter(light => !light.id.startsWith("firefly_"));
        return;
    }

        if (this.fireflys.length === 0) {
        this.create();
    }
    
    const margin = 50;

    for (let i = 0; i < this.fireflys.length; i++) {
        const firefly = this.fireflys[i];
        const lightId = `firefly_${i}`;

        firefly.radius += firefly.twinkle;
        if (firefly.radius > 0.3 || firefly.radius < 0.1) {
            firefly.twinkle = -firefly.twinkle;
        }

        firefly.x += Math.cos(firefly.direction) * firefly.speed * deltaTime / 16;
        firefly.y += Math.sin(firefly.direction) * firefly.speed * deltaTime / 16;
        firefly.direction += (Math.random() - 0.5) * 0.05;

        if (firefly.x < -firefly.radius) firefly.x = game.canvas.width + firefly.radius;
        if (firefly.x > game.canvas.width + firefly.radius) firefly.x = -firefly.radius;
        if (firefly.y < -firefly.radius) firefly.y = game.canvas.height + firefly.radius;
        if (firefly.y > game.canvas.height + firefly.radius) firefly.y = -firefly.radius;

        let light = lighting.lights.find(l => l.id === lightId);

        if (!light) {
            light = {
                id: lightId,
                x: firefly.x,
                y: firefly.y,
                radius: 48,
                color: { r: 223, g: 152, b: 48 },
                intensity: 0.58,
                flicker: false
            };
            lighting.addLight(light.id, light.x, light.y, light.radius, light.color, light.intensity, "firefly", true);
        } else {
            light.x = firefly.x;
            light.y = firefly.y;
        }

        const isInViewport =
            firefly.x + firefly.radius + margin >= camera.cameraX &&
            firefly.x - firefly.radius - margin <= camera.cameraX + window.innerWidth / game.zoomLevel &&
            firefly.y + firefly.radius + margin >= camera.cameraY &&
            firefly.y - firefly.radius - margin <= camera.cameraY + window.innerHeight / game.zoomLevel;

        if (isInViewport) {
            lighting.addLight(light.id, light.x, light.y, light.radius, light.color, light.intensity, "firefly", true);
        } else {
            lighting.lights = lighting.lights.filter(l => l.id !== lightId);
        }
    }
    utils.tracker('weather.fireflys.update()');
},

    draw: function () {
        if (!this.fireflysActive) return;
        game.ctx.save();
        this.fireflys.forEach((firefly) => {
            game.ctx.beginPath();
            game.ctx.fillStyle = 'rgba(255, 223, 0, 0.2)';
            game.ctx.arc(firefly.x, firefly.y, firefly.radius * 2.5, 0, Math.PI * 2);
            game.ctx.fill();
            game.ctx.closePath();
            game.ctx.beginPath();
            game.ctx.fillStyle = 'gold';
            game.ctx.shadowBlur = 10;
            game.ctx.shadowColor = 'rgba(255, 223, 0, 1)';
            game.ctx.arc(firefly.x, firefly.y, firefly.radius, 0, Math.PI * 2);
            game.ctx.fill();
            game.ctx.closePath();
        });
        game.ctx.restore();
    }
}
};