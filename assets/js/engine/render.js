var render = {
    updateGameLogic: function(deltaTime) {
    
        for (let id in game.sprites) {
            const sprite = game.sprites[id];
            if (sprite.update) {
                sprite.update(deltaTime);
                sprite.checkTileActions();
            }
        }
    
        game.gameTime.update(deltaTime);
        lighting.updateDayNightCycle();
        animate.updateAnimatedTiles(deltaTime);
        weather.updateSnow(deltaTime);
        weather.updateRain(deltaTime);
        weather.updateFog(deltaTime);
        weather.updateFireflys(deltaTime);
        weather.updateLightning(deltaTime);
        camera.update();
        particles.updateParticles(deltaTime);
        effects.transitions.update();
        lighting.updateLights(deltaTime);

            // Rain sound effect handling
    if (weather.rainActive) {
        audio.playAudio("rain", assets.load('rain'), 'ambience', true);
    } else {
        audio.stopLoopingAudio('rain', 'ambience', 0.5);
    }

        if(typeof ui_window !== 'undefined' && ui_window.checkAndUpdateUIPositions) {
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
    }
}