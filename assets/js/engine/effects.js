var effects = {
    transitions: {
        active: false,
        type: 'fadeIn', // default transition type
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
                case 'swirl':
                    this.renderSwirl();
                    break;
                case 'square':
                    this.renderSquare();
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
        },

        renderSwirl: function() {
            const angle = this.progress * Math.PI * 2;
            const radius = Math.max(game.canvas.width, game.canvas.height);
            game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
            game.ctx.save();
            game.ctx.translate(game.canvas.width / 2, game.canvas.height / 2);
            game.ctx.rotate(angle);
            game.ctx.fillRect(-radius, -radius, radius * 2, radius * 2);
            game.ctx.restore();
        },

        renderSquare: function() {
            const size = this.progress * Math.max(game.canvas.width, game.canvas.height);
            const halfSize = size / 2;
            game.ctx.fillStyle = 'rgba(0, 0, 0, 1)';
            game.ctx.fillRect(
                game.canvas.width / 2 - halfSize,
                game.canvas.height / 2 - halfSize,
                size,
                size
            );
        }
    },

    createParticles: function(x, y, options) {
        const particleCount = options.count || 20;
        const baseSpeed = options.speed || 2;
        const baseAngle = options.angle || 0;
        const spread = options.spread || Math.PI * 2; // Default to 360 degrees
        const colors = options.colors || ['rgba(255, 0, 0, 1)']; // Default color if none provided

        for (let i = 0; i < particleCount; i++) {
            const angle = baseAngle + (Math.random() - 0.5) * spread;
            const speed = baseSpeed * (0.5 + Math.random() * 0.5); // Add some variability to the speed
            const color = colors[Math.floor(Math.random() * colors.length)]; // Select a random color

            game.particles.push({
                x: x,
                y: y,
                vx: Math.cos(angle) * speed,
                vy: Math.sin(angle) * speed,
                life: options.life || 50,
                color: color,
                size: options.size || 2,
                type: options.type || 'default' // Add a type to handle different behaviors
            });
        }
    },

    updateParticles: function(deltaTime) {
        game.particles = game.particles.filter(particle => particle.life > 0);

        game.particles.forEach(particle => {
            if (particle.type === 'ripple') {
                particle.size += 0.1; // Gradually increase the size for ripples
                particle.life -= deltaTime / 16;
                particle.color = `rgba(0, 0, 255, ${particle.life / 50})`; // Fade out the color
            } else {
                particle.x += particle.vx * deltaTime / 16;
                particle.y += particle.vy * deltaTime / 16;
                particle.life -= deltaTime / 16;
            }
        });
    },

    renderParticles: function() {
        game.particles.forEach(particle => {
            game.ctx.fillStyle = particle.color;
            game.ctx.globalAlpha = particle.life / 50; // Match reduced lifetime
            game.ctx.beginPath();
            game.ctx.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2);
            game.ctx.fill();
            game.ctx.globalAlpha = 1; // Reset alpha
        });
    },

    createGrassEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(34, 139, 34, 0.5)', 'rgba(50, 205, 50, 0.5)'], // Subtle green colors
            count: 10,
            speed: 0.5,
            life: 30,
            size: 1,
            spread: Math.PI / 8 // Narrow spread
        });
    },

    createSandEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(194, 178, 128, 0.5)', 'rgba(210, 180, 140, 0.5)'], // Subtle sand colors
            count: 10,
            speed: 1,
            life: 30,
            size: 1,
            spread: Math.PI / 4 // Wider spread
        });
    },

    createMudEffect: function(x, y) {
        this.createParticles(x, y, {
            colors: ['rgba(139, 69, 19, 0.5)', 'rgba(160, 82, 45, 0.5)'], // Subtle brown colors
            count: 10,
            speed: 0.75,
            life: 30,
            size: 1,
            spread: Math.PI / 6 // Medium spread
        });
    }
};