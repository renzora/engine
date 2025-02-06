camera = {
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    active: true,
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    lerpEnabled: true,
    manual: false,

    update() {
        if (game.isEditorActive) return;

        let activeSprite = game.sprites[game.playerid];
        if (!activeSprite) return;

        if (activeSprite.x !== this.targetCameraX || activeSprite.y !== this.targetCameraY) {
            this.activeCamera = true;
        }

        if (!this.activeCamera) return;
        if (this.manual) return;

        if (this.activeCamera) {
            var scaledWindowWidth = game.canvas.width / game.zoomLevel;
            var scaledWindowHeight = (game.canvas.height / game.zoomLevel) - (50 / game.zoomLevel);

            this.targetCameraX = activeSprite.x + activeSprite.width / 2 - scaledWindowWidth / 2;
            this.targetCameraY = activeSprite.y + activeSprite.height / 2 - scaledWindowHeight / 2;

            this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
            this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));

            if (this.lerpEnabled) {
                this.cameraX = this.cameraX * (1 - this.lerpFactor) + this.targetCameraX * this.lerpFactor;
                this.cameraY = this.cameraY * (1 - this.lerpFactor) + this.targetCameraY * this.lerpFactor;
            } else {
                this.cameraX = this.targetCameraX;
                this.cameraY = this.targetCameraY;
            }
        }
        plugin.hook('onCameraUpdate');
    }
};