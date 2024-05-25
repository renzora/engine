var Sprite = function (options) {
    var sprite = {
        x: options.x || 300,
        y: options.y || 150,
        width: 16,
        height: 26,
        scale: 1,
        speed: 90,
        currentFrame: 0,
        direction: 'S',
        animationSpeed: 0.2,
        frameCounter: 0,
        moving: false,
        stopping: false,
        directions: {},
        hairstyle: options.hairstyle || 3,
        outfit: options.outfit || 0,

        directionMap: {
            'S': 0, // Down
            'E': 6, // Right (Starts from column 7, 0-based index)
            'N': 12, // Up (Starts from column 13, 0-based index)
            'W': 6  // Left (Uses the same frames as Right but flipped)
        },

        draw: function() {
            let bodyImage = assets.load('character');
            let hairImage = assets.load('hair');
            let outfitImage = assets.load('outfit');
            if (!bodyImage || !hairImage || !outfitImage) return;

            let directionOffset = this.directionMap[this.direction] ?? 0;
            let frameColumn = directionOffset + (Math.floor(this.currentFrame) % 6);
            let sx = frameColumn * this.width;
            let sy = 0; // All frames are on the first row

            let tempCanvas = document.createElement('canvas');
            let tempCtx = tempCanvas.getContext('2d');
            tempCanvas.width = this.width;
            tempCanvas.height = this.height;

            // Draw the body
            tempCtx.drawImage(bodyImage, sx, sy, this.width, this.height, 0, 0, this.width, this.height);

            // Draw the hair
            let hairSy = this.hairstyle * this.height; // Row for the hairstyle
            tempCtx.drawImage(hairImage, sx, hairSy, this.width, this.height, 0, 0, this.width, this.height);

            // Draw the outfit
            let outfitSy = this.outfit * this.height; // Row for the outfit
            tempCtx.drawImage(outfitImage, sx, outfitSy, this.width, this.height, 0, 0, this.width, this.height);

            game.ctx.save();
            game.ctx.translate(this.x, this.y);

            // Flip horizontally for left direction
            if (this.direction === 'W') {
                game.ctx.scale(-this.scale, this.scale);
                game.ctx.translate(-this.width * this.scale, 0); // Adjust position after flipping
            } else {
                game.ctx.scale(this.scale, this.scale);
            }

            game.ctx.shadowBlur = 15;
            game.ctx.fillStyle = 'rgba(0, 0, 0, 0.15)';
            game.ctx.beginPath();
            game.ctx.ellipse(11, 20, this.width * this.scale * 0.22, this.height * this.scale * 0.15, 0, 0, 2 * Math.PI);
            game.ctx.fill();

            game.ctx.drawImage(tempCanvas, 0, 0, this.width, this.height, 0, 0, this.width, this.height);
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
                this.moving = false;
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
        
            dx = isNaN(dx) ? 0 : dx;
            dy = isNaN(dy) ? 0 : dy;
        
            this.vx = dx;
            this.vy = dy;
        
            let newX = this.x + this.vx;
            let newY = this.y + this.vy;
        
            newX = isNaN(newX) ? this.x : newX;
            newY = isNaN(newY) ? this.y : newY;
        
            // Collision check before applying new position
            if (!game.collision(newX, newY, this)) {
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

    return sprite;
};
