var sprite = {
    x: 50,
    y: 150,
    width: 16,  // Sprite width
    height: 32, // Sprite height
    scale: 1,
    speed: 90,
    currentFrame: 0,
    direction: 'S',
    animationSpeed: 0.2,
    frameCounter: 0,
    moving: false,
    stopping: false,
    directionMap: {
        'S': 0, // Down
        'W': 3, // Left
        'E': 1, // Right
        'N': 2  // Up
    },
    directions: {},

    draw: function() {
        let image = assets.load('sprite');
        if (!image) return;

        let directionRow = this.directionMap[this.direction] ?? 0;
        let frameColumn = Math.floor(this.currentFrame) % 6;
        let sx = frameColumn * this.width;
        let sy = directionRow * this.height;
        let shadowWidth = this.width * this.scale * 0.22;
        let shadowHeight = this.height * this.scale * 0.15;

        game.ctx.save();
        game.ctx.translate(this.x, this.y);
        game.ctx.shadowBlur = 15;
        game.ctx.fillStyle = 'rgba(0, 0, 0, 0.15)';
        game.ctx.beginPath();
        game.ctx.ellipse(11, 20, shadowWidth, shadowHeight, 0, 0, 2 * Math.PI);
        game.ctx.fill();
        game.ctx.scale(this.scale, this.scale);
        game.ctx.drawImage(image, sx, sy, this.width, this.height, 0, 0, this.width, this.height);
        game.ctx.restore();
    },

    addDirection: function(direction) {
        this.directions[direction] = true;
        this.updateDirection();
        this.moving = true;
        this.stopping = false;
    },

    removeDirection: function(direction) {
        delete this.directions[direction];
        this.updateDirection();
        if (Object.keys(this.directions).length === 0) {
            this.stopping = true;
        }
    },

    updateDirection: function() {
        if (this.directions['up']) this.direction = 'N';
        if (this.directions['down']) this.direction = 'S';
        if (this.directions['left']) this.direction = 'W';
        if (this.directions['right']) this.direction = 'E';
        if (this.directions['up'] && this.directions['right']) this.direction = 'N';
        if (this.directions['down'] && this.directions['right']) this.direction = 'S';
        if (this.directions['down'] && this.directions['left']) this.direction = 'W';
        if (this.directions['up'] && this.directions['left']) this.direction = 'N';
    },

    animate: function() {
        if (this.moving) {
            this.frameCounter += this.animationSpeed;
            if (this.stopping) {
                if (this.currentFrame < 3 || this.currentFrame > 5) {
                    this.currentFrame = 3;
                } else if (this.frameCounter >= 1) {
                    this.currentFrame = Math.min(this.currentFrame + 1, 5);
                    this.frameCounter = 0;
                }
            } else if (this.currentFrame < 0 || this.currentFrame >= 6) {
                this.currentFrame = 0; // Start loop animation
            } else if (this.frameCounter >= 1) {
                if (this.currentFrame < 5) {
                    this.currentFrame++;
                } else {
                    this.currentFrame = 0; // Loop back to the start of the loop animation
                }
                this.frameCounter = 0;
            }
        } else if (this.stopping && this.frameCounter >= 1) {
            if (this.currentFrame < 5) {
                this.currentFrame++;
            } else {
                this.stopping = false; // Stop animation completed
            }
            this.frameCounter = 0;
        }
    },

    update: function() {
        let deltatime = game.deltaTime / 1000;

        let dx = 0;
        let dy = 0;

        if (this.directions['right']) dx += this.speed * deltatime;
        if (this.directions['left']) dx -= this.speed * deltatime;
        if (this.directions['down']) dy += this.speed * deltatime;
        if (this.directions['up']) dy -= this.speed * deltatime;

        // Normalize diagonal speed
        if (dx !== 0 && dy !== 0) {
            const norm = Math.sqrt(dx * dx + dy * dy);
            dx = (dx / norm) * this.speed * deltatime;
            dy = (dy / norm) * this.speed * deltatime;
        }

        this.vx = dx;
        this.vy = dy;

        let newX = this.x + this.vx;
        let newY = this.y + this.vy;

        // Collision check before applying new position
        if (!game.collision(newX, newY)) {
            this.x = newX;
            this.y = newY;
        }

        // Ensure sprite stays within world bounds
        this.x = Math.max(0, Math.min(this.x, game.worldWidth - this.width * this.scale));
        this.y = Math.max(0, Math.min(this.y, game.worldHeight - this.height * this.scale));

        this.animate();

        if (dx === 0 && dy === 0) {
            this.movementFrameCounter = 0;
        }
    }
};
