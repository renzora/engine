animate = {
    updateAnimatedTiles: function(deltaTime) {
        if (!game.roomData || !game.roomData.items) return;
    
        game.roomData.items.forEach(roomItem => {
            const itemData = assets.use('objectData')[roomItem.id];
            if (itemData && itemData.length > 0) {
                if (!roomItem.animationState) {
                    roomItem.animationState = itemData.map(tileData => ({
                        currentFrame: 0,
                        elapsedTime: 0
                    }));
                }
    
                itemData.forEach((tileData, index) => {
                    if (tileData.i && Array.isArray(tileData.i[0])) {
                        const animationData = tileData.i;
                        const animationState = roomItem.animationState[index];
    
                        animationState.elapsedTime += deltaTime;
    
                        if (animationState.elapsedTime >= tileData.d) {
                            animationState.elapsedTime -= tileData.d;
                            animationState.currentFrame = (animationState.currentFrame + 1) % animationData.length;
                        }
    
                        tileData.currentFrame = animationState.currentFrame;
                    }
                });
            }
        });
    }
}