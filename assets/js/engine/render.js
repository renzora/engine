var render = {
    updateGameLogic: function(deltaTime) {
        gamepad.updateGamepadState();
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.update) {
                sprite.update(deltaTime);
                sprite.checkTileActions();
            }
        }

        camera.update();
        game.gameTime.update(deltaTime);
        lighting.updateDayNightCycle();
        animate.updateAnimatedTiles(deltaTime);
        weather.updateSnow(deltaTime);
        weather.updateRain(deltaTime);
        weather.updateFireflys(deltaTime);
        particles.updateParticles(deltaTime);
        effects.transitions.update();
        lighting.updateLights(deltaTime);

        // Rain sound effect handling
        if (weather.rainActive) {
            audio.playAudio("rain", assets.load('rain'), 'ambience', true);
        } else {
            audio.stopLoopingAudio('rain', 'ambience', 0.5);
        }

        if (typeof ui_window !== 'undefined' && ui_window.checkAndUpdateUIPositions) {
            ui_window.checkAndUpdateUIPositions();
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

    tooltips: function() {
        if (!game.tooltips.length) return; // Skip rendering if no tooltips

        game.ctx.save(); // Save the current context state
        game.ctx.font = '3px Tahoma';
        game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.8)'; // Set stroke color with opacity
        game.ctx.lineWidth = 1; // Ensure the line width is appropriate
    
        game.tooltips.forEach(tooltip => {
            const textWidth = game.ctx.measureText(tooltip.message).width;
            const x = tooltip.x;
            const y = tooltip.y;
    
            // Measure the text height
            const textHeight = parseInt(game.ctx.font, 10);
            const padding = 2; // Padding around the text
            const rectHeight = textHeight + padding * 2;
            
            // Draw rounded tooltip background
            game.ctx.fillStyle = 'rgba(0, 0, 0, 0.8)'; // Set background fill color with opacity
            const rectX = x;
            const rectY = y - rectHeight - padding;
            const rectWidth = textWidth + padding * 2;
            const borderRadius = 1; // Set the border radius for rounded corners
    
            game.ctx.beginPath();
            game.ctx.moveTo(rectX + borderRadius, rectY);
            game.ctx.lineTo(rectX + rectWidth - borderRadius, rectY);
            game.ctx.quadraticCurveTo(rectX + rectWidth, rectY, rectX + rectWidth, rectY + borderRadius);
            game.ctx.lineTo(rectX + rectWidth, rectY + rectHeight - borderRadius);
            game.ctx.quadraticCurveTo(rectX + rectWidth, rectY + rectHeight, rectX + rectWidth - borderRadius, rectY + rectHeight);
            game.ctx.lineTo(rectX + borderRadius, rectY + rectHeight);
            game.ctx.quadraticCurveTo(rectX, rectY + rectHeight, rectX, rectY + rectHeight - borderRadius);
            game.ctx.lineTo(rectX, rectY + borderRadius);
            game.ctx.quadraticCurveTo(rectX, rectY, rectX + borderRadius, rectY);
            game.ctx.closePath();
            game.ctx.fill();
            game.ctx.stroke();
    
            // Draw tooltip text
            game.ctx.fillStyle = 'white'; // Set text color
            game.ctx.fillText(tooltip.message, x + padding, y - padding - 1);
        });
    
        game.ctx.restore(); // Restore the previous context state
    }
    
    
    
    
    
    
}