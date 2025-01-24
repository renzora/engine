sprite = {
    margin: 0,
    keys: {
      'ArrowUp': "up",
      'ArrowLeft': "left",
      'ArrowRight': "right",
      'ArrowDown': "down",
      'w': "up",
      'a': "left",
      's': "down",
      'd': "right"
    },
  
    // 1) We add an `init` function so we can register key events via input.assign()
    init: function() {
      // Assign global keydown -> calls sprite.handleKeyDownGlobal
      input.assign('keydown', (e) => {
        this.handleKeyDownGlobal(e);
      });
  
      // Assign global keyup -> calls sprite.handleKeyUpGlobal
      input.assign('keyup', (e) => {
        this.handleKeyUpGlobal(e);
      });
  
      // Let the game handle mouse up:
      input.assign('mouseup', function(e) {
        game.handleMouseUp(e);
      });
    },
  
    // 2) Called whenever keydown happens anywhere.
    handleKeyDownGlobal: function(e) {
      const playerSprite = game.sprites[game.playerid];
      if (!playerSprite) return;
  
      const dir = this.keys[e.key];
      if (dir) {
        playerSprite.addDirection(dir);
      }
    },
  
    // 3) Same for keyup
    handleKeyUpGlobal: function(e) {
      const playerSprite = game.sprites[game.playerid];
      if (!playerSprite) return;
  
      const dir = this.keys[e.key];
      if (dir) {
        playerSprite.removeDirection(dir);
      }
    },
  
    create: function(options) {
      let newSprite = {
        id: options.id,
        width: options.isVehicle ? options.width || 44 : options.isAnimal ? options.width || 48 : 16,
        height: options.isVehicle ? options.height || 44 : options.isAnimal ? options.height || 32 : 32,
        scale: 1,
        speed: 0,
        topSpeed: options.topSpeed !== undefined ? options.topSpeed : 70,
        currentFrame: 0,
        frameCounter: 0,
        currentAnimation: options.currentAnimation !== undefined ? options.currentAnimation : 'idle',
        direction: 'S',
        animationSpeed: options.animationSpeed !== undefined ? options.animationSpeed : 0.2,
        overrideAnimation: null,
        moving: false,
        stopping: false,
        directions: {},
        path: [],
        pathIndex: 0,
        isMovingToTarget: false,
        isAnimal: options.isAnimal !== undefined ? options.isAnimal : false,
        animalType: options.animalType || null,
        riderId: null,
        body: options.body !== undefined ? options.body : 1,
        head: options.head !== undefined ? options.head : 1,
        eyes: options.eyes !== undefined ? options.eyes : 1,
        hair: options.hair !== undefined ? options.hair : 0,
        hands: options.hands !== undefined ? options.hands : 1,
        outfit: options.outfit !== undefined ? options.outfit : 0,
        facial: options.facial !== undefined ? options.facial : 0,
        hat: options.hat !== undefined ? options.hat : 0,
        glasses: options.glasses !== undefined ? options.glasses : 0,
        isEnemy: options.isEnemy || false,
        attack: options.attack || 10,
        defense: options.defense || 5,
        intensity: options.intensity || 1,
        targetAim: false,
        targetX: 0,
        targetY: 0,
        targetRadius: 10,
        maxRange: options.maxRange !== undefined ? options.maxRange : 30,
        currentItem: null,
        joystickDirections: {},
        isRunning: false,
        health: options.health !== undefined ? options.health : 100,
        maxHealth: options.maxHealth !== undefined ? options.maxHealth : 100,
        energy: options.energy !== undefined ? options.energy : 100,
        maxEnergy: options.maxEnergy !== undefined ? options.maxEnergy : 100,
        runningSpeed: 100,
        isCarrying: options.isCarrying !== undefined ? options.isCarrying : false,
        carriedItem: options.carriedItem !== undefined ? options.carriedItem : false,
        messages: options.messages || [],
        stopRadius: options.stopRadius !== undefined ? options.stopRadius : 30,
        activeSprite: options.activeSprite !== undefined ? options.activeSprite : true,
        canShoot: options.canShoot || false,
        isVehicle: options.isVehicle || false,
        directionIndex: options.directionIndex !== undefined ? options.directionIndex : 16,
        angle: 0,
        turnSpeed: options.turnSpeed || 1,
        steeringSensitivity: options.steeringSensitivity || 1,
        currentSpeed: 0,
        maxSpeed: options.maxSpeed || 120,
        acceleration: options.acceleration || 5,
        braking: options.braking || 10
      };
  
      Object.setPrototypeOf(newSprite, this.SpritePrototype);
  
      newSprite.x = options.x || 0;
      newSprite.y = options.y || 0;
      newSprite.angle = (newSprite.directionIndex / 48) * Math.PI * 2;
  
      // If boundary is set, you might do something else, but we won't auto-walk here
      if (options.boundaryX !== undefined && options.boundaryY !== undefined) {
        newSprite.boundary = {
          x: options.boundaryX,
          y: options.boundaryY
        };
      }
  
      game.sprites[options.id] = newSprite;
      return newSprite;
    },
  
    SpritePrototype: {
      draw: function() {
        if (!this.activeSprite) return;
  
        const spriteData = assets.use('spriteData')[this.animalType || 'character'];
        if (!spriteData) {
          console.error(`Sprite data not found for key: ${this.animalType || 'character'}`);
          return;
        }
  
        const image = assets.use(spriteData.image);
  
        let row = this.isVehicle ? this.directionIndex : (this.direction || 'S');
        let flip = false;
  
        if (!this.isVehicle) {
          row = spriteData.directions[row];
          if (typeof row === 'string') {
            row = spriteData.directions[row];
            flip = true;
          }
        }
  
        if (row === undefined) {
          console.error(`Invalid direction for sprite key: ${this.animalType || 'character'}`);
          return;
        }
  
        const animation = spriteData.animations[this.currentAnimation];
        if (!animation) {
          console.error(`Animation '${this.currentAnimation}' not found for key: ${this.animalType || 'character'}`);
          return;
        }
  
        const frameIndex = animation.frames[this.currentFrame];
        const sx = (frameIndex - 1) * spriteData.width;
        const sy = row * spriteData.height;
  
        game.ctx.save();
  
        const offsetX = -spriteData.width / 2;
        const offsetY = -spriteData.height / 2;
  
        if (flip) {
          game.ctx.translate(
            Math.floor(this.x + this.width / 2),
            Math.floor(this.y + this.height / 2)
          );
          game.ctx.scale(-1, 1);
        } else {
          game.ctx.translate(
            Math.floor(this.x + this.width / 2),
            Math.floor(this.y + this.height / 2)
          );
        }
  
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
        if (plugin.exists('debug')) debug.tracker('sprite.draw');
      },
  
      drawShadow: function() {
        if (!this.activeSprite) return;
        game.ctx.save();
  
        if (this.isVehicle) {
          const centerX = this.x + (this.width * this.scale / 2);
          const centerY = this.y + (this.height * this.scale / 2);
          game.ctx.translate(centerX, centerY);
          game.ctx.rotate(this.angle);
  
          const shadowWidth = this.width * this.scale * 0.5;
          const shadowHeight = this.height * this.scale * 0.3;
  
          game.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
          game.ctx.beginPath();
          game.ctx.ellipse(0, 0, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
          game.ctx.fill();
        } else {
          game.ctx.translate(this.x, this.y + (this.height * this.scale / 2) - 14);
  
          const shadowBaseWidth = this.width * this.scale * 0.4;
          const shadowBaseHeight = this.height * this.scale * 0.1;
          const shadowScaleFactor = 0.1;
          const shadowOpacityFactor = 0.03;
  
          const frameFactor = Math.abs(Math.sin((this.currentFrame % 8) * (Math.PI / 4)));
          const shadowWidth = shadowBaseWidth + (shadowScaleFactor * frameFactor * shadowBaseWidth);
          const shadowHeight = shadowBaseHeight + (shadowScaleFactor * frameFactor * shadowBaseHeight);
          const shadowOpacity = 0.05 + (shadowOpacityFactor * frameFactor);
  
          const shadowX = (this.width / 2) * this.scale;
          const shadowY = (this.height - 1) * this.scale - 7;
  
          game.ctx.shadowBlur = 15;
          game.ctx.fillStyle = `rgba(0, 0, 0, ${shadowOpacity})`;
          game.ctx.beginPath();
          game.ctx.ellipse(shadowX, shadowY, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
          game.ctx.fill();
        }
  
        game.ctx.restore();
      },
  
      updateVehicleDirection: function(turnStrength, deltaTime) {
        const turnRate = this.steeringSensitivity * (this.currentSpeed / this.maxSpeed);
        this.angle += turnStrength * turnRate * (deltaTime / 1000);
  
        if (Math.abs(turnStrength) > 0.01) {
          const turnDeceleration = 10;
          this.currentSpeed = Math.max(
            0,
            this.currentSpeed - turnDeceleration * Math.abs(turnStrength) * (deltaTime / 1000)
          );
        }
  
        this.angle %= (2 * Math.PI);
        if (this.angle < 0) {
          this.angle += 2 * Math.PI;
        }
  
        this.directionIndex = Math.round((this.angle / (2 * Math.PI)) * 48) % 48;
      },
  
      moveVehicle: function() {
        if (this.currentSpeed !== 0) {
          let newX = this.x + Math.cos(this.angle) * this.currentSpeed * (game.deltaTime / 1000);
          let newY = this.y + Math.sin(this.angle) * this.currentSpeed * (game.deltaTime / 1000);
  
          // Default moveX/moveY to true if collision plugin doesn't exist
          let moveX = true;
          let moveY = true;
  
          if (plugin.exists('collision')) {
            let collisionResultX = collision.check(newX, this.y, this);
            moveX = !collisionResultX.collisionDetected;
  
            let collisionResultY = collision.check(this.x, newY, this);
            moveY = !collisionResultY.collisionDetected;
          }
  
          if (moveX && moveY) {
            this.x = newX;
            this.y = newY;
          } else if (moveX) {
            this.x = newX;
          } else if (moveY) {
            this.y = newY;
          }
  
          const margin = 0;
          this.x = Math.max(
            margin, 
            Math.min(this.x, game.worldWidth - this.width * this.scale - margin)
          );
          this.y = Math.max(
            margin, 
            Math.min(this.y, game.worldHeight - this.height * this.scale - margin)
          );
        }
        if (plugin.exists('debug')) debug.tracker('vehicle.move');
      },
  
      changeAnimation: function(newAnimation) {
        if (this.overrideAnimation && this.overrideAnimation !== newAnimation) {
          return;
        }
  
        const spriteData = assets.use('spriteData')[this.animalType || 'character'];
        if (!spriteData || !spriteData.animations[newAnimation]) {
          console.error(`Animation '${newAnimation}' not found for type: ${this.animalType || 'character'}`);
          return;
        }
  
        if (this.currentAnimation !== newAnimation) {
          this.currentAnimation = newAnimation;
          if (plugin.exists('debug')) debug.tracker('sprite.changeAnimation');
        }
      },
  
      // Direction methods for keyboard movement:
      addDirection: function(direction) {
        this.directions[direction] = true;
        this.updateDirection();
        this.moving = true;
        this.stopping = false;
      },
  
      removeDirection: function(direction) {
        delete this.directions[direction];
        this.updateDirection();
        if (Object.keys(this.directions).length === 0) {
          this.stopping = true;
          this.moving = false;
        }
      },
  
      updateDirection: function() {
        if (this.directions['up'] && this.directions['right']) this.direction = 'NE';
        else if (this.directions['down'] && this.directions['right']) this.direction = 'SE';
        else if (this.directions['down'] && this.directions['left']) this.direction = 'SW';
        else if (this.directions['up'] && this.directions['left']) this.direction = 'NW';
        else if (this.directions['up']) this.direction = 'N';
        else if (this.directions['down']) this.direction = 'S';
        else if (this.directions['left']) this.direction = 'W';
        else if (this.directions['right']) this.direction = 'E';
      },
  
      startRunning: function() {
        this.isRunning = true;
        this.speed = this.runningSpeed;
      },
  
      stopRunning: function() {
        this.isRunning = false;
        this.speed = 80;
      },
  
      updateHealth: function(amount) {
        if (typeof amount === "string") {
          amount = parseInt(amount);
        }
        this.health = Math.max(0, Math.min(this.maxHealth, this.health + amount));
  
        if (this.health <= 0) {
          console.log(`${this.id} has died.`);
          delete game.sprites[this.id];
        }
  
        const healthBar = document.getElementById('ui_health');
        if (healthBar && this.id === game.playerid) {
          const healthPercentage = (this.health / this.maxHealth) * 100;
          healthBar.style.width = healthPercentage + '%';
          healthBar.nextElementSibling.innerText = `${Math.round(healthPercentage)}%`;
        }
      },
  
      updateEnergy: function(amount) {
        if (typeof amount === "string") {
          amount = parseInt(amount);
        }
        this.energy = Math.max(0, Math.min(this.maxEnergy, this.energy + amount));
  
        const energyBar = document.getElementById('ui_energy');
        if (energyBar) {
          const energyPercentage = (this.energy / this.maxEnergy) * 100;
          energyBar.style.width = energyPercentage + '%';
          energyBar.nextElementSibling.innerText = `${Math.round(energyPercentage)}%`;
        }
      },
  
      animate: function() {
        const spriteData = assets.use('spriteData')[this.animalType || 'character'];
        if (!spriteData || !spriteData.animations) {
          console.error(`Animation data not found for sprite type: ${this.animalType || 'character'}`);
          return;
        }
  
        const animation = spriteData.animations[this.currentAnimation];
        if (!animation) {
          console.error(`Animation '${this.currentAnimation}' not found`);
          return;
        }
  
        const frameDuration = (1 / animation.speed) * (1000 / 60);
        this.frameCounter += game.deltaTime / frameDuration;
  
        if (this.frameCounter >= animation.frames.length) {
          this.frameCounter = 0;
        }
  
        this.currentFrame = Math.floor(this.frameCounter);
        if (plugin.exists('debug')) debug.tracker('sprite.animate');
      },
  
      // No direct pathfinding methods here, but if we detect isMovingToTarget:
      update: function() {
        // If the pathfinding plugin is controlling movement:
        if (this.isMovingToTarget && plugin.exists('pathfinding')) {
          // Let the plugin move this sprite's position
          plugin.pathfinding.moveAlongPath(this);
  
          // We still animate, so we do that after
          this.animate();
  
          // Optionally sync lighting if this is the player
          if (this.id === game.playerid && plugin.exists('lighting')) {
            const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
            if (playerLight) {
              playerLight.x = this.x + 8;
              playerLight.y = this.y + 8;
            }
          }
  
          if (plugin.exists('debug')) debug.tracker('sprite.update');
          return;
        }
  
        // Otherwise, standard WASD/keyboard movement:
        let dx = 0, dy = 0;
        if (this.directions['right']) dx += this.speed * (game.deltaTime / 1000);
        if (this.directions['left'])  dx -= this.speed * (game.deltaTime / 1000);
        if (this.directions['down'])  dy += this.speed * (game.deltaTime / 1000);
        if (this.directions['up'])    dy -= this.speed * (game.deltaTime / 1000);
  
        // Normalize diagonal
        if (dx !== 0 && dy !== 0) {
          const norm = Math.sqrt(dx * dx + dy * dy);
          dx = (dx / norm) * this.speed * (game.deltaTime / 1000);
          dy = (dy / norm) * this.speed * (game.deltaTime / 1000);
        }
  
        // If no movement => idle
        if (dx === 0 && dy === 0) {
          this.moving = false;
          this.stopping = true;
  
          if (!this.overrideAnimation) {
            this.changeAnimation('idle');
          }
          this.animate();
  
          if (this.id === game.playerid && plugin.exists('lighting')) {
            const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
            if (playerLight) {
              playerLight.x = this.x + 8;
              playerLight.y = this.y + 8;
            }
          }
          if (plugin.exists('debug')) debug.tracker('sprite.update');
          return;
        }
  
        // Otherwise apply movement (with collision checks if plugin exists)
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
  
        // Clamp final position
        const margin = 0;
        this.x = Math.max(
          margin, 
          Math.min(this.x, game.worldWidth - this.width * this.scale - margin)
        );
        this.y = Math.max(
          margin, 
          Math.min(this.y, game.worldHeight - this.height * this.scale - margin)
        );
  
        this.moving = true;
        this.stopping = false;
  
        // Animation based on speed
        if (!this.overrideAnimation) {
          if (this.speed < 50) {
            this.changeAnimation('speed_1');
          } else if (this.speed < 140) {
            this.changeAnimation('speed_2');
          } else if (this.speed <= 170) {
            this.changeAnimation('speed_3');
          } else {
            this.changeAnimation('speed_4');
          }
        } else {
          this.changeAnimation(this.overrideAnimation);
        }
  
        // Always animate
        this.animate();
  
        // Handle lighting
        if (this.id === game.playerid && plugin.exists('lighting')) {
          const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
          if (playerLight) {
            playerLight.x = this.x + 8;
            playerLight.y = this.y + 8;
          }
        }
        if (plugin.exists('debug')) debug.tracker('sprite.update');
      },
  
      dealDamage: function() {
        const aimX = this.targetX;
        const aimY = this.targetY;
        const maxRadius = this.targetRadius;
  
        for (let id in game.sprites) {
          const targetSprite = game.sprites[id];
          if (targetSprite.isEnemy) {
            const spriteCenterX = targetSprite.x + targetSprite.width / 2;
            const spriteCenterY = targetSprite.y + targetSprite.height / 2;
            const distance = Math.sqrt((aimX - spriteCenterX) ** 2 + (aimY - spriteCenterY) ** 2);
  
            if (distance <= maxRadius) {
              const damage = Math.max(0, 10 - (distance / maxRadius) * 10);
              targetSprite.updateHealth(-damage);
              console.log(`Enemy ${id} took ${damage.toFixed(2)} damage`);
              if (plugin.exists('effects')) effects.shakeMap(300, 2);
            }
          }
        }
      }
    }
  };
  