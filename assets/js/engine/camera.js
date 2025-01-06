camera = {
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    activeCamera: true,
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    lerpEnabled: true,
    manual: false,
    panning: false,
    panSpeed: 5,
    cutsceneMode: false,

    setCameraPosition: function(x, y) {
        this.cameraX = x;
        this.cameraY = y;
        this.manual = true;
        this.panning = false;
        this.activeCamera = false;
        this.cutsceneMode = false;
    },

    startCutscene: function() {
        this.cutsceneMode = true; 
        this.activeCamera = false;
    },

    endCutscene: function() {
        this.cutsceneMode = false;
        this.activeCamera = true;
    },

    panTo: function(targetX, targetY, speed, random) {
        if (random) {
            this.targetCameraX = Math.random() * (game.worldWidth - game.canvas.width / game.zoomLevel);
            this.targetCameraY = Math.random() * (game.worldHeight - game.canvas.height / game.zoomLevel);
        } else {
            this.targetCameraX = targetX;
            this.targetCameraY = targetY;
        }
    
        this.panSpeed = speed || this.panSpeed;
        this.panning = true; 
        this.manual = false;
        this.activeCamera = false;
        this.cutsceneMode = true;
    },

    update: function() {
        let activeSprite = game.sprites[game.playerid];
    
        if (game.isEditorActive) {
            return;
        }
    
        if (activeSprite && !this.cutsceneMode) {
            if (activeSprite.x !== this.targetCameraX || activeSprite.y !== this.targetCameraY) {
                this.activeCamera = true;
                this.panning = false;
            }
        }
    
        if (this.activeCamera || this.panning) {
            if (this.manual && !this.panning) {
                return;
            }
    
            if (this.panning) {
                let deltaX = this.targetCameraX - this.cameraX;
                let deltaY = this.targetCameraY - this.cameraY;
                let distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        
                if (distance <= this.panSpeed) {
                    if (this.targetCameraX === null && this.targetCameraY === null) {
                        this.panning = false;
                    } else {
                        this.targetCameraX = Math.random() * (game.worldWidth - game.canvas.width / game.zoomLevel);
                        this.targetCameraY = Math.random() * (game.worldHeight - game.canvas.height / game.zoomLevel);
                    }
                } else {
                    this.cameraX += (deltaX / distance) * Math.min(this.panSpeed, distance);
                    this.cameraY += (deltaY / distance) * Math.min(this.panSpeed, distance);
                }
            } else if (this.activeCamera && activeSprite) {
                var scaledWindowWidth = game.canvas.width / game.zoomLevel;
                var scaledWindowHeight = (game.canvas.height / game.zoomLevel) - (50 / game.zoomLevel);
    
                this.targetCameraX = activeSprite.x + activeSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = activeSprite.y + activeSprite.height / 2 - scaledWindowHeight / 2;
    
                this.targetCameraX = Math.max(0, Math.min(this.targetCameraX, game.worldWidth - scaledWindowWidth));
                this.targetCameraY = Math.max(0, Math.min(this.targetCameraY, game.worldHeight - scaledWindowHeight));
    
                if (this.lerpEnabled) {
                    this.cameraX = this.lerp(this.cameraX, this.targetCameraX, this.lerpFactor);
                    this.cameraY = this.lerp(this.cameraY, this.targetCameraY, this.lerpFactor);
                } else {
                    this.cameraX = this.targetCameraX;
                    this.cameraY = this.targetCameraY;
                }
            }
        }
    },    
        
    lerp: function(start, end, t) {
        return start * (1 - t) + end * t;
    }
};
