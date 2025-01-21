sprite = {
    margin: 0,

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

        if (options.boundaryX !== undefined && options.boundaryY !== undefined) {
            newSprite.boundary = {
                x: options.boundaryX,
                y: options.boundaryY
            };
            setInterval(() => {
                const targetTileX = Math.floor(Math.random() * (newSprite.boundary.x + 1));
                const targetTileY = Math.floor(Math.random() * (newSprite.boundary.y + 1));
                newSprite.walkToClickedTile(targetTileX, targetTileY);
            }, 60000);
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

            let row = this.isVehicle 
                ? this.directionIndex 
                : (this.direction || 'S');

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
            utils.tracker('sprite.draw');
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

                let collisionResultX = collision.check(newX, this.y, this);
                let moveX = !collisionResultX.collisionDetected;

                let collisionResultY = collision.check(this.x, newY, this);
                let moveY = !collisionResultY.collisionDetected;

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
            utils.tracker('vehicle.move');
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
                utils.tracker('sprite.changeAnimation');
            }
        },

        addDirection: function(direction) {
            this.directions[direction] = true;
            this.updateDirection();
            this.moving = true;
            this.stopping = false;
        
            if (this.id === game.playerid) {
                audio.playAudio('footsteps1', assets.use('footsteps1'), 'sfx', true);
            }
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
            utils.tracker('sprite.animate');
        },

        walkToClickedTile: function(tileX, tileY) {
            const boundary = this.boundary;
            if (boundary && (tileX > boundary.x || tileY > boundary.y)) {
                return;
            }
        
            const currentX = Math.floor(this.x / 16);
            const currentY = Math.floor(this.y / 16);
            this.path = this.calculatePath(currentX, currentY, tileX, tileY);
            this.pathIndex = 0;
            this.isMovingToTarget = true;
            audio.playAudio("footsteps1", assets.use('footsteps1'), 'sfx', true);
            this.changeAnimation('speed_1');
            utils.tracker('sprite.walkToClickedTile');
        },

        calculatePath: function(startX, startY, endX, endY) {
            const grid = collision.walkableGridCache;
            const graph = new Graph(grid, { diagonal: true });
            const start = graph.grid[startX][startY];
            const end = graph.grid[endX][endY];
        
            if (grid[startX][startY] === 0 || grid[endX][endY] === 0) {
                console.log("Pathfinding: Start or end point is not walkable");
                return [];
            }
        
            const result = astar.search(graph, start, end);
            utils.tracker('sprite.calculatePath');
        
            if (result.length === 0) {
                console.log("Pathfinding: No path found");
                return [];
            }
        
            return result.map(function(node) {
                return { x: node.x, y: node.y, alpha: 1 };
            });
        },

        moveAlongPath: function() {
            if (!this.path || this.pathIndex >= this.path.length) {
                this.isMovingToTarget = false;
                this.moving = false;
                this.stopping = true;
                this.currentFrame = 0;
                this.path = [];
                audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
                return;
            }
        
            const nextStep = this.path[this.pathIndex];
            const targetX = nextStep.x * 16;
            const targetY = nextStep.y * 16;
            const deltaX = targetX - this.x;
            const deltaY = targetY - this.y;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
            const walkingPaceFactor = 0.6;
            this.speed = Math.max(10, this.topSpeed * walkingPaceFactor);
        
            if (distance < this.speed * (game.deltaTime / 1000)) {
                this.x = targetX;
                this.y = targetY;
                this.pathIndex++;
                if (this.pathIndex > 1) {
                    this.path.shift();
                    this.pathIndex--;
                }
            } else {
                const angle = Math.atan2(deltaY, deltaX);
                this.x += Math.cos(angle) * this.speed * (game.deltaTime / 1000);
                this.y += Math.sin(angle) * this.speed * (game.deltaTime / 1000);
        
                if (Math.abs(deltaX) > Math.abs(deltaY)) {
                    this.direction = deltaX > 0 ? 'E' : 'W';
                } else {
                    this.direction = deltaY > 0 ? 'S' : 'N';
                }
        
                if (Math.abs(deltaX) > 0 && Math.abs(deltaY) > 0) {
                    if (deltaX > 0 && deltaY > 0) this.direction = 'SE';
                    else if (deltaX > 0 && deltaY < 0) this.direction = 'NE';
                    else if (deltaX < 0 && deltaY > 0) this.direction = 'SW';
                    else if (deltaX < 0 && deltaY < 0) this.direction = 'NW';
                }
            }
            utils.tracker('sprite.moveAlongPath');
        },

        stopPathfinding: function() {
            this.path = [];
            this.pathIndex = 0;
            this.isMovingToTarget = false;
            this.moving = false;
            this.stopping = true;
            this.currentFrame = 0;
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
            console.log(`Pathfinding and movement stopped for sprite: ${this.id}`);
        },

        update: function() {
            const margin = 0;

            if (this.isMovingToTarget) {
                this.moveAlongPath();
            } else {
                let dx = 0;
                let dy = 0;

                if (this.directions['right']) dx += this.speed * (game.deltaTime / 1000);
                if (this.directions['left'])  dx -= this.speed * (game.deltaTime / 1000);
                if (this.directions['down'])  dy += this.speed * (game.deltaTime / 1000);
                if (this.directions['up'])    dy -= this.speed * (game.deltaTime / 1000);

                if (dx !== 0 && dy !== 0) {
                    const norm = Math.sqrt(dx * dx + dy * dy);
                    dx = (dx / norm) * this.speed * (game.deltaTime / 1000);
                    dy = (dy / norm) * this.speed * (game.deltaTime / 1000);
                }

                dx = isNaN(dx) ? 0 : dx;
                dy = isNaN(dy) ? 0 : dy;

                this.vx = dx;
                this.vy = dy;

                if (dx !== 0 || dy !== 0) {
                    let newX = this.x + this.vx;
                    let newY = this.y + this.vy;

                    newX = isNaN(newX) ? this.x : newX;
                    newY = isNaN(newY) ? this.y : newY;

                    let collisionResultX = collision.check(newX, this.y, this);
                    let moveX = !collisionResultX.collisionDetected;
                    let collisionResultY = collision.check(this.x, newY, this);
                    let moveY = !collisionResultY.collisionDetected;

                    if (moveX && moveY) {
                        this.x = newX;
                        this.y = newY;
                    } else if (moveX) {
                        this.x = newX;
                    } else if (moveY) {
                        this.y = newY;
                    }

                    this.x = Math.max(margin, Math.min(this.x, game.worldWidth - this.width * this.scale - margin));
                    this.y = Math.max(margin, Math.min(this.y, game.worldHeight - this.height * this.scale - margin));

                    this.moving = true;
                    this.stopping = false;
                } else {
                    this.moving = false;
                    this.stopping = true;
                }

                if (!this.moving && !this.overrideAnimation) {
                    this.changeAnimation('idle');
                } else if (!this.overrideAnimation) {
                    if (this.speed < 50) {
                        this.changeAnimation('speed_1');
                    } else if (this.speed < 140) {
                        this.changeAnimation('speed_2');
                    } else if (this.speed <= 170) {
                        this.changeAnimation('speed_3');
                    } else {
                        this.changeAnimation('speed_4');
                    }
                }

                if (this.overrideAnimation) {
                    this.changeAnimation(this.overrideAnimation);
                }
            }

            this.animate();

            if (this.id === game.playerid && ui.pluginExists('lighting')) {
                const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
                if (playerLight) {
                    playerLight.x = this.x + 8;
                    playerLight.y = this.y + 8;
                }
            }

            utils.tracker('sprite.update');
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
                        effects.shakeMap(300, 2);
                    }
                }
            }
        }
    }
};