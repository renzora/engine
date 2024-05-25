var camera = {
    cameraX: 0,
    cameraY: 0,
    update: function() {
        if (editor.isEditMode) {
            // Editor mode logic here
        } else {
            // Calculate the scaled window dimensions
            var scaledWindowWidth = window.innerWidth / game.zoomLevel;
            var scaledWindowHeight = window.innerHeight / game.zoomLevel;

            // Check if the world dimensions are smaller than the canvas dimensions
            if (game.worldWidth < scaledWindowWidth || game.worldHeight < scaledWindowHeight) {
                // Calculate the difference and divide by 2 to center
                var xOffset = game.worldWidth < scaledWindowWidth ? (scaledWindowWidth - game.worldWidth) / 2 : 0;
                var yOffset = game.worldHeight < scaledWindowHeight ? (scaledWindowHeight - game.worldHeight) / 2 : 0;

                // Adjust camera to center the map
                this.cameraX = -xOffset;
                this.cameraY = -yOffset;
            } else {
                // Center the camera on the main sprite, considering the scaled window size
                let mainSprite = game.sprites['main'];
                if (mainSprite) {
                    this.cameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                    this.cameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;

                    // Ensure the camera doesn't go outside the world bounds
                    this.cameraX = Math.max(0, Math.min(this.cameraX, game.worldWidth - scaledWindowWidth));
                    this.cameraY = Math.max(0, Math.min(this.cameraY, game.worldHeight - scaledWindowHeight));
                } else {
                    console.error('Main sprite not found.');
                }
            }
        }
    }
};