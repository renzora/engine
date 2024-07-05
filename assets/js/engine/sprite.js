var sprite = {
    create: function (options) {
        let newSprite = {
            id: options.id,
            x: options.x !== undefined ? options.x : 300,
            y: options.y !== undefined ? options.y : 150,
            width: 16,
            height: 27,
            scale: 0.95,
            speed: options.speed !== undefined ? options.speed : 90,
            currentFrame: 0,
            direction: 'S',
            animationSpeed: 0.2,
            frameCounter: 0,
            moving: false,
            stopping: false,
            directions: {},
            body: options.body !== undefined ? options.body : 1,
            head: options.head !== undefined ? options.head : 1,
            hairstyle: options.hairstyle || 1,
            outfit: options.outfit || 1,
            facialHair: options.facialHair || 0,
            hat: options.hat || 0,
            glasses: options.glasses || 0,
            isEnemy: options.isEnemy || false,
            attack: options.attack || 10,
            defense: options.defense || 5,
            intensity: options.intensity || 1,
            targetAim: false,
            targetX: 0,
            targetY: 0,
            targetRadius: 10,
            maxRange: 400,
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
            runningSpeed: 120,
            messages: options.messages || [],

            directionMap: {
                'S': 0,
                'E': 6,
                'N': 12,
                'W': 6
            },

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

        // Automatically handle area movement if area is specified in options
        if (options.area) {
            setInterval(() => {
                const directions = ['up', 'down', 'left', 'right'];
                const randomDirection = directions[Math.floor(Math.random() * directions.length)];
                const randomDuration = Math.random() * 2 + 1;

                let newX = newSprite.x;
                let newY = newSprite.y;

                switch (randomDirection) {
                    case 'up':
                        newY -= newSprite.speed * randomDuration;
                        break;
                    case 'down':
                        newY += newSprite.speed * randomDuration;
                        break;
                    case 'left':
                        newX -= newSprite.speed * randomDuration;
                        break;
                    case 'right':
                        newX += newSprite.speed * randomDuration;
                        break;
                }

                if (newX >= options.area.x && newX <= options.area.x + options.area.width - newSprite.width &&
                    newY >= options.area.y && newY <= options.area.y + options.area.height - newSprite.height) {
                    newSprite.addDirection(randomDirection);
                    setTimeout(() => {
                        newSprite.removeDirection(randomDirection);
                    }, randomDuration * 1000);
                }
            }, 500);
        }



        // Automatically add the new sprite to the game.sprites object
        game.sprites[options.id] = newSprite;

        // Set interval for NPC to say random messages
        if (newSprite.messages.length > 0) {
            setInterval(() => {
                game.randomNpcMessage(newSprite);
            }, Math.random() * 20000 + 20000); // Random interval between 5 and 15 seconds
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
            const directions = ['up', 'down', 'left', 'right'];
            const randomDirection = directions[Math.floor(Math.random() * directions.length)];
            const randomDuration = Math.random() * 2 + 1;

            let newX = sprite.x;
            let newY = sprite.y;

            switch (randomDirection) {
                case 'up':
                    newY -= sprite.speed * randomDuration;
                    break;
                case 'down':
                    newY += sprite.speed * randomDuration;
                    break;
                case 'left':
                    newX -= sprite.speed * randomDuration;
                    break;
                case 'right':
                    newX += sprite.speed * randomDuration;
                    break;
            }

            if (newX >= area.x && newX <= area.x + area.width - sprite.width &&
                newY >= area.y && newY <= area.y + area.height - sprite.height) {
                sprite.addDirection(randomDirection);
                setTimeout(() => {
                    sprite.removeDirection(randomDirection);
                }, randomDuration * 1000);
            }
        }, 500);
    },

    draw: function() {
        let bodyImage = assets.load('body');
        let headImage = assets.load('head');
        let hairImage = assets.load('hair');
        let outfitImage = assets.load('outfit');
        let facialHairImage = assets.load('facial');
        let hatImage = assets.load('hats');
        let glassesImage = assets.load('glasses');
    
        if (!bodyImage || !headImage || !hairImage || !outfitImage || !facialHairImage || !hatImage || !glassesImage) return;
    
        let directionOffset = this.directionMap[this.direction] ?? 0;
        let frameColumn = directionOffset + (Math.floor(this.currentFrame) % 6);
        let sx = frameColumn * this.width;
        let sy = 0;
    
        let tempCanvas = document.createElement('canvas');
        let tempCtx = tempCanvas.getContext('2d');
        tempCanvas.width = this.width;
        tempCanvas.height = this.height;
    
        // Draw body part (16x16) and outfit if body is not 0
        if (this.body !== 0) {
            tempCtx.drawImage(bodyImage, sx, sy, this.width, 16, 0, 13, this.width, 16); // Adjusted to 13 to push up by 7 pixels
    
            if (this.outfit !== 0) {
                let outfitSy = (this.outfit - 1) * 16;
                tempCtx.drawImage(outfitImage, sx, outfitSy, this.width, 16, 0, 13, this.width, 16); // Adjusted to 13 to push up by 7 pixels
            }
        }
    
        // Draw head part (16x16) and its accessories if head is not 0
        if (this.head !== 0) {
            tempCtx.drawImage(headImage, sx, sy, this.width, 16, 0, 3, this.width, 16);
    
            if (this.hairstyle !== 0) {
                let hairSy = (this.hairstyle - 1) * 16;
                tempCtx.drawImage(hairImage, sx, hairSy, this.width, 16, 0, 3, this.width, 16);
            }
    
            if (this.facialHair !== 0) {
                let facialHairSy = (this.facialHair - 1) * 16;
                tempCtx.drawImage(facialHairImage, sx, facialHairSy, this.width, 16, 0, 3, this.width, 16);
            }
    
            if (this.glasses !== 0) {
                let glassesSy = (this.glasses - 1) * 16;
                tempCtx.drawImage(glassesImage, sx, glassesSy, this.width, 16, 0, 3, this.width, 16);
            }
    
            if (this.hat !== 0) {
                let hatSy = (this.hat - 1) * 16;
                tempCtx.drawImage(hatImage, sx, hatSy, this.width, 16, 0, 1, this.width, 16); // Adjusted to 1 to prevent clipping
            }
        }
    
        game.ctx.save();
        game.ctx.translate(this.x, this.y); // No need to translate down
    
        if (this.direction === 'W') {
            game.ctx.scale(-this.scale, this.scale);
            game.ctx.translate(-this.width * this.scale, 0);
        } else {
            game.ctx.scale(this.scale, this.scale);
        }
    
        game.ctx.drawImage(tempCanvas, 0, 0, this.width, this.height, 0, 0, this.width * this.scale, this.height * this.scale);
    
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
            console.error(`Start point (${startX}, ${startY}) is not walkable.`);
            return [];
        }
        if (grid[endX][endY] === 0) {
            console.error(`End point (${endX}, ${endY}) is not walkable.`);
            return [];
        }
    
        const result = astar.search(graph, start, end);
    
        if (result.length === 0) {
            console.error('No valid path found.');
            return [];
        }
    
        const path = result.map(function(node) {
            return { x: node.x, y: node.y, alpha: 1 }; // Add alpha property for opacity
        });
    
        this.path = path;
        this.pathIndex = 0; // Reset the path index for following the path
    
        console.log('Calculated Path:', path);
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
                if (deltaX > 0) this.direction = 'E';
                else this.direction = 'W';
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

        audio.playAudio('walkGrass', assets.load('walkGrass'), 'sfx', true);
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
        if (this.directions['up']) this.direction = 'N';
        if (this.directions['down']) this.direction = 'S';
        if (this.directions['left']) this.direction = 'W';
        if (this.directions['right']) this.direction = 'E';
        if (this.directions['up'] && this.directions['right']) this.direction = 'E';
        if (this.directions['down'] && this.directions['right']) this.direction = 'E';
        if (this.directions['down'] && this.directions['left']) this.direction = 'W';
        if (this.directions['up'] && this.directions['left']) this.direction = 'W';
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

        const healthBar = document.getElementById('health');
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

        const energyBar = document.getElementById('energy');
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

    update: function(deltaTime) {
        // Check if the shift key is held down
        if (input.isShiftPressed) {
            return; // Skip the update logic if the shift key is pressed
        }
    
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
    
            if (game.collision(newX, this.y, this)) {
                moveX = false;
            }
    
            if (game.collision(this.x, newY, this)) {
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
                    if (!game.collision(this.x + dx, this.y, this)) {
                        this.x += dx;
                        this.direction = (dx > 0) ? 'E' : 'W';
                    } else if (!game.collision(this.x, this.y + dy, this)) {
                        this.y += dy;
                        this.direction = (dy > 0) ? 'S' : 'N';
                    }
                }
            } else {
                if (moveX) this.x = newX;
                if (moveY) this.y = newY;
            }
    
            this.x = Math.max(0, Math.min(this.x, game.worldWidth - this.width * this.scale));
            this.y = Math.max(0, Math.min(this.y, game.worldHeight - this.height * this.scale));
    
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
        const mainSprite = game.sprites[id];
        if (!mainSprite || !mainSprite.targetAim) return;

        const handX = mainSprite.x + mainSprite.width / 2 + mainSprite.handOffsetX;
        const handY = mainSprite.y + mainSprite.height / 2 + mainSprite.handOffsetY;
        const deltaX = mainSprite.targetX - handX;
        const deltaY = mainSprite.targetY - handY;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);

        if (distance <= mainSprite.maxRange) {
            const targetRadius = mainSprite.targetRadius;
            const aimDistance = Math.sqrt(
                (this.x + this.width / 2 - mainSprite.targetX) ** 2 +
                (this.y + this.height / 2 - mainSprite.targetY) ** 2
            );

            if (aimDistance <= targetRadius) {
                const headDistance = Math.sqrt(
                    (this.x + this.width / 2 - mainSprite.targetX) ** 2 +
                    (this.y - mainSprite.targetY) ** 2
                );
                let damage = mainSprite.attack * (1 - (headDistance / targetRadius));
                damage = Math.max(0, damage);
                this.takeDamage(damage);
            }
        }
    }
};