window[id] = {
    id: id,
    rainDrops: [],
    active: null,
    overrideActive: false,

    start: function() {
        assets.preload([
            { name: 'rain_plugin_sfx', path: 'plugins/rain/rain.mp3' }  
        ],() => {
            this.create(0.7);
        });
    },

    unmount: function() {
        assets.unload('rain_plugin_sfx');
    },

    onRender: function() {
        this.draw();
        this.update();

        if (this.active) {
            audio.playAudio("rain_plugin_sfx", assets.use('rain_plugin_sfx'), 'ambience', true);
        } else {
            audio.stopLoopingAudio('rain_plugin_sfx', 'ambience', 0.5);
        }
    },

    create: function (opacity) {
        this.rainDrops = [];
        for (let i = 0; i < 1000; i++) {
            this.rainDrops.push({
                x: Math.random() * game.canvas.width,
                y: Math.random() * game.canvas.height,
                length: Math.random() * 8,
                opacity: Math.random() * opacity,
                speed: 7,
            });
        }
    },
    
    update: function () {
        if (!this.active) return;
        for (let drop of this.rainDrops) {
            drop.y += drop.speed;
            if (drop.y > game.canvas.height) {
                drop.y = -drop.length;
                drop.x = Math.random() * game.canvas.width;
            }
        }
        utils.tracker('rain.update()');
    },
    
    draw: function () {
        if (!this.active) return;
        game.ctx.strokeStyle = 'rgba(174, 194, 224, 0.4)';
        game.ctx.lineWidth = 1;
        game.ctx.lineCap = 'round';
        this.rainDrops.forEach((drop) => {
            game.ctx.globalAlpha = drop.opacity;
            game.ctx.beginPath();
            game.ctx.moveTo(drop.x, drop.y);
            game.ctx.lineTo(drop.x, drop.y + drop.length);
            game.ctx.stroke();
        });
        game.ctx.globalAlpha = 1;
    }
};