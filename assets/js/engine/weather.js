var weather = {
    fireflys: [],
    fireflyLights: {},
    rainDrops: [],
    snowflakes: [],
    maxSnowflakes: 1000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    cloudShadows: [],
    maxClouds: 30,
    cloudShadowSize: 32,
    cloudSpeed: 0.1,
    cloudsActive: true,
    snowActive: false,
    rainActive: false,
    fireflysActive: false,
    nightActive: false,

    createClouds: function() {
        this.cloudShadows = [];
        for (let i = 0; i < this.maxClouds; i++) {
            const cloudWidth = this.cloudShadowSize * (2 + Math.random() * 2); // Larger cloud width
            const cloudHeight = this.cloudShadowSize * (1.5 + Math.random()); // Larger cloud height
            const segments = Math.floor(3 + Math.random() * 5); // Number of overlapping circles
    
            const segmentsData = [];
            for (let j = 0; j < segments; j++) {
                segmentsData.push({
                    x: (Math.random() - 0.5) * cloudWidth * 0.6, // Random position offset within cloud width
                    y: (Math.random() - 0.5) * cloudHeight * 0.4, // Random position offset within cloud height
                    radius: Math.random() * this.cloudShadowSize * 0.5 + this.cloudShadowSize * 0.3 // Random size for each circle
                });
            }
    
            this.cloudShadows.push({
                x: Math.random() * game.worldWidth,
                y: Math.random() * game.worldHeight,
                width: cloudWidth,
                height: cloudHeight,
                speedX: Math.random() * this.cloudSpeed - this.cloudSpeed / 2, // Slight horizontal drift
                speedY: Math.random() * this.cloudSpeed, // Slow vertical movement
                segments: segmentsData // Store circle data for organic shape
            });
        }
    },
    

    updateClouds: function(deltaTime) {
        if (!this.cloudsActive) return;

        for (let cloud of this.cloudShadows) {
            cloud.x += cloud.speedX * deltaTime / 16;
            cloud.y += cloud.speedY * deltaTime / 16;

            // Wrap around the world
            if (cloud.x > game.worldWidth) cloud.x = -cloud.width;
            if (cloud.x + cloud.width < 0) cloud.x = game.worldWidth;
            if (cloud.y > game.worldHeight) cloud.y = -cloud.height;
        }
        utils.tracker('weather.updateClouds');
    },

    drawClouds: function() {
        if (!this.cloudsActive) return;
    
        game.ctx.save();
        game.ctx.globalAlpha = 0.03; // Semi-transparent shadows
        game.ctx.fillStyle = '#000'; // Shadow color
    
        for (let cloud of this.cloudShadows) {
            game.ctx.beginPath();
            // Use the segments data to create a fluffy cloud shape
            for (let segment of cloud.segments) {
                game.ctx.moveTo(cloud.x + segment.x, cloud.y + segment.y); // Move to circle center
                game.ctx.arc(
                    cloud.x + segment.x, // Circle's X position
                    cloud.y + segment.y, // Circle's Y position
                    segment.radius, // Circle radius
                    0,
                    Math.PI * 2
                );
            }
            game.ctx.closePath();
            game.ctx.fill(); // Fill the combined shape
        }
    
        game.ctx.restore();
    },
    

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
    },

    stopSnow: function() {
        this.snowflakes = [];
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

    createFireflys: function() {
        this.fireflys = [];
        this.fireflyLights = {}; // Reset lights
        for (let i = 0; i < 20; i++) {
            const firefly = {
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                radius: Math.random() * 0.3 + 0.1,
                twinkle: Math.random() * 0.02 + 0.01,
                speed: Math.random() * 0.1 + 0.05, // Slower speed for gentle movement
                direction: Math.random() * Math.PI * 2 // Random angle for movement direction
            };
            this.fireflys.push(firefly);

            // Create a light for each firefly
            const lightId = `firefly_${i}`;
            this.fireflyLights[lightId] = {
                id: lightId,
                x: firefly.x,
                y: firefly.y,
                lr: 31, // Example light radius
                color: { r: 255, g: 114, b: 8 }, // Yellow light
                li: 0.2, // Brightness
                flicker: true, // Optional flicker effect
                lfs: 0.02,
                lfa: 0.03
            };
        }
    },

    updateFireflys: function(deltaTime) {
        const margin = 50; // Extra margin outside the viewport
    
        for (let i = 0; i < this.fireflys.length; i++) {
            const firefly = this.fireflys[i];
            const lightId = `firefly_${i}`;
    
            // Twinkling effect
            firefly.radius += firefly.twinkle;
            if (firefly.radius > 0.3 || firefly.radius < 0.1) {
                firefly.twinkle = -firefly.twinkle;
            }
    
            // Move firefly based on its direction and speed
            firefly.x += Math.cos(firefly.direction) * firefly.speed * deltaTime / 16;
            firefly.y += Math.sin(firefly.direction) * firefly.speed * deltaTime / 16;
    
            // Randomly adjust direction slightly for subtle wandering
            firefly.direction += (Math.random() - 0.5) * 0.05;
    
            // Wrap around canvas boundaries
            if (firefly.x < -firefly.radius) firefly.x = game.canvas.width + firefly.radius;
            if (firefly.x > game.canvas.width + firefly.radius) firefly.x = -firefly.radius;
            if (firefly.y < -firefly.radius) firefly.y = game.canvas.height + firefly.radius;
            if (firefly.y > game.canvas.height + firefly.radius) firefly.y = -firefly.radius;
    
            // Check if the light exists in the lighting system
            let light = lighting.lights.find(l => l.id === lightId);
    
            if (!light) {
                // Create a new light if it doesn't exist
                light = {
                    id: lightId,
                    x: firefly.x,
                    y: firefly.y,
                    radius: 25,
                    color: { r: 194, g: 150, b: 0 },
                    intensity: 0.58,
                    flicker: true
                };
                lighting.addLight(light.id, light.x, light.y, light.radius, light.color, light.intensity, "firefly", true);
            } else {
                // Update the light position
                light.x = firefly.x;
                light.y = firefly.y;
            }
    
            // Viewport detection considering the firefly's radius and extra margin
            const isInViewport =
                firefly.x + firefly.radius + margin >= camera.cameraX &&
                firefly.x - firefly.radius - margin <= camera.cameraX + window.innerWidth / game.zoomLevel &&
                firefly.y + firefly.radius + margin >= camera.cameraY &&
                firefly.y - firefly.radius - margin <= camera.cameraY + window.innerHeight / game.zoomLevel;
    
            if (isInViewport) {
                // Add or update the light in the lighting system
                lighting.addLight(light.id, light.x, light.y, light.radius, light.color, light.intensity, "firefly", true);
            } else {
                // Remove light only if the entire light source + margin is out of the viewport
                lighting.lights = lighting.lights.filter(l => l.id !== lightId);
            }
        }
        utils.tracker('weather.updateFireflys');
    },
    


    drawFireflys: function() {
        if (!this.fireflysActive) return;
        game.ctx.save(); // Save the current state of the context
        this.fireflys.forEach(firefly => {
            // Draw outer ring (glow effect)
            game.ctx.beginPath();
            game.ctx.fillStyle = 'rgba(255, 223, 0, 0.2)'; // Semi-transparent yellow
            game.ctx.arc(firefly.x, firefly.y, firefly.radius * 2.5, 0, Math.PI * 2); // Larger radius for the glow
            game.ctx.fill();
            game.ctx.closePath();

            // Draw firefly core
            game.ctx.beginPath();
            game.ctx.fillStyle = 'gold'; // Bright core color
            game.ctx.shadowBlur = 10; // Create a glow effect
            game.ctx.shadowColor = 'rgba(255, 223, 0, 1)'; // Glow color (bright yellow)
            game.ctx.arc(firefly.x, firefly.y, firefly.radius, 0, Math.PI * 2);
            game.ctx.fill();
            game.ctx.closePath();
        });
        game.ctx.restore(); // Restore the original context state
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
    },
};
