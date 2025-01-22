window[id] = {
    id: id,
    fireflies: [],
    fireflyLights: {},
    active: null,
    overrideActive: false,

    start: function() {
        this.create();
    },

    unmount: function() {
        console.log(`Weather Plugin unmounted: ${this.id}`);
    },

    onRender: function() {
        game.ctx.restore();
        this.draw();
        this.update();
    },

    create: function () {
        this.fireflies = [];
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
            this.fireflies.push(firefly);

            if(plugin.exists('lighting') && plugin.exists('time')) {

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
    }
    },

    update: function () {
    if (!this.active && plugin.exists('lighting')) {
        lighting.lights = lighting.lights.filter(light => !light.id.startsWith("firefly_"));
        return;
    }

        if (this.fireflies.length === 0) {
        this.create();
    }
    
    const margin = 50;

    for (let i = 0; i < this.fireflies.length; i++) {
        const firefly = this.fireflies[i];
        const lightId = `firefly_${i}`;

        firefly.radius += firefly.twinkle;
        if (firefly.radius > 0.3 || firefly.radius < 0.1) {
            firefly.twinkle = -firefly.twinkle;
        }

        firefly.x += Math.cos(firefly.direction) * firefly.speed * game.deltaTime / 16;
        firefly.y += Math.sin(firefly.direction) * firefly.speed * game.deltaTime / 16;
        firefly.direction += (Math.random() - 0.5) * 0.05;

        if (firefly.x < -firefly.radius) firefly.x = game.canvas.width + firefly.radius;
        if (firefly.x > game.canvas.width + firefly.radius) firefly.x = -firefly.radius;
        if (firefly.y < -firefly.radius) firefly.y = game.canvas.height + firefly.radius;
        if (firefly.y > game.canvas.height + firefly.radius) firefly.y = -firefly.radius;

        if(plugin.exists('lighting')) {
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
    }
    if(plugin.exists('debug')) debug.tracker('fireflies.update()');
},

    draw: function () {
        if (!this.active) return;
        game.ctx.save();
        this.fireflies.forEach((firefly) => {
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
};