snow = {
  snowflakes: [],
  maxSnowflakes: 2000,
  snowflakeSize: 0.5,
  swayDirection: -1,
  active: null,
  overrideActive: false,

  start: function() {
    this.create(0.6);
    this.active = true;
  },

  unmount: function() {
    // Add cleanup logic here if needed
  },

  onRender: function() {
    game.ctx.restore();
    this.draw();
    this.update();
  },

  create: function(opacity) {
    this.snowflakes = [];
    for (let i = 0; i < this.maxSnowflakes; i++) {
      // Assign meltdown properties and a slight horizontal drift factor
      let meltdownStart = Math.random() * game.canvas.height * 0.8; // random point before the bottom
      let meltdownRate = 0.002 + Math.random() * 0.003; // how quickly it shrinks
      let horizontalDrift = 0.05 + Math.random() * 0.05; // slight angle

      this.snowflakes.push({
        x: Math.random() * game.canvas.width,
        y: Math.random() * game.canvas.height,
        radius: this.snowflakeSize,
        speed: 0.6 + Math.random() * 0.6,
        sway: Math.random() * 0.5 + 0.1,
        offset: Math.random() * 1000,
        opacity: opacity,

        // Melting properties
        meltdownStart: meltdownStart,
        meltdownRate: meltdownRate,
        meltdownTriggered: false,

        // Additional angle drift
        horizontalDrift: horizontalDrift
      });
    }
  },

  stop: function() {
    this.snowflakes = [];
  },

  update: function() {
    if (!this.active) return;

    this.snowflakes.forEach((snowflake) => {
      // Move downward
      snowflake.y += snowflake.speed;
      // Add some sway
      snowflake.x += Math.sin((snowflake.y + snowflake.offset) * 0.01) * snowflake.sway * this.swayDirection;
      // Add a slight horizontal drift to make it fall at an angle
      snowflake.x += snowflake.horizontalDrift * snowflake.speed;

      // Check if we should trigger melting
      if (!snowflake.meltdownTriggered && snowflake.y >= snowflake.meltdownStart) {
        snowflake.meltdownTriggered = true;
      }

      // If melting is triggered, shrink the radius
      if (snowflake.meltdownTriggered) {
        snowflake.radius -= snowflake.meltdownRate;
        // Once it's too small, reset it to top
        if (snowflake.radius <= 0) {
          snowflake.y = -10;
          snowflake.x = Math.random() * game.canvas.width;
          snowflake.radius = this.snowflakeSize;
          snowflake.meltdownTriggered = false;
          // Reset meltdown start so it melts again in a new fall
          snowflake.meltdownStart = Math.random() * game.canvas.height * 0.8;
        }
      }

      // Reset if it goes off the bottom
      if (snowflake.y > game.canvas.height) {
        snowflake.y = -10;
        snowflake.x = Math.random() * game.canvas.width;
        snowflake.radius = this.snowflakeSize;
        snowflake.meltdownTriggered = false;
        snowflake.meltdownStart = Math.random() * game.canvas.height * 0.8;
      }

      // Wrap around horizontally if needed
      if (snowflake.x < 0) {
        snowflake.x = game.canvas.width;
      } else if (snowflake.x > game.canvas.width) {
        snowflake.x = 0;
      }
    });

    if (plugin.exists('debug')) debug.tracker('snow.update()');
  },

  draw: function() {
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

    if (plugin.exists('debug')) debug.tracker('snow.draw()');
  }
};