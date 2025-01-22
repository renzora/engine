window["rain"] = {
    id: "rain",
    rainDrops: [],
    active: false,
    overrideActive: false,

    start: function() {
        assets.preload([
            { name: 'rain_plugin_sfx', path: 'plugins/rain/rain.mp3' }
        ], () => {
            this.create(0.7);
            this.active = true;
        });
    },

    unmount: function() {
        assets.unload('rain_plugin_sfx');
        this.rainDrops = [];
        this.active = false;
    },


    onRender: function() {
        game.ctx.restore();
        this.draw();
        this.update();

        if (this.active) {
            if (plugin.exists('audio')) {
                audio.playAudio("rain_plugin_sfx", assets.use('rain_plugin_sfx'), 'ambience', true);
            }
        } else {
            if (plugin.exists('audio')) {
                audio.stopLoopingAudio('rain_plugin_sfx', 'ambience', 0.5);
            }
        }
    },

    create: function(opacity) {
        this.rainDrops = [];
        // 1000 drops across the world
        for (let i = 0; i < 1000; i++) {
            this.rainDrops.push({
                x: Math.random() * game.worldWidth,
                y: Math.random() * game.worldHeight,
                length: Math.random() * 8,
                opacity: Math.random() * opacity,
                speed: 7
            });
        }
    },
    
    update: function() {
        if (!this.active) return;

        for (let drop of this.rainDrops) {
            drop.y += drop.speed;
            // Wrap around from bottom to top
            if (drop.y > game.worldHeight) {
                drop.y = -drop.length;
                drop.x = Math.random() * game.worldWidth;
            }
        }
    },
    
    draw: function() {
        if (!this.active) return;
        // The main transform is already set (zoom + camera translate)
        game.ctx.save();
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
        game.ctx.restore();
    }
};
