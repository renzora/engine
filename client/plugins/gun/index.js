gun = {
    start: function() {

    },

    unmount: function() {

    },

    aimTool: function() {
        if (game.mainSprite && game.mainSprite.targetAim) {
            const handX = game.mainSprite.x + game.mainSprite.width / 2 + game.mainSprite.handOffsetX;
            const handY = game.mainSprite.y + game.mainSprite.height / 2 + game.mainSprite.handOffsetY;
    
            const deltaX = game.mainSprite.targetX - handX;
            const deltaY = game.mainSprite.targetY - handY;
            const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
    
            let adjustedTargetX = game.mainSprite.targetX;
            let adjustedTargetY = game.mainSprite.targetY;
            if (distance > game.mainSprite.maxRange) {
                const ratio = game.mainSprite.maxRange / distance;
                adjustedTargetX = handX + deltaX * ratio;
                adjustedTargetY = handY + deltaY * ratio;
            }
    
            const isObstructed = (x, y) => {
                if (game.roomData && game.roomData.items) {
                    for (const roomItem of game.roomData.items) {
                        const itemData = assets.use('objectData')[roomItem.id];
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
    
                            if (
                                x >= tileRect.x &&
                                x <= tileRect.x + tileRect.width &&
                                y >= tileRect.y &&
                                y <= tileRect.y + tileRect.height
                            ) {
                                const tileData = itemData[0]; 
                                if (tileData.w !== 1) {
                                    return { obstructed: true, collisionX: x, collisionY: y };
                                }
                            }
                        }
                    }
                }
                return { obstructed: false };
            };
    
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
    
            if (obstructionDetected && Math.sqrt((finalTargetX - handX) ** 2 + (finalTargetY - handY) ** 2) < 10) {
                return;
            }
    
            const crosshairSize = 2;
            const sniperLineLength = 4;
            const targetRadius = game.mainSprite.targetRadius * 0.75;
    
            game.ctx.save();
            game.ctx.shadowColor = 'rgba(0, 0, 0, 0.5)';
            game.ctx.shadowBlur = 2;
            game.ctx.shadowOffsetX = 0.3;
            game.ctx.shadowOffsetY = 0.3;
            game.ctx.strokeStyle = 'rgba(255, 0, 0, 0.4)';
            game.ctx.lineWidth = 1;
            game.ctx.lineCap = 'butt';
    
            const centerX = finalTargetX;
            const centerY = finalTargetY;
    
            game.ctx.beginPath();
            game.ctx.moveTo(handX, handY);
            game.ctx.lineTo(centerX, centerY);
            game.ctx.stroke();
    
            const aimCrosshairColor = gamepad.buttons.includes('r2') ? 'rgba(200, 0, 0, 0.8)' : 'rgba(255, 0, 0, 0.4)';
    
            game.ctx.strokeStyle = aimCrosshairColor;
            game.ctx.lineWidth = 1;
            game.ctx.beginPath();
            game.ctx.arc(centerX, centerY, targetRadius, 0, 2 * Math.PI);
            game.ctx.stroke();
            game.ctx.beginPath();
            game.ctx.moveTo(centerX - crosshairSize, centerY);
            game.ctx.lineTo(centerX + crosshairSize, centerY);
            game.ctx.moveTo(centerX, centerY - crosshairSize);
            game.ctx.lineTo(centerX, centerY + crosshairSize);
            game.ctx.stroke();
            game.ctx.beginPath();
            game.ctx.moveTo(centerX - sniperLineLength, centerY);
            game.ctx.lineTo(centerX - crosshairSize, centerY);
            game.ctx.moveTo(centerX + crosshairSize, centerY);
            game.ctx.lineTo(centerX + sniperLineLength, centerY);
            game.ctx.moveTo(centerX, centerY - sniperLineLength);
            game.ctx.lineTo(centerX, centerY - crosshairSize);
            game.ctx.moveTo(centerX, centerY + crosshairSize);
            game.ctx.lineTo(centerX, centerY + sniperLineLength);
            game.ctx.stroke();
            game.ctx.restore();
        }
    }
}