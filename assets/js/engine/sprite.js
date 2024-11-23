var sprite = {
    margin: 0,
    create: function (options) {
        let newSprite = {
            id: options.id,
            width: options.isAnimal ? options.width || 48 : 16,
            height: options.isAnimal ? options.height || 32 : 32,
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
            directions: {},
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
        };

        Object.setPrototypeOf(newSprite, this.SpritePrototype);
    
        // Convert tile coordinates to pixel coordinates
        newSprite.x = options.x * 16;
        newSprite.y = options.y * 16;
    
        // Set the boundary for movement if provided (for NPCs)
        if (options.boundaryX !== undefined && options.boundaryY !== undefined) {
            newSprite.boundary = {
                x: options.boundaryX,
                y: options.boundaryY
            };
    
setInterval(() => {
    const targetTileX = Math.floor(Math.random() * (newSprite.boundary.x + 1)); // Random X within 0 to boundary.x
    const targetTileY = Math.floor(Math.random() * (newSprite.boundary.y + 1)); // Random Y within 0 to boundary.y
    newSprite.walkToClickedTile(targetTileX, targetTileY);
}, 60000); // Update position every 5 seconds


        }

        if (options.isPlayer) {
            console.log(`Adding light for player: ${newSprite.id}`);
            const lightColor = { r: 255, g: 255, b: 255 }; // White light
            const lightRadius = 65; // Adjust the radius as needed
            const lightIntensity = 0.15; // Adjust the intensity as needed
            lighting.addLight(newSprite.id + '_light', newSprite.x + 8, newSprite.y + 8, lightRadius, lightColor, lightIntensity, 'playerLight', true, 0, 0);
        }

        console.log(lighting.lights);
    
        // Automatically add the new sprite to the game.sprites object
        game.sprites[options.id] = newSprite;
    
        return newSprite;
    },

    SpritePrototype: {
draw: function() {
    const spriteData = assets.use('spritesData')[this.animalType || 'character'];

    if (!spriteData) {
        console.error(`Sprite data not found for type: ${this.animalType || 'character'}`);
        return;
    }

    const image = assets.use(spriteData.image);
    let direction = this.direction || 'S';
    let row = spriteData.directions[direction];

    // Handle flipped directions
    let flip = false;
    if (typeof row === 'string') {
        direction = row; // Use the flipped direction
        row = spriteData.directions[direction]; // Get the row for the flipped direction
        flip = true; // Mark the direction as flipped
    }

    if (row === undefined) {
        console.error(`Invalid direction '${direction}' for sprite type: ${this.animalType || 'character'}`);
        return;
    }

    const animation = spriteData.animations[this.currentAnimation];
    if (!animation) {
        console.error(`Animation '${this.currentAnimation}' not found for type: ${this.animalType || 'character'}`);
        return;
    }

    const frameIndex = animation.frames[this.currentFrame];
    const sx = (frameIndex - 1) * spriteData.width;
    const sy = row * spriteData.height;

    game.ctx.save();

    // Calculate the offset to center the sprite inside the collision box
    const offsetX = -spriteData.width / 2;
    const offsetY = -spriteData.height / 2;

    // Translate to the sprite's center point relative to the collision box
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

    // Draw the sprite
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
            game.ctx.save();
            game.ctx.translate(this.x, this.y + (this.height * this.scale / 2) - 14);
        
            // Calculate shadow size and opacity based on the current frame
            const shadowBaseWidth = this.width * this.scale * 0.4; // Reduced from 0.6
            const shadowBaseHeight = this.height * this.scale * 0.1; // Reduced from 0.2
            const shadowScaleFactor = 0.1; // Adjust this factor to control the shadow scaling intensity
            const shadowOpacityFactor = 0.03; // Adjust this factor to control the shadow opacity intensity
        
            // Calculate the scaling and opacity based on the current frame (simplified walking animation cycle)
            const frameFactor = Math.abs(Math.sin((this.currentFrame % 8) * (Math.PI / 4))); // Sine wave for smooth transition
            const shadowWidth = shadowBaseWidth + (shadowScaleFactor * frameFactor * shadowBaseWidth);
            const shadowHeight = shadowBaseHeight + (shadowScaleFactor * frameFactor * shadowBaseHeight);
            const shadowOpacity = 0.05 + (shadowOpacityFactor * frameFactor); // Base opacity plus dynamic component
        
            let shadowX = (this.width / 2) * this.scale; // Default center
            let shadowY = (this.height - 1) * this.scale - 2; // Default bottom
        
            game.ctx.shadowBlur = 15;
            game.ctx.fillStyle = `rgba(0, 0, 0, ${shadowOpacity})`; // Dynamic shadow opacity
            game.ctx.beginPath();
            game.ctx.ellipse(shadowX, shadowY, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
            game.ctx.fill();
        
            game.ctx.restore();
        },

changeAnimation: function(newAnimation) {
    // If an override animation is active, don't change the animation
    if (this.overrideAnimation && this.overrideAnimation !== newAnimation) {
        return;
    }

    const spriteData = assets.use('spritesData')[this.animalType || 'character'];

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
            this.speed = 80; // Normal speed when not running
        },
    
        updateHealth: function(amount) {
            if (typeof amount === "string") {
                amount = parseInt(amount);
            }
            this.health = Math.max(0, Math.min(this.maxHealth, this.health + amount));
        
            if (this.health <= 0) {
                // Handle sprite death
                console.log(`${this.id} has died.`);
                delete game.sprites[this.id]; // Remove the sprite from the game
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
    
animate: function () {
    const spriteData = assets.use('spritesData')[this.animalType || 'character'];

    if (!spriteData || !spriteData.animations) {
        console.error(`Animation data not found for sprite type: ${this.animalType || 'character'}`);
        return;
    }

    const animation = spriteData.animations[this.currentAnimation];
    if (!animation) {
        console.error(`Animation '${this.currentAnimation}' not found`);
        return;
    }

    // Calculate the frame duration based on animation speed
    const frameDuration = (1 / animation.speed) * (1000 / 60); // Duration in frames at 60fps
    this.frameCounter += game.deltaTime / frameDuration;

    // Loop the animation when the end is reached
    if (this.frameCounter >= animation.frames.length) {
        this.frameCounter = 0; // Reset to loop
    }

    // Update the current frame based on frameCounter
    this.currentFrame = Math.floor(this.frameCounter);
    utils.tracker('sprite.animate');

},

        walkToClickedTile: function(tileX, tileY) {
            const boundary = this.boundary;
            if (boundary && (tileX > boundary.x || tileY > boundary.y)) {
                return;
            }
        
            var currentX = Math.floor(this.x / 16);
            var currentY = Math.floor(this.y / 16);
            this.path = this.calculatePath(currentX, currentY, tileX, tileY);
            this.pathIndex = 0; // Reset the path index
            this.isMovingToTarget = true; // Mark sprite as moving to target
            audio.playAudio("footsteps1", assets.use('footsteps1'), 'sfx', true);
            this.changeAnimation('speed_1');
            utils.tracker('sprite.walkToClickedTile');
        },  
    
        calculatePath: function(startX, startY, endX, endY) {
            const grid = collision.walkableGridCache; // Use the cached walkable grid
            const graph = new Graph(grid, { diagonal: true });
            const start = graph.grid[startX][startY];
            const end = graph.grid[endX][endY];
        
            // Check if start and end points are walkable
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
        
            const path = result.map(function(node) {
                return { x: node.x, y: node.y, alpha: 1 }; // Add alpha property for opacity
            });
        
            return path; // Return the path to be stored in `this.path`
        },    
    
moveAlongPath: function() {
    if (!this.path || this.pathIndex >= this.path.length) {
        this.isMovingToTarget = false; // Stop moving when the path is exhausted
        this.moving = false;
        this.stopping = true;
        this.currentFrame = 0; // Reset to default standing position
        this.path = []; // Clear the path once the destination is reached
        audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        return;
    }

    const nextStep = this.path[this.pathIndex];
    const targetX = nextStep.x * 16;
    const targetY = nextStep.y * 16;
    const deltaX = targetX - this.x;
    const deltaY = targetY - this.y;
    const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

    // Determine walking speed relative to topSpeed
    const walkingPaceFactor = 0.6; // Walking speed is 60% of topSpeed
    this.speed = Math.max(10, this.topSpeed * walkingPaceFactor); // Ensure a minimum speed of 10

    if (distance < this.speed * (game.deltaTime / 1000)) {
        this.x = targetX;
        this.y = targetY;
        this.pathIndex++; // Move to the next step in the path

        // Remove the step behind the sprite    
        if (this.pathIndex > 1) {
            this.path.shift(); // Remove the previous path step
            this.pathIndex--; // Adjust the index
        }
    } else {
        const angle = Math.atan2(deltaY, deltaX);
        const newX = this.x + Math.cos(angle) * this.speed * (game.deltaTime / 1000);
        const newY = this.y + Math.sin(angle) * this.speed * (game.deltaTime / 1000);

        // Update position directly without collision checks
        this.x = newX;
        this.y = newY;

        // Determine direction based on movement
        if (Math.abs(deltaX) > Math.abs(deltaY)) {
            this.direction = deltaX > 0 ? 'E' : 'W';
        } else {
            this.direction = deltaY > 0 ? 'S' : 'N';
        }

        // Adjust direction for diagonal movement
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
            // Clear the path and reset path-related variables
            this.path = [];
            this.pathIndex = 0;
            this.isMovingToTarget = false;
            this.moving = false;
            this.stopping = true;
        
            // Reset the current frame to a default standing position
            this.currentFrame = 0;
        
            // Stop walking sound effect
            audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        
            // You can add any additional clean-up here if needed
            console.log(`Pathfinding and movement stopped for sprite: ${this.id}`);
        },

update: function(deltaTime) {
    const margin = 0; // Define a margin to keep the sprite away from the edges

    if (this.isMovingToTarget) {
        this.moveAlongPath();
    } else {
        let dx = 0;
        let dy = 0;

        if (this.directions['right']) dx += this.speed * (deltaTime / 1000);
        if (this.directions['left']) dx -= this.speed * (deltaTime / 1000);
        if (this.directions['down']) dy += this.speed * (deltaTime / 1000);
        if (this.directions['up']) dy -= this.speed * (deltaTime / 1000);

        if (dx !== 0 && dy !== 0) {
            const norm = Math.sqrt(dx * dx + dy * dy);
            dx = (dx / norm) * this.speed * (deltaTime / 1000);
            dy = (dy / norm) * this.speed * (deltaTime / 1000);
        }

        dx = isNaN(dx) ? 0 : dx;
        dy = isNaN(dy) ? 0 : dy;

        this.vx = dx;
        this.vy = dy;

        if (dx !== 0 || dy !== 0) {
            // Calculate new positions
            let newX = this.x + this.vx;
            let newY = this.y + this.vy;

            newX = isNaN(newX) ? this.x : newX;
            newY = isNaN(newY) ? this.y : newY;

            let moveX = true;
            let moveY = true;

            // Perform collision checks only if the sprite is moving
            const collisionResultX = collision.check(newX, this.y, this);
            moveX = !collisionResultX.collisionDetected;
            const collisionResultY = collision.check(this.x, newY, this);
            moveY = !collisionResultY.collisionDetected;

            // Update position based on collision results
            if (moveX && moveY) {
                this.x = newX;
                this.y = newY;
            } else if (moveX) {
                this.x = newX;
            } else if (moveY) {
                this.y = newY;
            }

            // Ensure the sprite stays within canvas boundaries
            this.x = Math.max(margin, Math.min(this.x, game.worldWidth - this.width * this.scale - margin));
            this.y = Math.max(margin, Math.min(this.y, game.worldHeight - this.height * this.scale - margin));

            this.moving = true;
            this.stopping = false;
        } else {
            this.moving = false;
            this.stopping = true;
        }

        // Adjust animation based on speed tiers
    if (!this.moving && !this.overrideAnimation) {
        this.changeAnimation('idle'); // Idle animation for 0 speed
    } else if (!this.overrideAnimation) {
        if (this.speed < 50) {
            this.changeAnimation('speed_1'); // Slow movement
        } else if (this.speed < 140) {
            this.changeAnimation('speed_2'); // Moderate speed
        } else if (this.speed <= 170) {
            this.changeAnimation('speed_3'); // Fast speed
        } else {
            this.changeAnimation('speed_4'); // Top speed
        }
    }

    if (this.overrideAnimation) {
        this.changeAnimation(this.overrideAnimation); // Override animation takes priority
    }
    }

    this.animate(); // Ensure the animation is updated in each cycle

    // Update light source position
    if (this.id === game.playerid) {
        const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
        if (playerLight) {
            playerLight.x = this.x + 8; // Center light on sprite
            playerLight.y = this.y + 8; // Center light on sprite
        }
    }

    utils.tracker('sprite.update');
},

        drawEnemyAttackAimTool: function() {
            const player = game.sprites[game.playerid];
            if (!player) return;
        
            const deltaX = player.x - this.x;
            const deltaY = player.y - this.y;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        
            // Only show the aim tool if within attack range
            if (distance > this.maxRange) return;
        
            // Calculate the adjusted target position
            let adjustedTargetX = player.x;
            let adjustedTargetY = player.y;
        
            // Check if within the enemy's max range
            if (distance <= this.maxRange) {
                adjustedTargetX = player.x;
                adjustedTargetY = player.y;
            } else {
                const ratio = this.maxRange / distance;
                adjustedTargetX = this.x + deltaX * ratio;
                adjustedTargetY = this.y + deltaY * ratio;
            }
        
            // Function to check for collision with non-walkable map objects
            const isObstructed = (x, y) => {
                if (game.roomData && game.roomData.items) {
                    for (const roomItem of game.roomData.items) {
                        const itemData = assets.use('objectData')[roomItem.id];
                        if (!itemData) continue;
        
                        const xCoordinates = roomItem.x || [];
                        const yCoordinates = roomItem.y || [];
        
                        for (let i = 0; i < xCoordinates.length; i++) {
                            const itemX = parseInt(xCoordinates[i], 10) * 16;
                            const itemY = parseInt(yCoordinates[i], 10) * 16;
                            const tileRect = {
                                x: itemX,
                                y: itemY,
                                width: 16,
                                height: 16
                            };
        
                            // Check if the point is within the tile's bounds
                            if (
                                x >= tileRect.x &&
                                x <= tileRect.x + tileRect.width &&
                                y >= tileRect.y &&
                                y <= tileRect.y + tileRect.height
                            ) {
                                const tileData = itemData[0]; // Assuming single tile data for simplicity
                                // Check if the tile is walkable
                                if (tileData.w !== 1) { // Assuming w=1 means walkable, otherwise it's not
                                    return { obstructed: true, collisionX: x, collisionY: y };
                                }
                            }
                        }
                    }
                }
                return { obstructed: false };
            };
        
            // Check for obstruction along the line of sight
            let finalTargetX = adjustedTargetX;
            let finalTargetY = adjustedTargetY;
            const steps = Math.ceil(distance);
            let obstructionDetected = false;
        
            for (let i = 1; i <= steps; i++) {
                const stepX = this.x + (deltaX * i) / steps;
                const stepY = this.y + (deltaY * i) / steps;
                const result = isObstructed(stepX, stepY);
                if (result.obstructed) {
                    finalTargetX = result.collisionX;
                    finalTargetY = result.collisionY;
                    obstructionDetected = true;
                    break;
                }
            }
        
            // Do not show the aim if the obstruction is very close
            if (obstructionDetected && Math.sqrt((finalTargetX - this.x) ** 2 + (finalTargetY - this.y) ** 2) < 10) {
                return;
            }
        
            // Draw the aim tool
            game.ctx.save();
        
            // Draw the line to the target
            game.ctx.strokeStyle = 'rgba(255, 0, 0, 0.8)';
            game.ctx.lineWidth = 1;
            game.ctx.setLineDash([5, 5]); // Dotted line
            game.ctx.beginPath();
            game.ctx.moveTo(this.x + this.width / 2, this.y + this.height / 2);
            game.ctx.lineTo(finalTargetX + player.width / 2, finalTargetY + player.height / 2);
            game.ctx.stroke();
            game.ctx.setLineDash([]); // Reset line dash
        
            // Draw target radius at the adjusted target position
            game.ctx.beginPath();
            game.ctx.arc(finalTargetX + player.width / 2, finalTargetY + player.height / 2, this.targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
        
            game.ctx.restore();
        },
    
        dealDamage: function() {
            const aimX = this.targetX;
            const aimY = this.targetY;
            const maxRadius = this.targetRadius;
        
            for (let id in game.sprites) {
                const sprite = game.sprites[id];
        
                if (sprite.isEnemy) {
                    const spriteCenterX = sprite.x + sprite.width / 2;
                    const spriteCenterY = sprite.y + sprite.height / 2;
                    const distance = Math.sqrt((aimX - spriteCenterX) ** 2 + (aimY - spriteCenterY) ** 2);
        
                    if (distance <= maxRadius) {
                        // Calculate damage based on distance, with maximum damage reduced to 50
                        const damage = Math.max(0, 10 - (distance / maxRadius) * 10);
                        sprite.updateHealth(-damage);
        
                        // Optionally, add some visual or audio feedback for the damage
                        console.log(`Enemy ${id} took ${damage.toFixed(2)} damage`);
                        effects.shakeMap(300, 2);
                    }
                }
            }
        },

        drawSelectedItem: function() {
            if (!this.currentItem) return;
        
            const itemData = game.itemsData.items.find(item => item.name === this.currentItem);
            if (!itemData) return;
        
            // Retrieve the direction-specific data
            const directionData = itemData.directions[this.direction];
            if (!directionData) return;
        
            // Use the angle and offset for the current direction
            const rotationAngle = (directionData.angle || 0) * Math.PI / 180;
            const itemXOffset = directionData.xOffset || 0;
            const itemYOffset = directionData.yOffset || 0;
        
            const itemScale = itemData.scale || 1;
            const iconSize = 16; // Assuming a 16x16 item icon
        
            game.ctx.save(); // Save the current canvas state
        
            // Translate to the item's center, apply offset, rotate, then draw the image
            game.ctx.translate(this.x + itemXOffset, this.y + itemYOffset);
            game.ctx.rotate(rotationAngle);
        
            // Apply an even stronger pulsating glow effect if the item has the `glow` property
            if (itemData.glow) {
                const time = Date.now();
                const glowStrength = 0.9 + 0.8 * Math.sin(time / 120); // Very strong pulsating effect
                game.ctx.shadowColor = `rgba(255, 255, 0, ${glowStrength})`; // Bright yellow glow
                game.ctx.shadowBlur = 60 + 40 * Math.sin(time / 120); // Very large dynamic blur for an intense glow
            }
        
            game.ctx.drawImage(
                game.itemsImg,
                itemData.x,
                itemData.y,
                iconSize,
                iconSize,
                -iconSize * itemScale / 2,
                -iconSize * itemScale / 2,
                iconSize * itemScale,
                iconSize * itemScale
            );
        
            game.ctx.restore(); // Restore the canvas state
        }

        // end of prototype
    }
};