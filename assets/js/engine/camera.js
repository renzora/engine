var camera = {
    cameraX: 0,
    cameraY: 0,
    targetCameraX: 0,
    targetCameraY: 0,
    activeCamera: true,
    lerpFactor: parseFloat(localStorage.getItem('lerpFactor')) || 0.1,
    lerpEnabled: true,
    manual: false, // Flag to determine if the camera is in manual mode
    panning: false, // Flag to check if panning is in progress
    panSpeed: 5, // Default speed for panning
    cutsceneMode: false, // Flag to disable auto-tracking during cutscenes or manual overrides

    // Method to manually set camera position and enable manual mode
    setCameraPosition: function(x, y) {
        this.cameraX = x;
        this.cameraY = y;
        this.manual = true;  // Enable manual mode
        this.panning = false; // Disable panning when manually setting the camera position
        this.activeCamera = false; // Disable automatic tracking in manual mode
        this.cutsceneMode = false; // Turn off cutscene mode if we set a manual position
    },

    // Method to start a cutscene or disable auto tracking
    startCutscene: function() {
        this.cutsceneMode = true;  // Disable automatic tracking during the cutscene
        this.activeCamera = false; // Ensure active camera is off
    },

    // Method to end a cutscene and resume normal camera behavior
    endCutscene: function() {
        this.cutsceneMode = false;  // Re-enable automatic tracking if necessary
        this.activeCamera = true;   // Resume automatic tracking after cutscene
    },

    // Method to pan the camera from the current position to a target position at a fixed speed
    panTo: function(targetX, targetY, speed) {
        this.targetCameraX = targetX;
        this.targetCameraY = targetY;
        this.panSpeed = speed || this.panSpeed;  // Use provided speed or default

        this.panning = true;  // Enable panning mode
        this.manual = false;  // Disable manual mode for panning
        this.activeCamera = false; // Disable automatic tracking during panning
        this.cutsceneMode = true;  // Treat panning as part of cutscene/movement override
    },

    update: function() {
        let activeSprite = game.sprites[game.activeSpriteId];
    
        // Check if the game is in editor mode; if so, stop any camera updates
        if (game.isEditorActive) {
            return; // Exit early if editor mode is active
        }
    
        // Check if there is an active sprite to track
        if (activeSprite && !this.cutsceneMode) {
            // Enable automatic camera tracking if the sprite has moved and cutsceneMode is off
            if (activeSprite.x !== this.targetCameraX || activeSprite.y !== this.targetCameraY) {
                this.activeCamera = true;
                this.panning = false; // Stop panning when sprite moves
            }
        }
    
        if (this.activeCamera || this.panning) {
            // Skip automatic updates if manual mode is active
            if (this.manual && !this.panning) {
                return;  // Exit the update function if manual mode is active
            }
    
            if (this.panning) {
                // Calculate the distance between the current camera position and the target
                let deltaX = this.targetCameraX - this.cameraX;
                let deltaY = this.targetCameraY - this.cameraY;
    
                // Calculate the distance to move in this frame based on speed
                let distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
                let moveX = (deltaX / distance) * this.panSpeed;
                let moveY = (deltaY / distance) * this.panSpeed;
    
                // Check if the camera has reached the target (or close enough)
                if (distance > this.panSpeed) {
                    this.cameraX += moveX;
                    this.cameraY += moveY;
                } else {
                    // Stop panning once the target is reached
                    this.cameraX = this.targetCameraX;
                    this.cameraY = this.targetCameraY;
                    this.panning = false;
    
                    // Optionally resume automatic tracking if cutscene ends after panning
                    if (!this.cutsceneMode) {
                        this.activeCamera = true;
                    }
                }
            } else if (this.activeCamera && activeSprite) {
                // Automatic camera tracking of the active sprite
                var scaledWindowWidth = game.canvas.width / game.zoomLevel; // Updated to reflect new canvas width
                var scaledWindowHeight = game.canvas.height / game.zoomLevel;
    
                // Center the camera on the sprite
                this.targetCameraX = activeSprite.x + activeSprite.width / 2 - scaledWindowWidth / 2;
                this.targetCameraY = activeSprite.y + activeSprite.height / 2 - scaledWindowHeight / 2;
    
                // Recalculate camera boundaries based on the new canvas size and ensure we respect world boundaries
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
