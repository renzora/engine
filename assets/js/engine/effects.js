const effects = {

    shakeMap: function(duration, intensity) {
        const originalCameraX = camera.cameraX;
        const originalCameraY = camera.cameraY;

        let elapsed = 0;
        const shakeInterval = setInterval(() => {
            elapsed += 16; // Approximate frame duration

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

    // New Letterbox Effect
    letterboxEffect: {
        active: false,
        barHeight: 0,
        maxBarHeight: 130, // Maximum height for the bars
        speed: 3, // Speed at which bars move in/out
        start: function() {
            this.active = true;
            this.barHeight = 0; // Start with the bars at 0 height
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
        },
        render: function() {
            if (this.barHeight > 0) {
                // Reset any transformations and ensure bars are drawn over everything
                game.ctx.setTransform(1, 0, 0, 1, 0, 0);
                
                // Draw top bar - fully black
                game.ctx.fillStyle = 'rgba(0, 0, 0, 1)'; // Solid black color for the bars
                game.ctx.fillRect(0, 0, game.canvas.width, this.barHeight); // Top bar
                
                // Draw bottom bar - fully black
                game.ctx.fillRect(0, game.canvas.height - this.barHeight, game.canvas.width, this.barHeight); // Bottom bar
            }
        }
    }
    

};
