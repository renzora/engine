pathfinding = {
    start: function() {

    },

    unmount: function() {

    },

    cancelPathfinding: function(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false;
            plugin.audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        }
    }
}