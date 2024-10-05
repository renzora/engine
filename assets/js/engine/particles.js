const particles = {
    activeEffects: {},
    createParticles: function(x, y, options, effectId) {
        const particleCount = options.count || 20;
        const baseSpeed = options.speed || 2;
        const baseAngleDeg = options.angle || 0;
        const baseAngle = baseAngleDeg * (Math.PI / 180);
        const spread = options.spread || 360;
        const spreadRad = spread * (Math.PI / 180);
        const colors = options.colors || ['rgba(255, 0, 0, 1)'];
        const repeat = options.repeat || false;
        const gravity = options.gravity || 0; // New: Gravity effect
        const sway = options.sway || 0; // New: Sway effect
        const twinkleSpeed = options.twinkle || 0; // New: Twinkle effect

        for (let i = 0; i < particleCount; i++) {
            const randomSpread = (Math.random() - 0.5) * spreadRad;
            const angle = baseAngle + randomSpread;
            const speed = baseSpeed * (0.5 + Math.random() * 0.5);
            const color = colors[Math.floor(Math.random() * colors.length)];

            const particle = {
                x: x,
                y: y,
                initialX: x,
                initialY: y,
                vx: Math.cos(angle) * speed,
                vy: Math.sin(angle) * speed,
                life: options.life || 50,
                maxLife: options.life || 50,
                color: color,
                size: options.size || 2,
                type: options.type || 'default',
                repeat: repeat,
                glow: options.glow || 0,
                initialOpacity: options.opacity || 1,
                blur: options.blur || 0,
                shape: options.shape || 'circle',
                gravity: gravity, // Apply gravity
                sway: sway, // Apply sway
                twinkleSpeed: twinkleSpeed, // Apply twinkle
                initialSize: options.size || 2, // Keep initial size for twinkle
                opacity: options.opacity || 1 // Keep initial opacity for fade
            };

            if (!this.activeEffects[effectId]) {
                this.activeEffects[effectId] = [];
            }
            this.activeEffects[effectId].push(particle);
            game.particles.push(particle);
        }
    },

    updateParticles: function(deltaTime) {
        game.particles = game.particles.filter(particle => particle.life > 0);

        game.particles.forEach(particle => {
            if (particle.type === 'ripple') {
                particle.size += 0.1;
                particle.life -= deltaTime / 16;
                particle.color = `rgba(0, 0, 255, ${particle.life / 50})`;
            } else {
                particle.x += particle.vx * deltaTime / 16;
                particle.y += particle.vy * deltaTime / 16 + particle.gravity; // Apply gravity
                particle.vx += Math.sin(particle.y * 0.01) * particle.sway; // Apply sway
                particle.life -= deltaTime / 16;

                const fadeStart = 0.2;
                const lifeFraction = particle.life / particle.maxLife;
                if (lifeFraction < fadeStart) {
                    particle.opacity = particle.initialOpacity * (lifeFraction / fadeStart);
                } else {
                    particle.opacity = particle.initialOpacity;
                }

                if (particle.twinkleSpeed > 0) { // Apply twinkle effect
                    particle.size += particle.twinkleSpeed;
                    if (particle.size > particle.initialSize * 1.5 || particle.size < particle.initialSize * 0.5) {
                        particle.twinkleSpeed = -particle.twinkleSpeed;
                    }
                }
            }

            if (particle.life <= 0 && particle.repeat) {
                particle.life = particle.maxLife;
                particle.x = particle.initialX;
                particle.y = particle.initialY;
            }
        });
    },

    renderParticles: function() {
        Object.values(this.activeEffects).forEach(effectParticles => {
            effectParticles.forEach(particle => {
                const ctx = game.ctx;

                ctx.save();
                ctx.globalAlpha = particle.opacity;

                if (particle.glow > 0) {
                    ctx.shadowColor = particle.color;
                    ctx.shadowBlur = particle.glow;
                }

                if (particle.blur > 0) {
                    ctx.filter = `blur(${particle.blur}px)`;
                }

                ctx.fillStyle = particle.color;
                ctx.beginPath();

                switch (particle.shape) {
                    case 'square':
                        ctx.fillRect(particle.x, particle.y, particle.size, particle.size);
                        break;
                    case 'triangle':
                        ctx.moveTo(particle.x, particle.y - particle.size);
                        ctx.lineTo(particle.x - particle.size, particle.y + particle.size);
                        ctx.lineTo(particle.x + particle.size, particle.y + particle.size);
                        ctx.closePath();
                        break;
                    default:
                        ctx.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2);
                        break;
                }

                ctx.fill();
                ctx.restore();
            });
        });
    },

    removeItemEffects: function(item) {
        const effectIdPrefix = `${item.id}_`;
        for (const effectId in this.activeEffects) {
            if (effectId.startsWith(effectIdPrefix)) {
                delete this.activeEffects[effectId];
            }
        }
    }
};
