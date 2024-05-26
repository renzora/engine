var debug = {
    sprite: function(sprite) {
        // Get the sprite's position and dimensions
        const spriteX = sprite.x;
        const spriteY = sprite.y;
        const spriteWidth = sprite.width * sprite.scale;
        const spriteHeight = sprite.height * sprite.scale;

        // Save the current context state
        game.ctx.save();

        // Restore the context state
        game.ctx.restore();
    },
    camera: function() {
        // Implementation for debugging camera
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
        if (!game.roomData || !game.roomData.items) return;

        game.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                const tileData = itemData[0]; // Assuming all tiles in the itemData array are the same
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];

                let index = 0; // Initialize the index

                for (let j = 0; j < yCoordinates.length; j++) {
                    for (let i = 0; i < xCoordinates.length; i++) {
                        const posX = parseInt(xCoordinates[i], 10) * 16;
                        const posY = parseInt(yCoordinates[j], 10) * 16;

                        // Determine collision data
                        let collisionOffsets;
                        if (Array.isArray(tileData.w)) {
                            collisionOffsets = tileData.w[index % tileData.w.length];
                        } else {
                            collisionOffsets = tileData.w === 0 ? [0, 0, 0, 0] : null;
                        }

                        if (collisionOffsets) {
                            const [nOffset, eOffset, sOffset, wOffset] = collisionOffsets;

                            // Calculate collision box
                            const collisionX = posX + wOffset;
                            const collisionY = posY + nOffset;
                            const collisionWidth = 16 - wOffset - eOffset;
                            const collisionHeight = 16 - nOffset - sOffset;

                            // Save the current context state
                            game.ctx.save();

                            // Draw a border around the collision area
                            game.ctx.strokeStyle = 'red'; // Set the border color
                            game.ctx.lineWidth = 1; // Set the border width
                            game.ctx.strokeRect(collisionX, collisionY, collisionWidth, collisionHeight);

                            // Set text properties
                            game.ctx.fillStyle = 'black'; // Text color
                            game.ctx.font = '2px Arial'; // Font size and family

                            // Render the text inside the tile
                            const text = collisionOffsets.join(','); // Get the specific value from the w array
                            game.ctx.fillText(text, posX + 2, posY + 12); // Adjust the position as needed

                            // Restore the context state
                            game.ctx.restore();
                        }

                        index++;
                    }
                }
            }
        });

        // Draw only the collision box for sprites
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite) {
                // Calculate the collision box position and dimensions
                const extraHeadroom = 2;
                const collisionBox = {
                    x: sprite.x,
                    y: sprite.y + sprite.height * sprite.scale / 2,
                    width: sprite.width * sprite.scale,
                    height: sprite.height * sprite.scale / 2
                };

                // Save the current context state
                game.ctx.save();

                // Draw the collision box
                game.ctx.strokeStyle = 'green'; // Set the collision box color
                game.ctx.lineWidth = 1; // Set the border width
                game.ctx.strokeRect(collisionBox.x, collisionBox.y, collisionBox.width, collisionBox.height);

                // Restore the context state
                game.ctx.restore();
            }
        }
    }
};
