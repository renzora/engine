var game = {
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isEditMode: false,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 640,
    worldHeight: 640,
    zoomLevel: 4,
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    playerid: null,
    viewportXStart: null,
    viewportXEnd: null,
    viewportYStart: null,
    viewportYEnd: null,
    gameTime: {
        hours: 7,
        minutes: 0,
        seconds: 0,
        days: 0,
        speedMultiplier: 100, // Game time progresses 10 times faster than real time
        daysOfWeek: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        update: function(deltaTime) {
            const gameSeconds = (deltaTime / 1000) * this.speedMultiplier;
            this.seconds += gameSeconds;

            if (this.seconds >= 60) {
                this.minutes += Math.floor(this.seconds / 60);
                this.seconds = this.seconds % 60;
            }
            if (this.minutes >= 60) {
                this.hours += Math.floor(this.minutes / 60);
                this.minutes = this.minutes % 60;
            }
            if (this.hours >= 24) {
                this.days += Math.floor(this.hours / 24);
                this.hours = this.hours % 24;
            }
        },
        display: function() {
            const pad = (num) => String(num).padStart(2, '0');
            const dayOfWeek = this.daysOfWeek[this.days % 7];
            return `${dayOfWeek} ${pad(this.hours)}:${pad(this.minutes)}`;
        }
    },

    reloadGameData: function() {
        // Specify the assets you want to reload
        const assetsToReload = ['objectData', 'roomData']; // Add other assets as needed

        assets.reloadAssets(assetsToReload, () => {
            console.log("Game data reloaded");
            this.roomData = assets.load('roomData');
            // Perform any additional updates necessary for your game
            this.updateGameElements();
        });
    },

    updateGameElements: function() {
        // Any specific updates to game elements after reloading data can be added here
        console.log("Game elements updated");
        // Example: this.refreshSprites(); if you need to update sprites specifically
    },

    init: function() {
        this.playerid = network.getToken('renaccount') || `player_${Math.floor(Math.random() * 10000)}`;

        assets.preload([
            { name: 'character', path: 'img/sprites/character.png' },
            { name: 'hair', path: 'img/sprites/hair.png' },
            { name: 'hats', path: 'img/sprites/hats.png' },
            { name: 'glasses', path: 'img/sprites/glasses.png' },
            { name: 'facial', path: 'img/sprites/facial.png' },
            { name: 'outfit', path: 'img/sprites/outfit.png' },
            { name: '1', path: 'img/tiles/1.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'objectScript', path: 'json/objectScript.json' },
            { name: 'roomData', path: 'json/roomData.json' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.roomData = assets.load('roomData');

            // Create player sprite
            const playerOptions = {
                id: this.playerid,
                x: 300,
                y: 500,
                isPlayer: true,
                speed: 90
            };
            sprite.create(playerOptions);
            
            for (let i = 0; i < 100; i++) {
                const npc = {
                    id: `npc${i}`,
                    x: Math.floor(Math.random() * 500), // Random x coordinate
                    y: Math.floor(Math.random() * 400), // Random y coordinate
                    isPlayer: false,
                    hairstyle: Math.floor(Math.random() * 29), // Assuming there are 5 different hairstyles
                    outfit: Math.floor(Math.random() * 3), // Assuming there are 5 different outfits
                    facialHair: Math.floor(Math.random() * 1), // Assuming there are 3 different facial hair options
                    hat: Math.floor(Math.random() * 2), // Assuming there are 2 different hat options
                    glasses: Math.floor(Math.random() * 2), // Assuming there are 2 different glasses options
                    area: {
                        x: Math.floor(Math.random() * 400), // Random x coordinate for area
                        y: Math.floor(Math.random() * 400), // Random y coordinate for area
                        width: 500,
                        height: 500
                    },
                    messages: [
                        "Hello!",
                        "How are you?",
                        "Nice to meet you!",
                        "Good day!",
                        "What's up?",
                        "Have you seen the weather?",
                        "I love this place!",
                        "Let's go on an adventure!",
                        "Do you like games?",
                        "This town is great!",
                        "I need a vacation.",
                        "Have you heard any news?",
                        "I'm feeling lucky today.",
                        "Do you have any pets?",
                        "I just saw a bird!",
                        "Let's chat for a while.",
                        "I found a treasure!",
                        "Do you like to read?",
                        "I enjoy a good meal.",
                        "The stars are beautiful.",
                        "Do you like music?",
                        "I met a new friend.",
                        "This place is magical.",
                        "I'm learning something new.",
                        "Have you traveled far?",
                        "Let's explore together!",
                        "Do you have any hobbies?",
                        "I love the outdoors.",
                        "What a lovely day!",
                        "Do you enjoy puzzles?",
                        "I'm feeling happy.",
                        "I saw a rainbow!",
                        "Have you seen a ghost?",
                        "I'm practicing magic.",
                        "I enjoy a good story.",
                        "This place is full of wonders.",
                        "Let's go for a walk.",
                        "Do you believe in magic?",
                        "I found a hidden path.",
                        "Do you like to dance?",
                        "I'm looking for adventure.",
                        "The flowers are blooming.",
                        "I saw a shooting star.",
                        "Do you like to cook?",
                        "I'm collecting items.",
                        "Let's have some fun!",
                        "I'm learning to draw.",
                        "Have you seen any animals?",
                        "This place is amazing.",
                        "I love making new friends."
                    ]
                };
                sprite.create(npc);
            }

            weather.starsActive = true;
            weather.fogActive = true;
            weather.rainActive = true;
            weather.snowActive = true;
            weather.nightActive = true;

            weather.createStars();
            weather.createFog(0.05);
            weather.createRain(0.7);
            weather.createSnow(0.2);

            this.loop();

            const mainSprite = game.sprites[this.playerid];
            if (mainSprite) {
                mainSprite.updateHealth(0);  // Ensure update occurs
                mainSprite.updateHealth(mainSprite.health);  
                mainSprite.updateEnergy(mainSprite.energy);  
            }

            // Send initial player state to the server
            network.send({
                command: 'playerStateUpdate',
                data: {
                    id: this.playerid,
                    ...this.sprites[this.playerid]
                }
            });

            // Add this line to allow triggering reload from the console or UI
            window.reloadGameData = this.reloadGameData.bind(this);
        });
    },

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    },

    updateCamera: function() {
        if(!game.isEditMode) {

        var scaledWindowWidth = window.innerWidth / game.zoomLevel;
        var scaledWindowHeight = window.innerHeight / game.zoomLevel;

        if (game.worldWidth < scaledWindowWidth || game.worldHeight < scaledWindowHeight) {
            var xOffset = game.worldWidth < scaledWindowWidth ? (scaledWindowWidth - game.worldWidth) / 2 : 0;
            var yOffset = game.worldHeight < scaledWindowHeight ? (scaledWindowHeight - game.worldHeight) / 2 : 0;

            this.targetCameraX = -xOffset;
            this.targetCameraY = -yOffset;
        } else {
            let mainSprite = game.sprites[this.playerid];
            if (mainSprite) {
                this.targetCameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;

                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));
            } else {
                console.error('Main sprite not found.');
            }
        }

        this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
        this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);

        if (typeof debug_window !== 'undefined' && debug_window.camera) {
            debug_window.camera();
        }
        }
    },

    lerp: function(start, end, t) {
        return start * (1 - t) + end * t;
    },

    handleAimAttack: function () {
        for (let id in this.sprites) {
            if (this.sprites[id].isEnemy) {
                this.sprites[id].handleAimAttack();
            }
        }
    },

    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(this.cameraX), -Math.round(this.cameraY));
    
        const renderQueue = [];
    
        // Calculate the boundaries of the viewport in world coordinates
        this.viewportXStart = Math.max(0, Math.floor(this.cameraX / 16));
        this.viewportXEnd = Math.min(this.worldWidth / 16, Math.ceil((this.cameraX + window.innerWidth / this.zoomLevel) / 16));
        this.viewportYStart = Math.max(0, Math.floor(this.cameraY / 16));
        this.viewportYEnd = Math.min(this.worldHeight / 16, Math.ceil((this.cameraY + window.innerHeight / this.zoomLevel) / 16));
    
        let tileCount = 0;
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                const itemData = assets.load('objectData')[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0];
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    let index = 0;
    
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
                            // Only add tiles within the viewport to the render queue
                            if (x >= this.viewportXStart && x < this.viewportXEnd && y >= this.viewportYStart && y < this.viewportYEnd) {
                                const posX = x * 16;
                                const posY = y * 16;
    
                                let tileFrameIndex;
                                if (tileData.d) {
                                    const currentFrame = tileData.currentFrame || 0;
                                    tileFrameIndex = Array.isArray(tileData.i) ? tileData.i[(currentFrame + index) % tileData.i.length] : tileData.i;
                                } else {
                                    tileFrameIndex = tileData.i[index];
                                }
    
                                const srcX = (tileFrameIndex % 150) * 16;
                                const srcY = Math.floor(tileFrameIndex / 150) * 16;
    
                                renderQueue.push({
                                    tileIndex: tileFrameIndex,
                                    posX: posX,
                                    posY: posY,
                                    z: Array.isArray(tileData.z) ? tileData.z[index % tileData.z.length] : tileData.z,
                                    draw: function() {
                                        game.ctx.drawImage(assets.load(tileData.t), srcX, srcY, 16, 16, this.posX, this.posY, 16, 16);
                                    }
                                });
    
                                tileCount++;
                            }
    
                            index++;
                        }
                    }
                }
            });
        }
        let spriteCount = 0;
        for (let id in this.sprites) {
            const sprite = this.sprites[id];
            const spriteRight = sprite.x + sprite.width;
            const spriteBottom = sprite.y + sprite.height;
            
            // Check if sprite is within the viewport
            if (spriteRight >= this.viewportXStart * 16 && sprite.x < this.viewportXEnd * 16 &&
                spriteBottom >= this.viewportYStart * 16 && sprite.y < this.viewportYEnd * 16) {
                renderQueue.push({
                    z: 1,
                    draw: function() {
                        game.sprites[id].draw();
                    }
                });
                spriteCount++;
            }
        }
    
        renderQueue.sort((a, b) => a.z - b.z);
        renderQueue.forEach(item => item.draw());
        this.ctx.imageSmoothingEnabled = false;
    
        weather.applyNightColorFilter(); // Apply the night color filter
        weather.drawSnow();
        weather.drawRain();
        weather.drawFog();
        weather.drawStars();
        weather.drawLightning();
        this.handleAimAttack();
    
        // Draw target aimer if active
        const mainSprite = this.sprites[this.playerid];
        if (mainSprite && mainSprite.targetAim) {
            const handX = mainSprite.x + mainSprite.width / 2 + mainSprite.handOffsetX;
            const handY = mainSprite.y + mainSprite.height / 2 + mainSprite.handOffsetY;
    
            const deltaX = mainSprite.targetX - handX;
            const deltaY = mainSprite.targetY - handY;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
            // Calculate the adjusted target position
            let adjustedTargetX = mainSprite.targetX;
            let adjustedTargetY = mainSprite.targetY;
            if (distance > mainSprite.maxRange) {
                const ratio = mainSprite.maxRange / distance;
                adjustedTargetX = handX + deltaX * ratio;
                adjustedTargetY = handY + deltaY * ratio;
            }
    
            // Function to check for collision with non-walkable map objects
            const isObstructed = (x, y) => {
                if (this.roomData && this.roomData.items) {
                    for (const roomItem of this.roomData.items) {
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
                const stepX = handX + (deltaX * i) / steps;
                const stepY = handY + (deltaY * i) / steps;
                const result = isObstructed(stepX, stepY);
                if (result.obstructed) {
                    finalTargetX = result.collisionX;
                    finalTargetY = result.collisionY;
                    obstructionDetected = true;
                    break;
                }
            }
    
            // Do not show the aim if the obstruction is very close
            if (obstructionDetected && Math.sqrt((finalTargetX - handX) ** 2 + (finalTargetY - handY) ** 2) < 10) {
                return;
            }
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.setLineDash([5, 5]); // Dotted line
            this.ctx.beginPath();
            this.ctx.moveTo(handX, handY);
            this.ctx.lineTo(finalTargetX, finalTargetY);
            this.ctx.stroke();
            this.ctx.setLineDash([]); // Reset line dash
    
            // Draw target radius at the final target position
            this.ctx.beginPath();
            this.ctx.arc(finalTargetX, finalTargetY, mainSprite.targetRadius, 0, 2 * Math.PI);
            this.ctx.stroke();
        }
    
        if (game.isEditMode && edit_mode_window.isSelecting && edit_mode_window.selectionStart && edit_mode_window.selectionEnd) {
            const startX = Math.min(edit_mode_window.selectionStart.x, edit_mode_window.selectionEnd.x);
            const startY = Math.min(edit_mode_window.selectionStart.y, edit_mode_window.selectionEnd.y);
            const endX = Math.max(edit_mode_window.selectionStart.x, edit_mode_window.selectionEnd.x) + 16;
            const endY = Math.max(edit_mode_window.selectionStart.y, edit_mode_window.selectionEnd.y) + 16;
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.lineWidth = 4 / this.zoomLevel;
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
        }
    
        if (game.isEditMode) {
            edit_mode_window.selectedTiles.forEach(tile => {
                this.ctx.fillStyle = 'rgba(0, 255, 0, 0.2)';
                this.ctx.fillRect(tile.x, tile.y, 16, 16);
            });
        }
    
        if (typeof debug_window !== 'undefined') {
            if (game.showGrid && debug_window.grid) {
                debug_window.grid();
            }
            if (game.showCollision && debug_window.tiles) {
                debug_window.tiles();
            }
            if (game.showTiles && debug_window.tiles) {
                debug_window.tiles();
            }
        }
    
        // Draw ID bubbles last to ensure they appear above all other elements
        for (let id in this.sprites) {
            this.drawIdBubble(this.sprites[id]);
            this.drawChatBubble(this.sprites[id]);
        }
    },

    randomNpcMessage: function(sprite) {
        if (sprite.messages && sprite.messages.length > 0) {
            const randomIndex = Math.floor(Math.random() * sprite.messages.length);
            const message = sprite.messages[randomIndex];
            this.updateChatMessages(sprite, message);
        }
    },
    
    drawIdBubble: function(sprite) {
        if (!sprite || !sprite.id) return;
    
        // Truncate text if it's longer than 16 characters
        let text = sprite.id;
        if (text.length > 16) {
            text = text.slice(0, 13);
        }
    
        const bubbleHeight = 7;
        const bubblePadding = 2;
        const fontSize = 3;
        const characterSpacing = -0.1; // Adjust this value for tighter or looser tracking
        
        // Calculate text width
        this.ctx.font = `${fontSize}px Tahoma`;
        let textWidth = 0;
        for (let char of text) {
            textWidth += this.ctx.measureText(char).width + characterSpacing;
        }
        textWidth -= characterSpacing; // Remove the extra spacing added after the last character
    
        // Calculate bubble dimensions
        const bubbleWidth = textWidth + 2 * bubblePadding;
    
        // Calculate bubble position
        const bubbleX = sprite.x + sprite.width / 2 - bubbleWidth / 2;
        const bubbleY = sprite.y - bubbleHeight - bubblePadding + 5; // Adjust this value to bring the bubble down
    
        // Draw rounded rectangle bubble with less pronounced corners
        const radius = 2; // Adjust the radius for subtler rounded corners
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        this.ctx.beginPath();
        this.ctx.moveTo(bubbleX + radius, bubbleY);
        this.ctx.lineTo(bubbleX + bubbleWidth - radius, bubbleY);
        this.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY, bubbleX + bubbleWidth, bubbleY + radius);
        this.ctx.lineTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight - radius);
        this.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight, bubbleX + bubbleWidth - radius, bubbleY + bubbleHeight);
        this.ctx.lineTo(bubbleX + radius, bubbleY + bubbleHeight);
        this.ctx.quadraticCurveTo(bubbleX, bubbleY + bubbleHeight, bubbleX, bubbleY + bubbleHeight - radius);
        this.ctx.lineTo(bubbleX, bubbleY + radius);
        this.ctx.quadraticCurveTo(bubbleX, bubbleY, bubbleX + radius, bubbleY);
        this.ctx.closePath();
        this.ctx.fill();
    
        // Draw each character with fixed spacing
        this.ctx.fillStyle = 'white';
        this.ctx.font = `${fontSize}px Tahoma`;
        let charX = bubbleX + bubblePadding;
        for (let char of text) {
            this.ctx.fillText(char, charX, bubbleY + bubbleHeight / 2 + fontSize / 3);
            charX += this.ctx.measureText(char).width + characterSpacing;
        }
    },
    
    drawChatBubble: function(sprite) {
        if (!sprite.chatMessages || sprite.chatMessages.length === 0) return;

        // Iterate through each message
        for (let i = 0; i < sprite.chatMessages.length; i++) {
            const messageData = sprite.chatMessages[i];
            const elapsedTime = Date.now() - messageData.time;
            
            if (elapsedTime > 5000) {
                sprite.chatMessages.splice(i, 1);
                i--;
                continue;
            }
            
            const fadeOutTime = 1000; // 1 second fade-out duration
            const alpha = elapsedTime > 4000 ? (1 - (elapsedTime - 4000) / fadeOutTime) : 1; // Start fading out after 4 seconds
        
            const message = messageData.text;
            const bubbleHeight = 7;
            const bubblePadding = 2;
            const fontSize = 3;
            const characterSpacing = -0.1; // Adjust this value for tighter or looser tracking
        
            // Calculate text width
            game.ctx.font = `${fontSize}px Tahoma`;
            let textWidth = 0;
            for (let char of message) {
                textWidth += game.ctx.measureText(char).width + characterSpacing;
            }
            textWidth -= characterSpacing; // Remove the extra spacing added after the last character
        
            // Calculate bubble dimensions
            const bubbleWidth = textWidth + 2 * bubblePadding;
        
            // Calculate bubble position
            const bubbleX = sprite.x + sprite.width / 2 - bubbleWidth / 2;
            const baseBubbleY = sprite.y - 12; // Move the first bubble up by 2-3 pixels
            const bubbleY = baseBubbleY - (i * (bubbleHeight + bubblePadding - 1)); // Reduce vertical spacing between bubbles
    
            // Draw rounded rectangle bubble with blue color
            const radius = 2; // Adjust the radius for subtler rounded corners
            game.ctx.fillStyle = `rgba(0, 0, 255, ${alpha * 0.9})`; // Blue color with fading effect
            game.ctx.beginPath();
            game.ctx.moveTo(bubbleX + radius, bubbleY);
            game.ctx.lineTo(bubbleX + bubbleWidth - radius, bubbleY);
            game.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY, bubbleX + bubbleWidth, bubbleY + radius);
            game.ctx.lineTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight - radius);
            game.ctx.quadraticCurveTo(bubbleX + bubbleWidth, bubbleY + bubbleHeight, bubbleX + bubbleWidth - radius, bubbleY + bubbleHeight);
            game.ctx.lineTo(bubbleX + radius, bubbleY + bubbleHeight);
            game.ctx.quadraticCurveTo(bubbleX, bubbleY + bubbleHeight, bubbleX, bubbleY + bubbleHeight - radius);
            game.ctx.lineTo(bubbleX, bubbleY + radius);
            game.ctx.quadraticCurveTo(bubbleX, bubbleY, bubbleX + radius, bubbleY);
            game.ctx.closePath();
            game.ctx.fill();
        
            // Draw each character with fixed spacing
            game.ctx.fillStyle = `rgba(255, 255, 255, ${alpha})`;
            game.ctx.font = `${fontSize}px Tahoma`;
            let charX = bubbleX + bubblePadding;
            for (let char of message) {
                game.ctx.fillText(char, charX, bubbleY + bubbleHeight / 2 + fontSize / 2);
                charX += game.ctx.measureText(char).width + characterSpacing;
            }
        }
    },

    updateChatMessages: function(sprite, newMessage) {
        if (!sprite.chatMessages) {
            sprite.chatMessages = [];
        }
        // Add new message to the queue
        sprite.chatMessages.push({ text: newMessage, time: Date.now() });
        
        // Ensure only the last 3 messages are kept
        if (sprite.chatMessages.length > 3) {
            sprite.chatMessages.shift();
        }
    },
    
    loop: function(timestamp) {
        if (!this.lastTime) {
            this.lastTime = timestamp;
            return requestAnimationFrame(this.loop.bind(this));
        }
    
        this.deltaTime = timestamp - this.lastTime;
        this.lastTime = timestamp;
        this.fps = 1000 / this.deltaTime;

        var debugFPS = document.getElementById('gameFps');
        debugFPS.innerHTML = "FPS: " + game.fps.toFixed(2);
    
        // Update game time
        this.gameTime.update(this.deltaTime);
    
        // Update HUD with game time
        document.getElementById('game_time').innerText = this.gameTime.display();
    
        for (let id in this.sprites) {
            this.sprites[id].update();
        }
    
        this.updateAnimatedTiles(this.deltaTime);
        weather.updateSnow();
        weather.updateRain();
        weather.updateFog();
        weather.updateStars();
        weather.updateLightning();
        this.updateCamera();
        this.render();
        weather.applyNightColorFilter(); // Apply the night color filter based on time
        requestAnimationFrame(this.loop.bind(this));
    },
    
    updateAnimatedTiles: function(deltaTime) {
        if (!this.roomData || !this.roomData.items) return;

        this.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                itemData.forEach(tileData => {
                    if (tileData.d) {
                        if (!tileData.currentFrame) {
                            tileData.currentFrame = 0;
                        }
                        if (!tileData.elapsedTime) {
                            tileData.elapsedTime = 0;
                        }
                        tileData.elapsedTime += deltaTime;
                        if (tileData.elapsedTime >= tileData.d) {
                            tileData.elapsedTime = 0;
                            tileData.currentFrame = (tileData.currentFrame + 1) % tileData.i.length;
                        }
                    }
                });
            }
        });
    },

    collision: function(x, y, sprite) {
        let collisionDetected = false;
        const extraHeadroom = 2;
    
        // Define the collision box for the sprite
        const spriteCollisionBox = {
            x: x,
            y: y + extraHeadroom,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale - 2 * extraHeadroom
        };
    
        const objectCollisionBox = {
            x: x,
            y: y + sprite.height * sprite.scale / 2,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale / 2
        };
    
        if (this.roomData && this.roomData.items) {
            collisionDetected = this.roomData.items.some(roomItem => {
                const itemData = assets.load('objectData')[roomItem.id];
                if (!itemData) return false;
    
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];
    
                let index = 0;
    
                return yCoordinates.some((yCoord, j) => {
                    return xCoordinates.some((xCoord, i) => {
                        const tileData = itemData[0]; // Assuming we are dealing with the first tile data group
                        const tilePosX = parseInt(xCoord, 10) * 16 + tileData.a[index % tileData.a.length];
                        const tilePosY = parseInt(yCoord, 10) * 16 + tileData.b[index % tileData.b.length];
                        const tileRect = {
                            x: tilePosX,
                            y: tilePosY,
                            width: 16,
                            height: 16
                        };
    
                        let collisionArray;
                        if (Array.isArray(tileData.w) && tileData.w.length > 0) {
                            collisionArray = tileData.w[index % tileData.w.length];
                        } else if (typeof tileData.w === 'number') {
                            collisionArray = [0, 0, 0, 0]; // Default offsets for non-walkable
                            if (tileData.w === 1) {
                                collisionArray = [16, 16, 16, 16]; // Fully walkable
                            }
                        }
    
                        index++;
    
                        if (collisionArray) {
                            const [nOffset, eOffset, sOffset, wOffset] = collisionArray;
                            return (
                                objectCollisionBox.x < tileRect.x + tileRect.width - eOffset &&
                                objectCollisionBox.x + objectCollisionBox.width > tileRect.x + wOffset &&
                                objectCollisionBox.y < tileRect.y + tileRect.height - sOffset &&
                                objectCollisionBox.y + objectCollisionBox.height > tileRect.y + nOffset
                            );
                        }
    
                        return false;
                    });
                });
            });
        }
    
        if (!collisionDetected) {
            for (let id in this.sprites) {
                if (this.sprites[id] !== sprite) {
                    const otherSprite = this.sprites[id];
                    const otherCollisionBox = {
                        x: otherSprite.x,
                        y: otherSprite.y + extraHeadroom,
                        width: otherSprite.width * otherSprite.scale,
                        height: otherSprite.height * otherSprite.scale - 2 * extraHeadroom
                    };
    
                    if (
                        spriteCollisionBox.x < otherCollisionBox.x + otherCollisionBox.width &&
                        spriteCollisionBox.x + spriteCollisionBox.width > otherCollisionBox.x &&
                        spriteCollisionBox.y < otherCollisionBox.y + otherCollisionBox.height &&
                        spriteCollisionBox.y + spriteCollisionBox.height > otherCollisionBox.y
                    ) {
                        collisionDetected = true;
                        break;
                    }
                }
            }
        }
    
        if (collisionDetected) {

        }
    
        return collisionDetected;
    },

    resolveCollision: function(sprite1, sprite2) {
        const overlapX = (sprite1.x + sprite1.width / 2) - (sprite2.x + sprite2.width / 2);
        const overlapY = (sprite1.y + sprite1.height / 2) - (sprite2.y + sprite2.height / 2);
    
        const absOverlapX = Math.abs(overlapX);
        const absOverlapY = Math.abs(overlapY);
    
        if (absOverlapX < absOverlapY) {
            if (overlapY < 0) {
                sprite1.y -= absOverlapY / 2;
                sprite2.y += absOverlapY / 2;
            } else {
                sprite1.y += absOverlapY / 2;
                sprite2.y -= absOverlapY / 2;
            }
        } else {
            if (overlapX < 0) {
                sprite1.x -= absOverlapX / 2;
                sprite2.x += absOverlapX / 2;
            } else {
                sprite1.x += absOverlapX / 2;
                sprite2.x -= absOverlapX / 2;
            }
        }
        console.log("sprite colliding");
    }
};
