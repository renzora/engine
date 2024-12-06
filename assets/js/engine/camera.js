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

    panTo: function(targetX, targetY, speed, random) {
        if (random) {
            // Generate an initial random target position within the scene boundaries
            this.targetCameraX = Math.random() * (game.worldWidth - game.canvas.width / game.zoomLevel);
            this.targetCameraY = Math.random() * (game.worldHeight - game.canvas.height / game.zoomLevel);
        } else {
            // Set the provided target position
            this.targetCameraX = targetX;
            this.targetCameraY = targetY;
        }
    
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
                // Calculate the distance to the target
                let deltaX = this.targetCameraX - this.cameraX;
                let deltaY = this.targetCameraY - this.cameraY;
                let distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        
                if (distance <= this.panSpeed) {
                    // Target reached: Generate a new random target if random panning is enabled
                    if (this.targetCameraX === null && this.targetCameraY === null) {
                        this.panning = false; // Stop panning if no random target
                    } else {
                        // Generate new random target position
                        this.targetCameraX = Math.random() * (game.worldWidth - game.canvas.width / game.zoomLevel);
                        this.targetCameraY = Math.random() * (game.worldHeight - game.canvas.height / game.zoomLevel);
                    }
                } else {
                    // Move toward the target
                    this.cameraX += (deltaX / distance) * Math.min(this.panSpeed, distance);
                    this.cameraY += (deltaY / distance) * Math.min(this.panSpeed, distance);
                }
            } else if (this.activeCamera && activeSprite) {
                // Automatic camera tracking of the active sprite
                var scaledWindowWidth = game.canvas.width / game.zoomLevel;
                var scaledWindowHeight = (game.canvas.height / game.zoomLevel) - (50 / game.zoomLevel); // Adjust height for the 50px margin
    
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
