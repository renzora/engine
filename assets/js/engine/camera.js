var camera = {
    cameraX: 0,
    cameraY: 0,
    update: function() {
        if (editor.isEditMode) {
            // Existing code for edit mode
        } else {
            var scaledWindowWidth = window.innerWidth / game.zoomLevel;
            var scaledWindowHeight = window.innerHeight / game.zoomLevel;
    
            if (game.worldWidth < scaledWindowWidth || game.worldHeight < scaledWindowHeight) {
                var xOffset = game.worldWidth < scaledWindowWidth ? (scaledWindowWidth - game.worldWidth) / 2 : 0;
                var yOffset = game.worldHeight < scaledWindowHeight ? (scaledWindowHeight - game.worldHeight) / 2 : 0;
    
                this.cameraX = -xOffset;
                this.cameraY = -yOffset;
            } else {
                let mainSprite = game.sprites['main'];
                if (mainSprite) {
                    this.cameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                    this.cameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;
    
                    this.cameraX = Math.max(0, Math.min(this.cameraX, game.worldWidth - scaledWindowWidth));
                    this.cameraY = Math.max(0, Math.min(this.cameraY, game.worldHeight - scaledWindowHeight));
    
                    // Round the camera position to the nearest integer
                    this.cameraX = Math.round(this.cameraX);
                    this.cameraY = Math.round(this.cameraY);
                } else {
                    console.error('Main sprite not found.');
                }
            }
        }
    }
    
};
