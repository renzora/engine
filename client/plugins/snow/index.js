window[id] = {
    id: id,
    snowflakes: [],
    maxSnowflakes: 3000,
    snowflakeSize: 0.5,
    swayDirection: -1,
    active: null,
    overrideActive: false,

    start: function() {
        this.create(0.6);
        this.active = true;
    },

    unmount: function() {
        console.log(`Snow Plugin unmounted: ${this.id}`);
    },

    onRender: function() {
        this.draw();
        this.update();
        game.ctx.save();
    },

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
        if (!this.active) return;
        this.snowflakes.forEach((snowflake) => {
            snowflake.y += snowflake.speed;
            snowflake.x += Math.sin(snowflake.y * 0.01) * snowflake.sway * this.swayDirection;

            if (snowflake.y > game.canvas.height) {
                snowflake.y = 0;
                snowflake.x = Math.random() * game.canvas.width;
            }
        });
        utils.tracker('snow.update()');
    },

    draw: function () {
        if (!this.active) return;
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
        utils.tracker('snow.draw()');
    }
};