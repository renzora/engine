var collision = {
    check: function(x, y, sprite) {
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
    
        if (game.roomData && game.roomData.items) {
            collisionDetected = game.roomData.items.some(roomItem => {
                
                const itemData = game.objectData[roomItem.id];
                if (!itemData) return false;
    
                const xCoordinates = roomItem.x || [];
                const yCoordinates = roomItem.y || [];
    
                return yCoordinates.some((tileY, rowIndex) => {
                    return xCoordinates.some((tileX, colIndex) => {
                        const index = rowIndex * xCoordinates.length + colIndex;
                        const tileData = itemData[0]; // Assuming we are dealing with the first tile data group
                        const tilePosX = tileX * 16 + tileData.a[index % tileData.a.length];
                        const tilePosY = tileY * 16 + tileData.b[index % tileData.b.length];
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
                            if (tileData.w === 1) {
                                collisionArray = [16, 16, 16, 16]; // Fully walkable
                            } else if (tileData.w === 0) {
                                collisionArray = [0, 0, 0, 0]; // Fully non-walkable
                            }
                        }
    
                        if (collisionArray) {
                            const [nOffset, eOffset, sOffset, wOffset] = collisionArray;
                            return (
                                objectCollisionBox.x < tileRect.x + tileRect.width - eOffset &&
                                objectCollisionBox.x + objectCollisionBox.width > tileRect.x + wOffset &&
                                objectCollisionBox.y < tileRect.y + tileRect.height - sOffset &&
                                objectCollisionBox.y + objectCollisionBox.height > tileRect.y + nOffset
                            );

                            audio.playAudio("bump1", assets.load('bump1'), 'sfx');
                        }
    
                        return false;
                    });
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
    }
}