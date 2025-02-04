snow = {
    worker: new Worker('plugins/snow/worker.js'),
    snowflakes: [],
    active: false,
    overrideActive: false,
    snowflakeSize: 0.5,
    density: 'medium',

    getDensitySettings() {
        return {
            light: 2000,
            medium: 5000,
            heavy: 10000,
            blizzard: 20000
        };
    },

    start(density = 'medium') {
        this.active = true;
        this.density = density;
        
        this.worker.postMessage({
            type: 'init',
            canvasWidth: game.canvas.width,
            canvasHeight: game.canvas.height,
            maxSnowflakes: this.getDensitySettings()[density],
            snowflakeSize: this.snowflakeSize
        });

        this.worker.onmessage = (e) => {
            if (e.data.type === 'snowflakesUpdate') {
                this.snowflakes = e.data.snowflakes;
            }
        };
    },

    setDensity(density) {
        if (!this.getDensitySettings()[density]) return;
        
        this.density = density;
        this.worker.postMessage({
            type: 'updateDensity',
            maxSnowflakes: this.getDensitySettings()[density],
            snowflakeSize: this.snowflakeSize
        });
    },

    unmount() {
        if (this.worker) {
            this.worker.terminate();
            this.worker = null;
        }
        this.snowflakes = [];
        this.active = false;
    },

    onRender() {
        if (!this.active) return;
        
        game.ctx.restore();
        this.draw();
        
        this.worker.postMessage({
            type: 'update',
            canvasWidth: game.canvas.width,
            canvasHeight: game.canvas.height
        });
    },

    stop() {
        this.active = false;
        this.worker.postMessage({ type: 'stop' });
        this.snowflakes = [];
    },

    draw() {
        if (!this.active || !this.snowflakes) return;

        game.ctx.save();
        game.ctx.fillStyle = 'rgba(255, 255, 255, 1)';
        game.ctx.globalAlpha = 0.8;

        for (const snowflake of this.snowflakes) {
            game.ctx.beginPath();
            game.ctx.arc(snowflake.x, snowflake.y, snowflake.radius, 0, Math.PI * 2);
            game.ctx.closePath();
            game.ctx.fill();
        }

        game.ctx.restore();

        if (plugin.exists('debug')) debug.tracker('snow.draw()');
    }
};

let snowflakes = [];
let maxSnowflakes = 5000;
let snowflakeSize = 0.5;
let swayDirection = -1;
let active = false;
let canvasWidth = 0;
let canvasHeight = 0;

function createSnowflakes(opacity = 0.6) {
    snowflakes = [];
    for (let i = 0; i < maxSnowflakes; i++) {
        let meltdownStart = Math.random() * canvasHeight * 0.8;
        let meltdownRate = 0.002 + Math.random() * 0.003;
        let horizontalDrift = 0.05 + Math.random() * 0.05;

        snowflakes.push({
            x: Math.random() * canvasWidth,
            y: Math.random() * canvasHeight,
            radius: snowflakeSize,
            speed: 0.6 + Math.random() * 0.6,
            sway: Math.random() * 0.5 + 0.1,
            offset: Math.random() * 1000,
            opacity: opacity,
            meltdownStart,
            meltdownRate,
            meltdownTriggered: false,
            horizontalDrift
        });
    }
}

function updateSnowflakes() {
    if (!active) return;

    for (const snowflake of snowflakes) {
        snowflake.y += snowflake.speed;
        snowflake.x += Math.sin((snowflake.y + snowflake.offset) * 0.01) * 
                       snowflake.sway * swayDirection;
        snowflake.x += snowflake.horizontalDrift * snowflake.speed;

        if (!snowflake.meltdownTriggered && snowflake.y >= snowflake.meltdownStart) {
            snowflake.meltdownTriggered = true;
        }

        if (snowflake.meltdownTriggered) {
            snowflake.radius -= snowflake.meltdownRate;
            if (snowflake.radius <= 0) {
                resetSnowflake(snowflake);
            }
        }

        if (snowflake.y > canvasHeight) {
            resetSnowflake(snowflake);
        }

        if (snowflake.x < 0) {
            snowflake.x = canvasWidth;
        } else if (snowflake.x > canvasWidth) {
            snowflake.x = 0;
        }
    }
}

function resetSnowflake(snowflake) {
    snowflake.y = -10;
    snowflake.x = Math.random() * canvasWidth;
    snowflake.radius = snowflakeSize;
    snowflake.meltdownTriggered = false;
    snowflake.meltdownStart = Math.random() * canvasHeight * 0.8;
}

self.onmessage = function(e) {
    const { type } = e.data;

    switch (type) {
        case 'init':
            canvasWidth = e.data.canvasWidth;
            canvasHeight = e.data.canvasHeight;
            maxSnowflakes = e.data.maxSnowflakes;
            snowflakeSize = e.data.snowflakeSize;
            createSnowflakes();
            active = true;
            break;

        case 'updateDensity':
            const oldActive = active;
            active = false;
            maxSnowflakes = e.data.maxSnowflakes;
            snowflakeSize = e.data.snowflakeSize;
            createSnowflakes();
            active = oldActive;
            break;

        case 'update':
            canvasWidth = e.data.canvasWidth;
            canvasHeight = e.data.canvasHeight;
            updateSnowflakes();
            self.postMessage({
                type: 'snowflakesUpdate',
                snowflakes: snowflakes
            });
            break;

        case 'stop':
            active = false;
            snowflakes = [];
            break;
    }
};