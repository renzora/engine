var debug = {
    sprite: function() {

    },
    camera: function() {

    },
    fps: function() {
        var debugFPS = document.getElementById('gameFps');
        if(debugFPS) { debugFPS.innerHTML = "FPS: " + game.fps.toFixed(3); }
    },
    grid: function() {
        game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
        game.ctx.lineWidth = 1;
    
        for(var x = 0; x <= game.worldWidth; x += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(x, 0);
            game.ctx.lineTo(x, game.worldHeight);
            game.ctx.stroke();
        }
    
        for(var y = 0; y <= game.worldHeight; y += 16) {
            game.ctx.beginPath();
            game.ctx.moveTo(0, y);
            game.ctx.lineTo(game.worldWidth, y);
            game.ctx.stroke();
        }
    }
}