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
        // Move downward
        snowflake.y += snowflake.speed;
        
        // Add sway movement
        snowflake.x += Math.sin((snowflake.y + snowflake.offset) * 0.01) * 
                       snowflake.sway * swayDirection;
        
        // Add horizontal drift
        snowflake.x += snowflake.horizontalDrift * snowflake.speed;

        // Handle melting
        if (!snowflake.meltdownTriggered && snowflake.y >= snowflake.meltdownStart) {
            snowflake.meltdownTriggered = true;
        }

        if (snowflake.meltdownTriggered) {
            snowflake.radius -= snowflake.meltdownRate;
            if (snowflake.radius <= 0) {
                resetSnowflake(snowflake);
            }
        }

        // Reset if off screen
        if (snowflake.y > canvasHeight) {
            resetSnowflake(snowflake);
        }

        // Wrap horizontally
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