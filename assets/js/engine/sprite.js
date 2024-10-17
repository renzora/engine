var sprite = {
    margin: 0,
    create: function (options) {
        let newSprite = {
            id: options.id,
            width: 16,
            height: 32,
            scale: 1,
            speed: options.speed !== undefined ? options.speed : 70,
            currentFrame: 0,
            direction: 'S',
            animationSpeed: 0.2,
            frameCounter: 0,
            moving: false,
            stopping: false,
            directions: {},
            path: [], // Initialize path here
            pathIndex: 0, // Initialize path index here
            isMovingToTarget: false,
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
            handOffsetX: -5,
            handOffsetY: 5,
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
            verticalOffset: 0,
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
            eyeBlinkInterval: Math.random() * 3000 + 2000,
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
            stopPathfinding: this.stopPathfinding,
            update: this.update,
            drawEnemyAttackAimTool: this.drawEnemyAttackAimTool,
            dealDamage: this.dealDamage,
            drawSelectedItem: this.drawSelectedItem,
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
            }, 5000); // Update position every 5 seconds
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
    
        // Set interval for NPC to say random messages
        if (newSprite.messages.length > 0) {
            setInterval(() => {
                game.randomNpcMessage(newSprite);
            }, Math.random() * 20000 + 20000); // Random interval between 20 and 40 seconds
        }
    
        return newSprite;
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
        }, 5000); // Update position every 5 seconds
    },

    draw: function() {
        if (!game.displaySprite) return;
        // Load images
        let headImage = assets.load('head'); 
        let hairImage = assets.load('hair'); 
        let outfitImage = assets.load('outfit');
        let facialImage = assets.load('facial'); 
        let hatImage = assets.load('hats'); 
        let glassesImage = assets.load('glasses');
        let eyesImage = assets.load('eyes'); 
        let handsImage = assets.load('hands'); 
        
        if (!headImage || !hairImage || !outfitImage || !facialImage || !hatImage || !glassesImage || !eyesImage || !handsImage) return;
        
        // Draw the item behind the sprite for specific directions
        if (['SE', 'E', 'NE', 'N'].includes(this.direction)) {
            this.drawSelectedItem(); // Draw behind the sprite
        }
    
        let directionMap = {
            'S': 0,
            'E': 8,
            'N': 16,
            'W': 8,
            'SE': 24,
            'SW': 24,
            'NE': 32,
            'NW': 32
        };
        
        let frameColumn = (Math.floor(this.currentFrame) % 8);
        let sx = (directionMap[this.direction] + frameColumn) * this.width;
        let sy = 0; // All frames are in the same row
        
        let tempCanvas = document.createElement('canvas');
        let tempCtx = tempCanvas.getContext('2d');
        tempCanvas.width = this.width;
        tempCanvas.height = this.height; 
        
        // Draw body part (16x16) and outfit if body is not 0
        if (this.body !== 0) {
            if (this.outfit !== 0) {
                tempCtx.drawImage(outfitImage, sx, sy, this.width, 16, 0, 16, this.width, 16); // Draw outfit
            }
        }
        
        // Draw head part (16x16) if head is not 0
        if (this.head !== 0) {
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
            let headSx = headDirectionMap[this.direction] * 16;
            let headY = 3; // Default head y-position with a 4-pixel gap
        
            // Adjust head position based on the frame
            if (this.currentFrame % 4 === 1 || this.currentFrame % 4 === 2) {
                headY += 2; // Adjust by 1 pixel down
            }
        
            // Draw the head
            tempCtx.drawImage(headImage, headSx, 0, 16, 16, 0, headY, 16, 16);
        
            // Draw eyes if present
            let eyesDirectionMap = {
                'S': 0,
                'E': 3,
                'SE': 6,
                'SW': 6, 
                'W': 3,
                'NE': 9,
                'NW': 9,
                'N': 12
            };
            let eyesSx = eyesDirectionMap[this.direction] * 16 + this.eyeFrame * 16;
            let eyesSy = (this.eyes - 1) * 16; // Adjust for the row based on the eyes index
            tempCtx.drawImage(eyesImage, eyesSx, eyesSy, 16, 16, 0, headY, 16, 16);
        
            // Draw hair if present
            if (this.hair !== 0) {
                let hairDirectionMap = {
                    'S': 0,
                    'E': 1,
                    'N': 2,
                    'SE': 3,
                    'NE': 4,
                    'W': 1,  
                    'SW': 3, 
                    'NW': 4  
                };
                let hairSx = hairDirectionMap[this.direction] * 16;
                let hairSy = (this.hair - 1) * 20; // Adjust for the new height of 20
                tempCtx.drawImage(hairImage, hairSx, hairSy, 16, 20, 0, headY, 16, 20); // Align hair with the head and account for the extra height
            }
        
            // Draw facial hair if present
            if (this.facial !== 0) {
                let facialSy = (this.facial - 1) * 16;
                tempCtx.drawImage(facialImage, headSx, facialSy, 16, 16, 0, headY, 16, 16); // Ensure facial hair is drawn correctly
            }
        
            // Draw glasses if present
            if (this.glasses !== 0) {
                let glassesDirectionMap = {
                    'S': 0,
                    'E': 1,
                    'SE': 2,
                    'SW': 2, 
                    'W': 1,
                    'NE': 3,
                    'NW': 3,
                    'N': 4
                };
                let glassesSx = glassesDirectionMap[this.direction] * 16;
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
                let hatDirectionMap = {
                    'S': 0,
                    'E': 1,
                    'N': 2,
                    'SE': 3,
                    'NE': 4,
                    'W': 1,
                    'SW': 3,
                    'NW': 4
                };
                let hatSx = hatDirectionMap[this.direction] * 16;
                let hatSy = (this.hat - 1) * 16; // Adjust for the row based on the hat index
                let hatY = headY - 6; // Adjusted to go up by 2 pixels
        
                tempCtx.drawImage(hatImage, hatSx, hatSy, 16, 16, 0, hatY, 16, 16); // Align hat with the head
            }
        }
        
        // Draw hands (16x16) using the direction map and cycling through the correct frames
        if (this.hands !== 0) {
            let handsDirectionMap = {
                'S': 0,
                'E': 8,
                'N': 16,
                'SE': 24,
                'NE': 32,
                'W': 8,  // Use 'E' flipped
                'SW': 24, // Use 'SE' flipped
                'NW': 32  // Use 'NE' flipped
            };
    
            let handsFrameOffsetMap = {
                'S': 0,
                'E': 8,
                'N': 16,
                'SE': 24,
                'NE': 32,
                'W': 8,  // Use 'E' flipped
                'SW': 24, // Use 'SE' flipped
                'NW': 32  // Use 'NE' flipped
            };
    
            let handsSx = (handsDirectionMap[this.direction] + this.currentFrame) * 16;
            let handsSy = (this.hands - 1) * 16; // Adjust for the row based on the hands index
            tempCtx.drawImage(handsImage, handsSx, handsSy, 16, 16, 0, 14, 16, 16); // Adjusted position for hands
        }
        
        game.ctx.save();
        
        // Apply vertical offset based on the frame for spring effect
        let springOffset = 0;
        if (this.moving) {
            springOffset = Math.abs(Math.sin((this.currentFrame % 8) * (Math.PI / 4))) * 2; // Adjust the multiplier for more/less spring
        }
        
        game.ctx.translate(this.x, this.y - this.verticalOffset - springOffset); // Apply vertical offset with spring effect
        
        if (this.direction === 'W' || this.direction === 'NW' || this.direction === 'SW') {
            game.ctx.scale(-this.scale, this.scale);
            game.ctx.translate(-this.width * this.scale, 0);
        } else {
            game.ctx.scale(this.scale, this.scale);
        }
        
        if (this.overlapping) {
            game.ctx.globalAlpha = 0.5; 
        } else {
            game.ctx.globalAlpha = 1;
        }
        
        game.ctx.drawImage(tempCanvas, 0, 0, this.width, this.height + 4, 0, 0, this.width * this.scale, (this.height + 4) * this.scale);
        
        game.ctx.globalAlpha = 1; 
        
        if (this.isEnemy) {
            game.ctx.fillStyle = 'red';
            game.ctx.fillRect(0, -10, this.width * this.scale, 5);
            game.ctx.fillStyle = 'green';
            game.ctx.fillRect(0, -10, this.width * this.scale * (this.health / this.maxHealth), 5);
        }
        
        game.ctx.restore();
        
        // Draw the item after the sprite for other directions
        if (['S', 'NW', 'W', 'SW'].includes(this.direction)) {
            this.drawSelectedItem(); // Draw in front of the sprite
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
        audio.playAudio("walkGrass", assets.load('walkGrass'), 'sfx', true);
    },  

    calculatePath: function(startX, startY, endX, endY) {
        const grid = collision.createWalkableGrid(); // Use the cached walkable grid
        const graph = new Graph(grid, { diagonal: true });
        const start = graph.grid[startX][startY];
        const end = graph.grid[endX][endY];
    
        // Check if start and end points are walkable
        if (grid[startX][startY] === 0 || grid[endX][endY] === 0) {
            console.log("Pathfinding: Start or end point is not walkable");
            return [];
        }
    
        const result = astar.search(graph, start, end);
    
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
            audio.stopLoopingAudio('walkGrass', 'sfx', 0.5);
            return;
        }
    
        const nextStep = this.path[this.pathIndex];
        const targetX = nextStep.x * 16;
        const targetY = nextStep.y * 16;
        const deltaX = targetX - this.x;
        const deltaY = targetY - this.y;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
        // Determine direction and ensure sprite is moving
        this.moving = true;
        this.stopping = false;
    
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
        audio.stopLoopingAudio('walkGrass', 'sfx', 0.5);
    
        // You can add any additional clean-up here if needed
        console.log(`Pathfinding and movement stopped for sprite: ${this.id}`);
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
                    const itemData = assets.load('objectData')[roomItem.id];
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

    animate: function() {
        if (this.moving) {
            this.frameCounter += this.animationSpeed;
            if (this.stopping) {
                if (this.currentFrame < 4 || this.currentFrame > 7) {
                    this.currentFrame = 4;
                } else if (this.frameCounter >= 1) {
                    this.currentFrame = Math.min(this.currentFrame + 1, 7);
                    this.frameCounter = 0;
                }
            } else if (this.currentFrame < 0 || this.currentFrame >= 8) {
                this.currentFrame = 0;
            } else if (this.frameCounter >= 1) {
                if (this.currentFrame < 7) {
                    this.currentFrame++;
                } else {
                    this.currentFrame = 0;
                }
                this.frameCounter = 0;
            }
        } else if (this.stopping && this.frameCounter >= 1) {
            if (this.currentFrame < 7) {
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
    
            // Randomize the blink interval after each blink, between 2 to 5 seconds
            this.eyeBlinkInterval = Math.random() * 3000 + 2000;
    
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
        const margin = 4; // Define a margin to keep the sprite away from the edges
    
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
    
            const collisionResultX = collision.check(newX, this.y, this);
            moveX = !collisionResultX.collisionDetected;
            const collisionResultY = collision.check(this.x, newY, this);
            moveY = !collisionResultY.collisionDetected;
    
            if (moveX && moveY) {
                this.x = newX;
                this.y = newY;
            } else if (moveX) {
                this.x = newX;
            } else if (moveY) {
                this.y = newY;
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
    
        this.animate();  // Make sure to animate the sprite in each update cycle
        this.animateEyes();
    
        // Update light source position
        if (this.id === game.playerid) {
            const playerLight = lighting.lights.find(light => light.id === this.id + '_light');
            if (playerLight) {
                //console.log(`Updating player light position to: (${this.x + 8}, ${this.y + 8})`);
                playerLight.x = this.x + 8; // Center light on sprite
                playerLight.y = this.y + 8; // Center light on sprite
            } else {
                console.log(`Player light not found for ID: ${this.id + '_light'}`);
            }
        }
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
};