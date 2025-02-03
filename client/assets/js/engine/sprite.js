sprite = {
  keys: {
    'ArrowUp': 'up',
    'ArrowLeft': 'left',
    'ArrowRight': 'right',
    'ArrowDown': 'down',
    'w': 'up',
    'a': 'left',
    's': 'down',
    'd': 'right'
  },

  init() {
    input.assign('keydown', e => this.handleKeyDownGlobal(e));
    input.assign('keyup', e => this.handleKeyUpGlobal(e));
    input.assign('mouseup', e => game.handleMouseUp(e));
  },

  handleKeyDownGlobal(e) {
    const playerSprite = game.sprites[game.playerid];
    if (!playerSprite) return;

    const dir = this.keys[e.key];
    if (dir) playerSprite.addDirection(dir);
  },

  handleKeyUpGlobal(e) {
    const playerSprite = game.sprites[game.playerid];
    if (!playerSprite) return;

    const dir = this.keys[e.key];
    if (dir) playerSprite.removeDirection(dir);
  },

  create(options) {
    const newSprite = {
      id: options.id,
      x: options.x || 0,
      y: options.y || 0,
      width: options.width || 16,
      height: options.height || 32,
      scale: 1,
      speed: options.speed || 0,
      topSpeed: options.topSpeed || 70,
      currentFrame: 0,
      frameCounter: 0,
      currentAnimation: options.currentAnimation || 'idle',
      direction: 'S',
      animationSpeed: options.animationSpeed || 0.2,
      overrideAnimation: null,
      moving: false,
      stopping: false,
      directions: {},
      type: options.type || 'character',
      active: options.active ?? true
    };
  
    if (options.boundaryX !== undefined && options.boundaryY !== undefined) {
      newSprite.boundary = { x: options.boundaryX, y: options.boundaryY };
    }
  
    Object.setPrototypeOf(newSprite, this.SpritePrototype);
    game.sprites[options.id] = newSprite;
    return newSprite;
  },

  SpritePrototype: {
    draw() {
      if (!this.active) return;

      const spriteData = assets.use('spriteData')[this.type];
      const image = assets.use(spriteData?.image);
      if (!spriteData || !image) return;

      let row = this.direction;
      let flip = false;

      row = spriteData.directions[row];
      if (typeof row === 'string') {
        row = spriteData.directions[row];
        flip = true;
      }

      const animation = spriteData.animations[this.currentAnimation];
      if (!animation) return;

      const frameIndex = animation.frames[this.currentFrame];
      const sx = (frameIndex - 1) * spriteData.width;
      const sy = row * spriteData.height;

      game.ctx.save();

      const offsetX = -spriteData.width / 2;
      const offsetY = -spriteData.height / 2;
      const translateX = Math.floor(this.x + this.width / 2);
      const translateY = Math.floor(this.y + this.height / 2);

      game.ctx.translate(translateX, translateY);
      if (flip) game.ctx.scale(-1, 1);

      game.ctx.drawImage(
        image,
        sx,
        sy,
        spriteData.width,
        spriteData.height,
        offsetX,
        offsetY,
        spriteData.width * this.scale,
        spriteData.height * this.scale
      );

      game.ctx.restore();
      plugin.debug.tracker('sprite.draw');
    },

    drawShadow() {
      if (!this.active) return;

      game.ctx.save();
      game.ctx.translate(this.x, this.y + (this.height * this.scale / 2) - 14);

      const frameFactor = Math.abs(Math.sin((this.currentFrame % 8) * (Math.PI / 4)));
      const shadowBaseWidth = this.width * this.scale * 0.4;
      const shadowBaseHeight = this.height * this.scale * 0.1;
      const shadowWidth = shadowBaseWidth + (0.1 * frameFactor * shadowBaseWidth);
      const shadowHeight = shadowBaseHeight + (0.1 * frameFactor * shadowBaseHeight);
      const shadowOpacity = 0.05 + (0.03 * frameFactor);
      const shadowX = (this.width / 2) * this.scale;
      const shadowY = (this.height - 1) * this.scale - 7;

      game.ctx.shadowBlur = 15;
      game.ctx.fillStyle = `rgba(0, 0, 0, ${shadowOpacity})`;
      game.ctx.beginPath();
      game.ctx.ellipse(shadowX, shadowY, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
      game.ctx.fill();
      game.ctx.restore();
    },

    changeAnimation(newAnimation) {
      if (this.overrideAnimation && this.overrideAnimation !== newAnimation) return;

      const spriteData = assets.use('spriteData')[this.type];
      if (!spriteData?.animations[newAnimation]) return;

      if (this.currentAnimation !== newAnimation) {
        this.currentAnimation = newAnimation;
        plugin.debug.tracker('sprite.changeAnimation');
      }
    },

    addDirection(direction) {
      this.directions[direction] = true;
      this.updateDirection();
      this.moving = true;
      this.stopping = false;
    },

    removeDirection(direction) {
      delete this.directions[direction];
      this.updateDirection();
      if (Object.keys(this.directions).length === 0) {
        this.stopping = true;
        this.moving = false;
      }
    },

    updateDirection() {
      const d = this.directions;
      if (d.up && d.right) this.direction = 'NE';
      else if (d.down && d.right) this.direction = 'SE';
      else if (d.down && d.left) this.direction = 'SW';
      else if (d.up && d.left) this.direction = 'NW';
      else if (d.up) this.direction = 'N';
      else if (d.down) this.direction = 'S';
      else if (d.left) this.direction = 'W';
      else if (d.right) this.direction = 'E';
    },

    animate() {
      const spriteData = assets.use('spriteData')[this.type];
      const animation = spriteData?.animations[this.currentAnimation];
      if (!animation) return;

      const frameDuration = (1 / animation.speed) * (1000 / 60);
      this.frameCounter += game.deltaTime / frameDuration;

      if (this.frameCounter >= animation.frames.length) {
        this.frameCounter = 0;
      }

      this.currentFrame = Math.floor(this.frameCounter);
      plugin.debug.tracker('sprite.animate');
    },

    update() {
      if (this.id === game.playerid && plugin.exists('lighting')) {
        const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
        if (playerLight) {
          playerLight.x = this.x + 8;
          playerLight.y = this.y + 8;
        }
      }

      if (this.isMovingToTarget && plugin.exists('pathfinding')) {
        plugin.pathfinding.moveAlongPath(this);
        this.animate();
        plugin.debug.tracker('sprite.update');
        return;
      }

      const dx = this.calculateDx();
      const dy = this.calculateDy();

      if (dx === 0 && dy === 0) {
        this.handleIdle();
        return;
      }

      this.updatePosition(dx, dy);
      this.updateAnimation();
      this.animate();
      plugin.debug.tracker('sprite.update');
    },

    calculateDx() {
      const timeScale = game.deltaTime / 1000;
      let dx = 0;
      if (this.directions.right) dx += this.speed * timeScale;
      if (this.directions.left) dx -= this.speed * timeScale;
      return dx;
    },

    calculateDy() {
      const timeScale = game.deltaTime / 1000;
      let dy = 0;
      if (this.directions.down) dy += this.speed * timeScale;
      if (this.directions.up) dy -= this.speed * timeScale;
      return dy;
    },

    handleIdle() {
      this.moving = false;
      this.stopping = true;
      if (!this.overrideAnimation) {
        this.changeAnimation('idle');
      }
      this.animate();
      plugin.debug.tracker('sprite.update');
    },

    updatePosition(dx, dy) {
      if (dx !== 0 && dy !== 0) {
        const norm = Math.sqrt(dx * dx + dy * dy);
        dx = (dx / norm) * this.speed * (game.deltaTime / 1000);
        dy = (dy / norm) * this.speed * (game.deltaTime / 1000);
      }

      const newX = this.x + dx;
      const newY = this.y + dy;

      if (plugin.exists('collision')) {
        const collisionResult = collision.check(newX, newY, this, this.x, this.y);
        if (!collisionResult.collisionDetected) {
          this.x = newX;
          this.y = newY;
        } else if (collisionResult.slideVector) {
          this.x += collisionResult.slideVector.x;
          this.y += collisionResult.slideVector.y;
        }
      } else {
        this.x = newX;
        this.y = newY;
      }

      const margin = 0;
      this.x = Math.max(margin, Math.min(this.x, game.worldWidth - this.width * this.scale - margin));
      this.y = Math.max(margin, Math.min(this.y, game.worldHeight - this.height * this.scale - margin));

      this.moving = true;
      this.stopping = false;
    },

    updateAnimation() {
      if (!this.overrideAnimation) {
        if (this.speed < 50) this.changeAnimation('speed_1');
        else if (this.speed < 140) this.changeAnimation('speed_2');
        else if (this.speed <= 170) this.changeAnimation('speed_3');
        else this.changeAnimation('speed_4');
      } else {
        this.changeAnimation(this.overrideAnimation);
      }
    }
  }
};