var debug = {
    sprite: function() {
        // Get the sprite's position and dimensions
        const spriteX = sprite.x;
        const spriteY = sprite.y;
        const spriteWidth = sprite.width * sprite.scale;
        const spriteHeight = sprite.height * sprite.scale;

        // Save the current context state
        game.ctx.save();

        // Draw a border around the sprite
        game.ctx.strokeStyle = 'red'; // Set the border color
        game.ctx.lineWidth = 2; // Set the border width
        game.ctx.strokeRect(spriteX, spriteY, spriteWidth, spriteHeight);

        // Restore the context state
        game.ctx.restore();
    },
    camera: function() {

    },
    fps: function() {
        var debugFPS = document.getElementById('gameFps');
        if (debugFPS) {
            debugFPS.innerHTML = "FPS: " + game.fps.toFixed(3);
        }
    },
    grid: function() {
        game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
        game.ctx.lineWidth = 1;
        for (var x = 0; x <= game.worldWidth; x += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(x, 0);
            game.ctx.lineTo(x, game.worldHeight);
            game.ctx.stroke();
        }
        for (var y = 0; y <= game.worldHeight; y += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(0, y);
            game.ctx.lineTo(game.worldWidth, y);
            game.ctx.stroke();
        }
    },
    tiles: function() {
        // Render tiles
        game.roomData.items.forEach(roomItem => {
            const itemTiles = assets.load('objectData')[roomItem.id];
            if (itemTiles) {
                roomItem.p.forEach((position, index) => {
                    const tile = itemTiles[index];
                    if (tile) {
                        const posX = parseInt(position.x, 10) * 16;
                        const posY = parseInt(position.y, 10) * 16;

                        // Draw a border around the tile
                        game.ctx.strokeStyle = 'red';
                        game.ctx.lineWidth = 2;
                        game.ctx.strokeRect(posX, posY, 16, 16);
                    }
                });
            }
        });
    }
};