var game = {
    canvas: undefined,
    ctx: undefined,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 640,
    worldHeight: 640,
    zoomLevel: 5,
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
            x: 300,
            y: 110,
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
        camera.update();
        this.render();
        requestAnimationFrame(this.loop.bind(this));
    },

    collision: function(x, y, sprite) {
        let collisionDetected = false;
        const extraHeadroom = 2;
    
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
    
        if (game.roomData && game.roomData.items) {
            collisionDetected = game.roomData.items.some(roomItem => {
                const itemTiles = assets.load('objectData')[roomItem.id];
                if (!itemTiles) return false;
    
                return roomItem.p.some((position, index) => {
                    const tile = itemTiles[index];
                    if (tile) {
                        const tileRect = {
                            x: parseInt(position.x, 10) * 16,
                            y: parseInt(position.y, 10) * 16,
                            width: 16,
                            height: 16
                        };
    
                        if (Array.isArray(tile.w) && tile.w.length === 4) {
                            const [nOffset, eOffset, sOffset, wOffset] = tile.w;
                            return (
                                objectCollisionBox.x < tileRect.x + tileRect.width - eOffset &&
                                objectCollisionBox.x + objectCollisionBox.width > tileRect.x + wOffset &&
                                objectCollisionBox.y < tileRect.y + tileRect.height - sOffset &&
                                objectCollisionBox.y + objectCollisionBox.height > tileRect.y + nOffset
                            );
                        } else if (typeof tile.w === 'number') {
                            const isWalkable = tile.w === 1;
                            const isColliding =
                                objectCollisionBox.x < tileRect.x + tileRect.width &&
                                objectCollisionBox.x + objectCollisionBox.width > tileRect.x &&
                                objectCollisionBox.y < tileRect.y + tileRect.height &&
                                objectCollisionBox.y + objectCollisionBox.height > tileRect.y;
                            return isColliding && !isWalkable;
                        }
                    }
                    return false;
                });
            });
        }
    
        if (!collisionDetected) {
            for (let id in game.sprites) {
                if (game.sprites[id] !== sprite) {
                    const otherSprite = game.sprites[id];
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

        this.roomData.items.forEach(roomItem => {
            const itemTiles = assets.load('objectData')[roomItem.id];
            if (itemTiles) {
                roomItem.p.forEach((position, index) => {
                    const tile = itemTiles[index];
                    if(tile) {
                        const posX = parseInt(position.x, 10) * 16;
                        const posY = parseInt(position.y, 10) * 16;

                        renderQueue.push({
                            tileIndex: tile.i,
                            posX: posX,
                            posY: posY,
                            z: tile.z,
                            draw: function() {
                                const srcX = (this.tileIndex % 150) * 16;
                                const srcY = Math.floor(this.tileIndex / 150) * 16;
                                game.ctx.drawImage(assets.load(tile.t), srcX, srcY, 16, 16, this.posX, this.posY, 16, 16);
                            }
                        });
                    }
                });
            }
        });

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
    }
};
