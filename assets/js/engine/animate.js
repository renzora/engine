var animate = {
    updateAnimatedTiles: function(deltaTime) {
        if (!game.roomData || !game.roomData.items) return;
    
        // Iterate over each item in the room data
        game.roomData.items.forEach(roomItem => {
            const itemData = assets.load('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                // Initialize roomItem's animation state if not already present
                if (!roomItem.animationState) {
                    roomItem.animationState = itemData.map(tileData => ({
                        currentFrame: 0,
                        elapsedTime: 0
                    }));
                }
    
                // Update each tile's animation state
                itemData.forEach((tileData, index) => {
                    if (tileData.i && Array.isArray(tileData.i[0])) {
                        const animationData = tileData.i;
                        const animationState = roomItem.animationState[index];
    
                        animationState.elapsedTime += deltaTime;
    
                        // Ensure that the frame only advances once per elapsed time period
                        if (animationState.elapsedTime >= tileData.d) {
                            animationState.elapsedTime -= tileData.d;
                            animationState.currentFrame = (animationState.currentFrame + 1) % animationData.length;
                        }
    
                        // Apply the current frame to the tileData for rendering
                        tileData.currentFrame = animationState.currentFrame;
                    }
                });
            }
        });
    }
}