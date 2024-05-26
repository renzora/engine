var game = {
    canvas: undefined,
    ctx: undefined,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 640,
    worldHeight: 640,
    zoomLevel: 6,
    roomData: undefined,
    sprites: {},

    init: function () {
        assets.preload([
            { name: 'character', path: 'img/sprites/character.png' },
            { name: 'hair', path: 'img/sprites/hair.png' },
            { name: 'hats', path: 'img/sprites/hats.png' },
            { name: 'glasses', path: 'img/sprites/glasses.png' },
            { name: 'facial', path: 'img/sprites/facial.png' },
            { name: 'outfit', path: 'img/sprites/outfit.png' },
            { name: '1', path: 'img/tiles/1.png' },
            { name: 'objectData', path: 'json/objectData.json' },
            { name: 'roomData', path: 'json/roomData.json' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.roomData = assets.load('roomData');
            this.createMainSprite();
            this.setupInputHandlers();
            this.loop();
        });
    },

    createMainSprite: function() {
        this.mainSprite = Sprite({
            x: 80,
            y: 200,
            hairstyle: 5,
            outfit: 3,
            facialHair: 1,
            hat: 0,
            glasses: 0
        });
        this.sprites['main'] = this.mainSprite;
    },

    createSprite: function(options) {
        let newSprite = Sprite(options);
        this.sprites[options.id] = newSprite;
        return newSprite;
    },

    moveSprite: function(sprite, direction, duration) {
        if (sprite) {
            sprite.addDirection(direction);
            setTimeout(() => {
                sprite.removeDirection(direction);
            }, duration * 1000);
        }
    },

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    },

    setupInputHandlers: function() {
        window.addEventListener('keydown', this.handleKeyDown.bind(this));
        window.addEventListener('keyup', this.handleKeyUp.bind(this));
    },

    handleKeyDown: function(event) {
        const directionMap = {
            'ArrowUp': 'up',
            'ArrowDown': 'down',
            'ArrowLeft': 'left',
            'ArrowRight': 'right'
        };
        let direction = directionMap[event.key];
        if (direction) {
            this.mainSprite.addDirection(direction);
        }
    },

    handleKeyUp: function(event) {
        const directionMap = {
            'ArrowUp': 'up',
            'ArrowDown': 'down',
            'ArrowLeft': 'left',
            'ArrowRight': 'right'
        };
        let direction = directionMap[event.key];
        if (direction) {
            this.mainSprite.removeDirection(direction);
        }
    },

    loop: function(timestamp) {
        if (!this.lastTime) {
            this.lastTime = timestamp;
        }

        this.deltaTime = timestamp - this.lastTime;
        this.lastTime = timestamp;

        for (let id in this.sprites) {
            this.sprites[id].update();
        }
        this.updateAnimatedTiles(this.deltaTime);
        camera.update();
        this.render();
        requestAnimationFrame(this.loop.bind(this));
    },

    updateAnimatedTiles: function(deltaTime) {
        if (!this.roomData || !this.roomData.items) return;

        this.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                itemData.forEach(tileData => {
                    if (tileData.d) { // Check if the tile is animated
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
                            // Handle walkable or non-walkable tiles
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
    
        return collisionDetected;
    },
    
    

    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
    
        let renderQueue = [];
    
        if (this.roomData && this.roomData.items) {
            this.roomData.items.forEach(roomItem => {
                const itemData = assets.load('objectData')[roomItem.id];
                if (itemData && itemData.length > 0) {
                    const tileData = itemData[0]; // Assuming all tiles in the itemData array are the same
                    const xCoordinates = roomItem.x || [];
                    const yCoordinates = roomItem.y || [];
    
                    let index = 0; // Initialize index
    
                    for (let y = Math.min(...yCoordinates); y <= Math.max(...yCoordinates); y++) {
                        for (let x = Math.min(...xCoordinates); x <= Math.max(...xCoordinates); x++) {
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
    
                            index++;
                        }
                    }
                }
            });
        }
    
        //debug.grid();
    
        for (let id in this.sprites) {
            renderQueue.push({
                z: 1,
                draw: function() {
                    game.sprites[id].draw();
                }
            });
        }
    
        renderQueue.sort((a, b) => a.z - b.z);
        renderQueue.forEach(item => item.draw());
        this.ctx.imageSmoothingEnabled = false;
    
        if (editor.isSelecting && editor.selectionStart && editor.selectionEnd) {
            const startX = Math.min(editor.selectionStart.x, editor.selectionEnd.x);
            const startY = Math.min(editor.selectionStart.y, editor.selectionEnd.y);
            const endX = Math.max(editor.selectionStart.x, editor.selectionEnd.x) + 16;
            const endY = Math.max(editor.selectionStart.y, editor.selectionEnd.y) + 16;
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.lineWidth = 4 / this.zoomLevel;
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
        }
    
        editor.selectedTiles.forEach(tile => {
            this.ctx.fillStyle = 'rgba(0, 255, 0, 0.2)';
            this.ctx.fillRect(tile.x, tile.y, 16, 16);
        });
        //debug.tiles();  // Call to debug tiles
    }
    
    
};
