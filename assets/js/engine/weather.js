var weather = {
    clouds: {
        cloudShadows: [],
        maxClouds: 30,
        cloudShadowSize: 32,
        cloudSpeed: 0.1,
        cloudsActive: true,

        create: function () {
            this.cloudShadows = [];
            for (let i = 0; i < this.maxClouds; i++) {
                const cloudWidth = this.cloudShadowSize * (2 + Math.random() * 2);
                const cloudHeight = this.cloudShadowSize * (1.5 + Math.random());
                const segments = Math.floor(3 + Math.random() * 5);
                const segmentsData = [];
                for (let j = 0; j < segments; j++) {
                    segmentsData.push({
                        x: (Math.random() - 0.5) * cloudWidth * 0.6,
                        y: (Math.random() - 0.5) * cloudHeight * 0.4,
                        radius: Math.random() * this.cloudShadowSize * 0.5 + this.cloudShadowSize * 0.3,
                    });
                }

                this.cloudShadows.push({
                    x: Math.random() * game.worldWidth,
                    y: Math.random() * game.worldHeight,
                    width: cloudWidth,
                    height: cloudHeight,
                    speedX: Math.random() * this.cloudSpeed - this.cloudSpeed / 2,
                    speedY: Math.random() * this.cloudSpeed,
                    segments: segmentsData,
                });
            }
        },

        update: function (deltaTime) {
            if (!this.cloudsActive) return;

            for (let cloud of this.cloudShadows) {
                cloud.x += (cloud.speedX * deltaTime) / 16;
                cloud.y += (cloud.speedY * deltaTime) / 16;

                if (cloud.x > game.worldWidth) cloud.x = -cloud.width;
                if (cloud.x + cloud.width < 0) cloud.x = game.worldWidth;
                if (cloud.y > game.worldHeight) cloud.y = -cloud.height;
            }
            utils.tracker('weather.clouds.update()');
        },

        draw: function () {
            if (!this.cloudsActive) return;

            game.ctx.save();
            game.ctx.globalAlpha = 0.03;
            game.ctx.fillStyle = '#000';

            for (let cloud of this.cloudShadows) {
                game.ctx.beginPath();
                for (let segment of cloud.segments) {
                    game.ctx.moveTo(cloud.x + segment.x, cloud.y + segment.y);
                    game.ctx.arc(
                        cloud.x + segment.x,
                        cloud.y + segment.y,
                        segment.radius,
                        0,
                        Math.PI * 2
                    );
                }
                game.ctx.closePath();
                game.ctx.fill();
            }

            game.ctx.restore();
        },
    },

    snow: {
        snowflakes: [],
        maxSnowflakes: 3000,
        snowflakeSize: 0.5,
        swayDirection: -1,
        snowActive: false,

        create: function (opacity) {
            if (!this.snowActive) return;
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
        },
    },

    rain: {
        rainDrops: [],
        rainActive: false,

        create: function (opacity) {
            if (!this.rainActive) return;
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

    fireflies: {
        fireflys: [],
        fireflyLights: {},
        fireflysActive: false,

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
            utils.tracker('weather.fireflies.update()');
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
        },
    },

    render: function () {
        this.snow.draw();
        this.rain.draw();
        this.fireflies.draw();
        this.clouds.draw();
    },
};
