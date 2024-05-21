var game = {
    canvas: undefined,
    ctx: undefined,
    timestamp: 0,
    lastTime: 0,
    deltaTime: 0,
    worldWidth: 640,
    worldHeight: 640,
    zoomLevel: 4,
    roomData: undefined,

    init: function () {
        assets.preload([
            { name: 'sprite', path: 'img/sprites/character.png' },
            { name: 'tileset', path: 'img/sprites/items.png' },
            { name: 'items', path: 'json/items.json' },
            { name: 'roomData', path: 'json/roomData.json' },
        ], () => {
            console.log("All assets loaded");
            this.canvas = document.createElement('canvas');
            this.ctx = this.canvas.getContext('2d');
            document.body.appendChild(this.canvas);
            this.resizeCanvas();
            this.roomData = assets.load('roomData');
            this.loop();
        });

    },

    resizeCanvas: function() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    },

    loop: function(timestamp) {
        this.deltaTime = timestamp - this.lastTime;
        this.lastTime = timestamp;
    
        sprite.update();
        camera.update();
        this.render();
        requestAnimationFrame(this.loop.bind(this));
    },

    collision: function(x, y) {
        let collisionDetected = false;
        if(game.roomData && game.roomData.items) {
            collisionDetected = game.roomData.items.some(roomItem => {
                const itemTiles = assets.load('items')[roomItem.id];
                if (!itemTiles) return false;
    
                return roomItem.p.some((position, index) => {
                    const tile = itemTiles[index];
                    if(tile && Array.isArray(tile.w) && tile.w.length === 4) {
                        const [nOffset, eOffset, sOffset, wOffset] = tile.w;
    
                        const tileRect = {
                            x: parseInt(position.x, 10) * 16,
                            y: parseInt(position.y, 10) * 16,
                            width: 16,
                            height: 16
                        };
    
                        const spriteRect = {
                            x: x,
                            y: y,
                            width: sprite.width * sprite.scale,
                            height: sprite.height * sprite.scale
                        };
    
                        return spriteRect.x < tileRect.x + tileRect.width - eOffset &&
                               spriteRect.x + spriteRect.width > tileRect.x + wOffset &&
                               spriteRect.y < tileRect.y + tileRect.height - sOffset &&
                               spriteRect.y + spriteRect.height > tileRect.y + nOffset;
                    }
    
                    return false;
                });
            });
        }
        return collisionDetected;
    },    
    
    render: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        this.ctx.setTransform(1, 0, 0, 1, 0, 0);
        this.ctx.scale(this.zoomLevel, this.zoomLevel);
        this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
        debug.grid();
    
        let renderQueue = [];
    
        this.roomData.items.forEach(roomItem => {
            const itemTiles = assets.load('items')[roomItem.id];
            if (itemTiles) {
                roomItem.p.forEach((position, index) => {
                    const tile = itemTiles[index];
                    if(tile) {
                        const posX = parseInt(position.x, 10) * 16;
                        const posY = parseInt(position.y, 10) * 16;
    
                        renderQueue.push({
                            tileIndex: tile.t,
                            posX: posX,
                            posY: posY,
                            z: tile.z,
                            draw: function() {
                                const srcX = (this.tileIndex % 150) * 16;
                                const srcY = Math.floor(this.tileIndex / 150) * 16;
                                game.ctx.drawImage(assets.load('tileset'), srcX, srcY, 16, 16, this.posX, this.posY, 16, 16);
                            }
                        });
                    }
                });
            }
        });
    
        renderQueue.push({
            z: 1,
            draw: function() {
                sprite.draw();
            }
        });
    
        renderQueue.sort((a, b) => a.z - b.z);
        renderQueue.forEach(item => item.draw());
        this.ctx.imageSmoothingEnabled = false;
    
        // Draw selection rectangle if in progress
        if (input.isSelecting && input.selectionStart && input.selectionEnd) {
            const startX = Math.min(input.selectionStart.x, input.selectionEnd.x);
            const startY = Math.min(input.selectionStart.y, input.selectionEnd.y);
            const endX = Math.max(input.selectionStart.x, input.selectionEnd.x) + 16;
            const endY = Math.max(input.selectionStart.y, input.selectionEnd.y) + 16;
    
            this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
            this.ctx.lineWidth = 2 / this.zoomLevel;
            this.ctx.strokeRect(startX, startY, endX - startX, endY - startY);
        }
    
        // Highlight selected tiles
        input.selectedTiles.forEach(tile => {
            this.ctx.fillStyle = 'rgba(0, 255, 0, 0.1)'; // Semi-transparent green
            this.ctx.fillRect(tile.x, tile.y, 16, 16);
        });
    }
    
};