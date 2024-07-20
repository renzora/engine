var camera = {
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    activeCamera: true,
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    lerpEnabled: true,

    update: function() {
        if (this.activeCamera) {
            let mainSprite = game.sprites[game.playerid];
            if (mainSprite) {
                var scaledWindowWidth = window.innerWidth / game.zoomLevel;
                var scaledWindowHeight = window.innerHeight / game.zoomLevel;
    
                this.targetCameraX = mainSprite.x + mainSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = mainSprite.y + mainSprite.height / 2 - scaledWindowHeight / 2;
    
                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));
    
                if (this.lerpEnabled) {
                    this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
                    this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);
                } else {
                    this.cameraX = this.targetCameraX;
                    this.cameraY = this.targetCameraY;
                }
    
                // Center map if smaller than viewport
                if (game.worldWidth < scaledWindowWidth) {
                    this.cameraX = -(scaledWindowWidth - game.worldWidth) / 2;
                }
                if (game.worldHeight < scaledWindowHeight) {
                    this.cameraY = -(scaledWindowHeight - game.worldHeight) / 2;
                }
    
                if (typeof debug_window !== 'undefined' && debug_window.camera) {
                    debug_window.camera();
                }
            } else {
                console.error('Main sprite not found.');
            }
        }
    },
    lerp: function(start, end, t) {
        return start * (1 - t) + end * t;
    }
}