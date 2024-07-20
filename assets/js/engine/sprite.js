var sprite = {
    margin: 0,
    create: function (options) {
        let newSprite = {
            id: options.id,
            width: 16,
            height: 27,
            scale: 0.95,
            speed: options.speed !== undefined ? options.speed : 70,
            currentFrame: 0,
            direction: 'S',
            animationSpeed: 0.2,
            frameCounter: 0,
            moving: false,
            stopping: false,
            directions: {},
            body: options.body !== undefined ? options.body : 1,
            head: options.head !== undefined ? options.head : 1,
            eyes: options.eyes !== undefined ? options.eyes : 1,
            hair: options.hair !== undefined ? options.hair : 0,
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
            maxRange: 80,
            handOffsetX: -5,
            handOffsetY: 5,
            currentItem: 'axe',
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
            directionMap: {
                'S': 0,
                'E': 6,
                'N': 12,
                'W': 6,
                'SE': 18,
                'SW': 18,
                'NE': 24,
                'NW': 24
            },
            eyeFrame: 0,
            eyeFrameCounter: 0,
            eyeBlinkInterval: Math.random() * 3000 + 2000, // Random interval between 2 and 5 seconds
            draw: this.draw,
            drawShadow: this.drawShadow,
            addDirection: this.addDirection,
            removeDirection: this.removeDirection,
            updateDirection: this.updateDirection,
            startRunning: this.startRunning,
            stopRunning: this.stopRunning,
            updateHealth: this.updateHealth,
            updateEnergy: this.updateEnergy,
            animate: this.animate,
            animateEyes: this.animateEyes,
            walkToClickedTile: this.walkToClickedTile,
            calculatePath: this.calculatePath,
            moveAlongPath: this.moveAlongPath,
            update: this.update,
            takeDamage: this.takeDamage,
            die: this.die,
            attackTarget: this.attackTarget,
            chasePlayer: this.chasePlayer,
            handleAimAttack: this.handleAimAttack,
            checkTileActions: this.checkTileActions
        };
    
        // Convert tile coordinates to pixel coordinates
        newSprite.x = options.x * 16;
        newSprite.y = options.y * 16;
    
        // Set the boundary for movement if provided (for NPCs)
        if (options.boundaryX !== undefined && options.boundaryY !== undefined) {
            newSprite.boundary = {
                x: options.boundaryX,
                y: options.boundaryY
            };
    
            // Automatically handle area movement within the boundary
            setInterval(() => {
                const targetTileX = options.x + Math.floor(Math.random() * (newSprite.boundary.x - options.x + 1));
                const targetTileY = options.y + Math.floor(Math.random() * (newSprite.boundary.y - options.y + 1));
                newSprite.walkToClickedTile(targetTileX, targetTileY);
            }, 30000); // Update position every 5 seconds
        }
    
        // Automatically add the new sprite to the game.sprites object
        game.sprites[options.id] = newSprite;
    
        // Set interval for NPC to say random messages
        if (newSprite.messages.length > 0) {
            setInterval(() => {
                game.randomNpcMessage(newSprite);
            }, Math.random() * 20000 + 20000); // Random interval between 20 and 40 seconds
        }
    
        return newSprite;
    },

    checkTileActions: function() {
        const currentTileId = game.getTileIdAt(Math.floor(this.x / 16), Math.floor(this.y / 16));
        const currentTileX = Math.floor(this.x / 16);
        const currentTileY = Math.floor(this.y / 16);
    
        if (currentTileId !== actions.lastTileId || currentTileX !== actions.lastTileX || currentTileY !== actions.lastTileY) {
            if (actions.lastTileId !== null) {
                actions.handleExitTileAction(actions.lastTileId);
            }
            actions.handleTileAction(currentTileId, currentTileX, currentTileY);
            actions.lastTileId = currentTileId;
            actions.lastTileX = currentTileX;
            actions.lastTileY = currentTileY;
        }
    },

    moveSprite: function (sprite, direction, duration) {
        if (sprite) {
            sprite.addDirection(direction);
            setTimeout(() => {
                sprite.removeDirection(direction);
            }, duration * 1000);
        }
    },

    npcMovement: function (sprite, area) {
        setInterval(() => {
            const targetX = Math.floor(Math.random() * (area.width - sprite.width)) + area.x;
            const targetY = Math.floor(Math.random() * (area.height - sprite.height)) + area.y;
            const tileX = Math.floor(targetX / 16);
            const tileY = Math.floor(targetY / 16);
            
            sprite.walkToClickedTile(tileX, tileY);
        }, 15000); // Update position every 5 seconds
    },

    draw: function() {
        let bodyImage = assets.load('body');
        let headImage = assets.load('head'); // Load the original head image
        let hairImage = assets.load('hair'); // Load the hair image (16x20)
        let outfitImage = assets.load('outfit');
        let facialImage = assets.load('facial'); // Load the facial hair image
        let hatImage = assets.load('hats'); // Load the new hat image
        let glassesImage = assets.load('glasses');
        let eyesImage = assets.load('eyes'); // Load the eyes image
    
        if (!bodyImage || !headImage || !hairImage || !outfitImage || !facialImage || !hatImage || !glassesImage || !eyesImage) return;
    
        let directionOffset = this.directionMap[this.direction] ?? 0;
        let frameColumn = directionOffset + (Math.floor(this.currentFrame) % 6);
        let sx = frameColumn * this.width;
        let sy = 0;
    
        let headDirectionMap = {
            'S': 0,
            'E': 1,
            'N': 2,
            'SE': 3,
            'NE': 4,
            'W': 1,
            'SW': 3,
            'NW': 4
        };
    
        let hairDirectionMap = {
            'S': 0,
            'E': 1,
            'N': 2,
            'SE': 3,
            'NE': 4,
            'W': 1,  // Use the same as 'E'
            'SW': 3, // Use the same as 'SE'
            'NW': 4  // Use the same as 'NE'
        };
    
        let glassesDirectionMap = {
            'S': 0,
            'E': 1,
            'SE': 2,
            'SW': 2, // Use SE but flipped horizontally
            'W': 1   // Use E but flipped horizontally
        };
    
        let eyesDirectionMap = {
            'S': 0,
            'E': 3,
            'SE': 6,
            'SW': 6, // Use SE but without flipping
            'W': 3   // Use E but without flipping
        };
    
        let tempCanvas = document.createElement('canvas');
        let tempCtx = tempCanvas.getContext('2d');
        tempCanvas.width = this.width;
        tempCanvas.height = this.height + 4; // Increase height by 4 pixels for the gap
    
        // Draw body part (16x16) and outfit if body is not 0
        if (this.body !== 0) {
            tempCtx.drawImage(bodyImage, sx, sy, this.width, 16, 0, 13 + 4, this.width, 16); // Adjusted to 13 + 4 to push up by 7 pixels and leave a gap
    
            if (this.outfit !== 0) {
                let outfitSy = (this.outfit - 1) * 16;
                tempCtx.drawImage(outfitImage, sx, outfitSy, this.width, 16, 0, 13 + 4, this.width, 16); // Adjusted to 13 + 4 to push up by 7 pixels and leave a gap
            }
        }
    
        // Draw head part (16x16) if head is not 0
        if (this.head !== 0) {
            let headDirection = headDirectionMap[this.direction];
            let headSx = headDirection * 16;
            let headY = 3 + 4; // Default head y-position with a 4-pixel gap
    
            // Adjust head position based on the frame
            if (this.currentFrame % 6 === 1 || this.currentFrame % 6 === 2 || this.currentFrame % 6 === 5) {
                headY += 1; // Adjust by 1 pixel down
            }
    
            // Draw the head
            tempCtx.drawImage(headImage, headSx, 0, 16, 16, 0, headY, 16, 16);
    
            // Draw eyes if present
            let eyesDirection = eyesDirectionMap[this.direction];
            let eyesSx = eyesDirection * 16 + this.eyeFrame * 16;
            let eyesSy = (this.eyes - 1) * 16; // Adjust for the row based on the eyes index
            tempCtx.drawImage(eyesImage, eyesSx, eyesSy, 16, 16, 0, headY, 16, 16);
    
            // Draw hair if present
            if (this.hair !== 0) {
                let hairDirection = hairDirectionMap[this.direction];
                let hairSx = hairDirection * 16;
                let hairSy = (this.hair - 1) * 20; // Adjust for the new height of 20
                tempCtx.drawImage(hairImage, hairSx, hairSy, 16, 20, 0, headY - 5, 16, 20); // Align hair with the head and account for the extra height
            }
    
            // Draw facial hair if present
            if (this.facial !== 0) {
                let facialSy = (this.facial - 1) * 16;
                tempCtx.drawImage(facialImage, headSx, facialSy, 16, 16, 0, headY, 16, 16); // Ensure facial hair is drawn correctly
            }
    
            // Draw glasses if present
            if (this.glasses !== 0 && glassesDirectionMap[this.direction] !== undefined) {
                let glassesDirection = glassesDirectionMap[this.direction];
                let glassesSx = glassesDirection * 16; // Use the same 16px width
                let glassesSy = (this.glasses - 1) * 8; // Adjust for the glasses height
    
                // Handle flipping for the SW and W directions
                if (this.direction === 'SW' || this.direction === 'W') {
                    tempCtx.save(); // Save the current context state
                    tempCtx.scale(-1, 1); // Flip horizontally
                    tempCtx.drawImage(glassesImage, glassesSx, glassesSy, 16, 8, -16, headY + 7, 16, 8); // Move glasses down by 7 pixels and flip
                    tempCtx.restore(); // Restore the context state
                } else {
                    tempCtx.drawImage(glassesImage, glassesSx, glassesSy, 16, 8, 0, headY + 7, 16, 8); // Move glasses down by 7 pixels
                }
            }
    
            // Draw hat if present
            if (this.hat !== 0) {
                let hatDirection = headDirectionMap[this.direction];
                let hatSx = hatDirection * 16;
                let hatSy = (this.hat - 1) * 16; // Adjust for the row based on the hat index
                let hatY = headY - 4; // Adjusted to go up by 2 pixels
    
                tempCtx.drawImage(hatImage, hatSx, hatSy, 16, 16, 0, hatY, 16, 16); // Align hat with the head
            }
        }
    
        game.ctx.save();
        game.ctx.translate(this.x, this.y); // No need to translate down
    
        if (this.direction === 'W' || this.direction === 'NW' || this.direction === 'SW') {
            game.ctx.scale(-this.scale, this.scale);
            game.ctx.translate(-this.width * this.scale, 0);
        } else {
            game.ctx.scale(this.scale, this.scale);
        }
    
        if (this.overlapping) {
            game.ctx.globalAlpha = 0.5; // Change opacity to 50% if overlapping
        } else {
            game.ctx.globalAlpha = 1; // Reset opacity to 100% if not overlapping
        }
    
        game.ctx.drawImage(tempCanvas, 0, 0, this.width, this.height + 4, 0, 0, this.width * this.scale, (this.height + 4) * this.scale);
    
        game.ctx.globalAlpha = 1; // Reset global alpha to default
    
        if (this.isEnemy) {
            game.ctx.fillStyle = 'red';
            game.ctx.fillRect(0, -10, this.width * this.scale, 5);
            game.ctx.fillStyle = 'green';
            game.ctx.fillRect(0, -10, this.width * this.scale * (this.health / this.maxHealth), 5);
        }
    
        game.ctx.restore();
    },
    
    drawShadow: function() {
        game.ctx.save();
        game.ctx.translate(this.x, this.y + (this.height * this.scale / 2) - 14);
    
        let shadowX, shadowY;
        const shadowWidth = this.width * this.scale * 0.6;
        const shadowHeight = this.height * this.scale * 0.2;
    
        switch (this.direction) {
            case 'N':
                shadowX = (this.width / 2) * this.scale; // Center shadow
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'S':
                shadowX = (this.width / 2) * this.scale; // Center shadow
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'E':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the right
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'W':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the left
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'NE':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the left
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'NW':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the left
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'SE':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the left
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            case 'SW':
                shadowX = (this.width / 2) * this.scale; // Move shadow to the left
                shadowY = (this.height - 1) * this.scale; // Move shadow down
                break;
            default:
                shadowX = (this.width / 2) * this.scale; // Default to center
                shadowY = (this.height) * this.scale; // Move shadow down
                break;
        }
    
        game.ctx.shadowBlur = 15;
        game.ctx.fillStyle = 'rgba(0, 0, 0, 0.05)';
        game.ctx.beginPath();
        game.ctx.ellipse(shadowX, shadowY, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
        game.ctx.fill();
    
        game.ctx.restore();
    },

 walkToClickedTile: function(tileX, tileY) {
    if (editor.isPlacingItem) {
        return; // Don't move if in item placement mode
    }

    // Ensure the target tile is within the boundary if boundary is set
    const boundary = this.boundary;
    if (boundary && (tileX > boundary.x || tileY > boundary.y)) {
        return;
    }

    var currentX = Math.floor(this.x / 16);
    var currentY = Math.floor(this.y / 16);
    this.path = this.calculatePath(currentX, currentY, tileX, tileY);
    this.pathIndex = 0;
    this.isMovingToTarget = true;
    audio.playAudio("walkGrass", assets.load('walkGrass'), 'sfx', true);
},

    calculatePath: function(startX, startY, endX, endY) {
        const grid = game.createWalkableGrid();
        const graph = new Graph(grid, { diagonal: true });
        const start = graph.grid[startX][startY];
        const end = graph.grid[endX][endY];
        
        // Check if start and end points are walkable
        if (grid[startX][startY] === 0) {
            return [];
        }
        if (grid[endX][endY] === 0) {
            return [];
        }
    
        const result = astar.search(graph, start, end);
    
        if (result.length === 0) {
            return [];
        }
    
        const path = result.map(function(node) {
            return { x: node.x, y: node.y, alpha: 1 }; // Add alpha property for opacity
        });
    
        this.path = path;
        this.pathIndex = 0; // Reset the path index for following the path
        return path;
    },    

   moveAlongPath: function() {
        if (!this.path || this.pathIndex >= this.path.length) {
            this.isMovingToTarget = false;
            this.moving = false;
            this.stopping = true;
            this.currentFrame = 0; // Reset to default standing position
            this.path = []; // Clear the path once the destination is reached
            audio.stopLoopingAudio('walkGrass','sfx', 0.5);
            return;
        }
    
        const nextStep = this.path[this.pathIndex];
        const targetX = nextStep.x * 16;
        const targetY = nextStep.y * 16;
        const deltaX = targetX - this.x;
        const deltaY = targetY - this.y;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
        if (distance < this.speed * (game.deltaTime / 1000)) {
            this.x = targetX;
            this.y = targetY;
            this.pathIndex++;
            
            // Remove the step behind the sprite
            if (this.pathIndex > 1) {
                this.path.shift();
                this.pathIndex--;
            }
        } else {
            const angle = Math.atan2(deltaY, deltaX);
            this.x += Math.cos(angle) * this.speed * (game.deltaTime / 1000);
            this.y += Math.sin(angle) * this.speed * (game.deltaTime / 1000);
    
            // Determine direction
            if (Math.abs(deltaX) > Math.abs(deltaY)) {
                if (deltaX > 0) this.direction = 'E';
                else this.direction = 'W';
            } else {
                if (deltaY > 0) this.direction = 'S';
                else this.direction = 'N';
            }
    
            // Adjust direction for diagonal movement
            if (Math.abs(deltaX) > 0 && Math.abs(deltaY) > 0) {
                if (deltaX > 0 && deltaY > 0) this.direction = 'SE';
                else if (deltaX > 0 && deltaY < 0) this.direction = 'NE';
                else if (deltaX < 0 && deltaY > 0) this.direction = 'SW';
                else if (deltaX < 0 && deltaY < 0) this.direction = 'NW';
            }
    
            this.moving = true; // Ensure moving flag is set when moving along the path
            this.stopping = false;
        }
    },

    addDirection: function(direction) {
        this.directions[direction] = true;
        this.updateDirection();
        this.moving = true;
        this.stopping = false;
    
        if (this.id === game.playerid) {
            audio.playAudio('walkGrass', assets.load('walkGrass'), 'sfx', true);
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
    
        const healthBar = document.getElementById('ui_health');
        if (healthBar) {
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
        if (this.moving) {
            this.frameCounter += this.animationSpeed;
            if (this.stopping) {
                if (this.currentFrame < 3 || this.currentFrame > 5) {
                    this.currentFrame = 3;
                } else if (this.frameCounter >= 1) {
                    this.currentFrame = Math.min(this.currentFrame + 1, 5);
                    this.frameCounter = 0;
                }
            } else if (this.currentFrame < 0 || this.currentFrame >= 6) {
                this.currentFrame = 0;
            } else if (this.frameCounter >= 1) {
                if (this.currentFrame < 5) {
                    this.currentFrame++;
                } else {
                    this.currentFrame = 0;
                }
                this.frameCounter = 0;
            }
        } else if (this.stopping && this.frameCounter >= 1) {
            if (this.currentFrame < 5) {
                this.currentFrame++;
            } else {
                this.stopping = false;
                this.currentFrame = 0; // Reset to default standing position
            }
            this.frameCounter = 0;
        }
    },

    animateEyes: function() {
        if (!this.lastBlinkTime) {
            this.lastBlinkTime = Date.now();
        }

        const currentTime = Date.now();
        const timeSinceLastBlink = currentTime - this.lastBlinkTime;

        if (timeSinceLastBlink >= this.eyeBlinkInterval && !this.eyeBlinking) {
            this.eyeBlinking = true;
            this.lastBlinkTime = currentTime;

            // Blink sequence
            setTimeout(() => {
                this.eyeFrame = 1;
                setTimeout(() => {
                    this.eyeFrame = 2;
                    setTimeout(() => {
                        this.eyeFrame = 0;
                        this.eyeBlinking = false;
                    }, 80); // Time between the second and third frame of the blink
                }, 80); // Time between the first and second frame of the blink
            }, 0); // Start blink immediately
        }
    },     

    update: function(deltaTime) {
        const margin = 2; // Define a margin to keep the sprite away from the edges
    
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
    
            let newX = this.x + this.vx;
            let newY = this.y + this.vy;
    
            newX = isNaN(newX) ? this.x : newX;
            newY = isNaN(newY) ? this.y : newY;
    
            let moveX = true;
            let moveY = true;
    
            if (collision.check(newX, this.y, this)) {
                moveX = false;
            }
    
            if (collision.check(this.x, newY, this)) {
                moveY = false;
            }
    
            if (dx !== 0 && dy !== 0) {
                if (moveX && moveY) {
                    this.x = newX;
                    this.y = newY;
                } else if (moveX) {
                    this.x = newX;
                    this.direction = (dx > 0) ? 'E' : 'W';
                } else if (moveY) {
                    this.y = newY;
                    this.direction = (dy > 0) ? 'S' : 'N';
                } else {
                    if (!collision.check(this.x + dx, this.y, this)) {
                        this.x += dx;
                        this.direction = (dx > 0) ? 'E' : 'W';
                    } else if (!collision.check(this.x, this.y + dy, this)) {
                        this.y += dy;
                        this.direction = (dy > 0) ? 'S' : 'N';
                    }
                }
            } else {
                if (moveX) this.x = newX;
                if (moveY) this.y = newY;
            }
    
            // Prevent the sprite from moving too close to the canvas edges
            this.x = Math.max(margin, Math.min(this.x, game.worldWidth - this.width * this.scale - margin));
            this.y = Math.max(margin, Math.min(this.y, game.worldHeight - this.height * this.scale - margin));
    
            // Ensure moving flag is set when directions are present
            if (dx !== 0 || dy !== 0) {
                this.moving = true;
                this.stopping = false;
            } else {
                this.moving = false;
                this.stopping = true;
                this.currentFrame = 0; // Reset to default standing position
            }
        }
    
        this.animate();
        this.animateEyes();
    },

    takeDamage: function(damage) {
        let actualDamage = damage - this.defense;
        actualDamage = Math.max(0, actualDamage);
        this.health -= actualDamage;
        if (this.health <= 0) {
            this.health = 0;
            this.die();
        }
    },

    die: function() {
        // Implement the enemy's death logic here
        // For example, remove the sprite from the game
        delete game.sprites[this.id];
    },

    attackTarget: function(target) {
        if (this.isEnemy && target) {
            target.takeDamage(this.attack);
        }
    },

    chasePlayer: function() {
        const player = game.sprites[id];
        if (!player) return;

        const deltaX = player.x - this.x;
        const deltaY = player.y - this.y;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

        this.removeDirection('up');
        this.removeDirection('down');
        this.removeDirection('left');
        this.removeDirection('right');

        if (distance < 200) { // Chase if within 200 pixels
            if (deltaX > 0) {
                this.addDirection('right');
            } else {
                this.addDirection('left');
            }
            if (deltaY > 0) {
                this.addDirection('down');
            } else {
                this.addDirection('up');
            }

            // Attack if within a certain range
            if (distance < 30) {
                this.attackTarget(player);
            }
        }
    },

    handleAimAttack: function() {
        if (!game.mainSprite || !game.mainSprite.targetAim) return;
    
        const handX = game.mainSprite.x + game.mainSprite.width / 2 + game.mainSprite.handOffsetX;
        const handY = game.mainSprite.y + game.mainSprite.height / 2 + game.mainSprite.handOffsetY;
        const deltaX = game.mainSprite.targetX - handX;
        const deltaY = game.mainSprite.targetY - handY;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
        if (distance <= game.mainSprite.maxRange) {
            const targetRadius = game.mainSprite.targetRadius;
            const aimDistance = Math.sqrt(
                (this.x + this.width / 2 - game.mainSprite.targetX) ** 2 +
                (this.y + this.height / 2 - game.mainSprite.targetY) ** 2
            );
    
            if (aimDistance <= targetRadius) {
                const headDistance = Math.sqrt(
                    (this.x + this.width / 2 - game.mainSprite.targetX) ** 2 +
                    (this.y - game.mainSprite.targetY) ** 2
                );
                let damage = game.mainSprite.attack * (1 - (headDistance / targetRadius));
                damage = Math.max(0, damage);
                this.takeDamage(damage);
            }
        }
    }
};
