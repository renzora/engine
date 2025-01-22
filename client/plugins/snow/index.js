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

    },
  
    onRender: function() {
      game.ctx.restore();
      this.draw();
      this.update();
    },
  
    create: function(opacity) {
      this.snowflakes = [];
      for (let i = 0; i < this.maxSnowflakes; i++) {
        this.snowflakes.push({
          x: Math.random() * game.canvas.width,
          y: Math.random() * game.canvas.height,
          radius: this.snowflakeSize,
          speed: 0.6 + Math.random() * 0.6,
          sway: Math.random() * 0.5 + 0.1,
          offset: Math.random() * 1000,
          opacity: opacity,
        });
      }
    },
  
    stop: function() {
      this.snowflakes = [];
    },
  
    update: function() {
      if (!this.active) return;
  
      this.snowflakes.forEach((snowflake) => {
        snowflake.y += snowflake.speed;
        snowflake.x += Math.sin((snowflake.y + snowflake.offset) * 0.01) * snowflake.sway * this.swayDirection;
  
        if (snowflake.y > game.canvas.height) {
          snowflake.y = -10;
          snowflake.x = Math.random() * game.canvas.width;
        }
  
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
  