const effects = {

    shakeMap: function(duration, intensity) {
        const originalCameraX = camera.cameraX;
        const originalCameraY = camera.cameraY;

        let elapsed = 0;
        const shakeInterval = setInterval(() => {
            elapsed += 16;

            if (elapsed < duration) {
                const offsetX = (Math.random() - 0.5) * intensity;
                const offsetY = (Math.random() - 0.5) * intensity;

                camera.cameraX = originalCameraX + offsetX;
                camera.cameraY = originalCameraY + offsetY;
            } else {
                clearInterval(shakeInterval);
                camera.cameraX = originalCameraX;
                camera.cameraY = originalCameraY;
            }
        }, 16);
    },

    transitions: {
        active: false,
        type: 'fadeIn',
        duration: 1000,
        startTime: null,
        progress: 0,

        start: function(type, duration) {
            this.active = true;
            this.type = type || 'fadeIn';
            this.duration = duration || 1000;
            this.startTime = performance.now();
            this.progress = 0;
        },

        update: function() {
            if (!this.active) return;

            const currentTime = performance.now();
            const elapsed = currentTime - this.startTime;
            this.progress = Math.min(elapsed / this.duration, 1);

            if (this.progress >= 1) {
                this.active = false;
            }
        },

        render: function() {
            if (!this.active) return;

            switch (this.type) {
                case 'fadeIn':
                    this.renderFadeIn();
                    break;
                case 'fadeOut':
                    this.renderFadeOut();
                    break;
            }
        },

        renderFadeIn: function() {
            const opacity = 1 - this.progress;
            game.ctx.fillStyle = `rgba(0, 0, 0, ${opacity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
        },

        renderFadeOut: function() {
            const opacity = this.progress;
            game.ctx.fillStyle = `rgba(0, 0, 0, ${opacity})`;
            game.ctx.fillRect(0, 0, game.canvas.width, game.canvas.height);
        }
    },

    letterbox: {
        active: false,
        barHeight: 0,
        maxBarHeight: 130,
        speed: 3,
        start: function() {
            this.active = true;
            this.barHeight = 0;
        },
        stop: function() {
            this.active = false;
        },
        update: function() {
            if (this.active && this.barHeight < this.maxBarHeight) {
                this.barHeight += this.speed;
                if (this.barHeight > this.maxBarHeight) {
                    this.barHeight = this.maxBarHeight;
                }
            }
            if (!this.active && this.barHeight > 0) {
                this.barHeight -= this.speed;
                if (this.barHeight < 0) {
                    this.barHeight = 0;
                }
            }
            this.render();
        },
        render: function() {
            if (this.barHeight > 0) {
                game.ctx.setTransform(1, 0, 0, 1, 0, 0);
                game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
                game.ctx.fillRect(0, 0, game.canvas.width, this.barHeight);
                game.ctx.fillRect(0, game.canvas.height - this.barHeight, game.canvas.width, this.barHeight); // Bottom bar
            }
        }
    }

};
