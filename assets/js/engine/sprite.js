var sprite = {
    create: function (options) {
        return {
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
            hairstyle: options.hairstyle || 1,
            outfit: options.outfit || 1,
            facialHair: options.facialHair || 0,
            hat: options.hat || 0,
            glasses: options.glasses || 0,

            directionMap: {
                'S': 0,
                'E': 6,
                'N': 12,
                'W': 6
            },

            draw: function () {
                let bodyImage = assets.load('character');
                let hairImage = assets.load('hair');
                let outfitImage = assets.load('outfit');
                let facialHairImage = assets.load('facial');
                let hatImage = assets.load('hats');
                let glassesImage = assets.load('glasses');

                if (!bodyImage || !hairImage || !outfitImage || !facialHairImage || !hatImage || !glassesImage) return;

                let directionOffset = this.directionMap[this.direction] ?? 0;
                let frameColumn = directionOffset + (Math.floor(this.currentFrame) % 6);
                let sx = frameColumn * this.width;
                let sy = 0;

                let tempCanvas = document.createElement('canvas');
                let tempCtx = tempCanvas.getContext('2d');
                tempCanvas.width = this.width;
                tempCanvas.height = this.height;

                tempCtx.drawImage(bodyImage, sx, sy, this.width, this.height, 0, 0, this.width, this.height);

                if (this.hairstyle !== 0) {
                    let hairSy = (this.hairstyle - 1) * 17;
                    tempCtx.drawImage(hairImage, sx, hairSy, this.width, 17, 0, 0, this.width, 17);
                }

                if (this.outfit !== 0) {
                    let outfitSy = (this.outfit - 1) * this.height;
                    tempCtx.drawImage(outfitImage, sx, outfitSy, this.width, this.height, 0, 0, this.width, this.height);
                }

                if (this.facialHair !== 0) {
                    let facialHairSy = (this.facialHair - 1) * 8;
                    tempCtx.drawImage(facialHairImage, sx, facialHairSy, this.width, 8, 0, 12, this.width, 8);
                }

                if (this.glasses !== 0) {
                    let glassesSy = (this.glasses - 1) * 16;
                    tempCtx.drawImage(glassesImage, sx, glassesSy, this.width, 16, 0, 6, this.width, 16);
                }

                if (this.hat !== 0) {
                    let hatSy = (this.hat - 1) * 16;
                    tempCtx.drawImage(hatImage, sx, hatSy, this.width, 16, 0, 0, this.width, 16);
                }

                game.ctx.save();
                game.ctx.translate(this.x, this.y);

                if (this.direction === 'W') {
                    game.ctx.scale(-this.scale, this.scale);
                    game.ctx.translate(-this.width * this.scale, 0);
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

            addDirection: function (direction) {
                this.directions[direction] = true;
                this.updateDirection();
                this.moving = true;
                this.stopping = false;
            },

            removeDirection: function (direction) {
                delete this.directions[direction];
                this.updateDirection();
                if (Object.keys(this.directions).length === 0) {
                    this.stopping = true;
                    this.moving = false;
                }
            },

            updateDirection: function () {
                if (this.directions['up']) this.direction = 'N';
                if (this.directions['down']) this.direction = 'S';
                if (this.directions['left']) this.direction = 'W';
                if (this.directions['right']) this.direction = 'E';
                if (this.directions['up'] && this.directions['right']) this.direction = 'N';
                if (this.directions['down'] && this.directions['right']) this.direction = 'S';
                if (this.directions['down'] && this.directions['left']) this.direction = 'W';
                if (this.directions['up'] && this.directions['left']) this.direction = 'N';
            },

            animate: function () {
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
                        this.currentFrame = 0;
                    } else if (this.frameCounter >= 1) {
                        if (this.currentFrame < 5) {
                            this.currentFrame++;
                        } else {
                            this.currentFrame = 0;
                        }
                        this.frameCounter = 0;
                    }
                } else if (this.stopping && this.frameCounter >= 1) {
                    if (this.currentFrame < 5) {
                        this.currentFrame++;
                    } else {
                        this.stopping = false;
                    }
                    this.frameCounter = 0;
                }
            },

            update: function () {
                let deltatime = game.deltaTime / 1000;

                let dx = 0;
                let dy = 0;

                if (this.directions['right']) dx += this.speed * deltatime;
                if (this.directions['left']) dx -= this.speed * deltatime;
                if (this.directions['down']) dy += this.speed * deltatime;
                if (this.directions['up']) dy -= this.speed * deltatime;

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

                if (!game.collision(newX, newY, this)) {
                    this.x = newX;
                    this.y = newY;
                }

                this.x = Math.max(0, Math.min(this.x, game.worldWidth - this.width * this.scale));
                this.y = Math.max(0, Math.min(this.y, game.worldHeight - this.height * this.scale));

                this.animate();

                if (dx === 0 && dy === 0) {
                    this.movementFrameCounter = 0;
                }
            }
        };
    },

    createSprite: function (options) {
        let newSprite = this.create(options);
        game.sprites[options.id] = newSprite;
        return newSprite;
    },

    moveSprite: function (sprite, direction, duration) {
        if (sprite) {
            sprite.addDirection(direction);
            setTimeout(() => {
                sprite.removeDirection(direction);
            }, duration * 1000);
        }
    },

    npcMovement: function (sprite, area) {
        setInterval(() => {
            const directions = ['up', 'down', 'left', 'right'];
            const randomDirection = directions[Math.floor(Math.random() * directions.length)];
            const randomDuration = Math.random() * 2 + 1;

            let newX = sprite.x;
            let newY = sprite.y;

            switch (randomDirection) {
                case 'up':
                    newY -= sprite.speed * randomDuration;
                    break;
                case 'down':
                    newY += sprite.speed * randomDuration;
                    break;
                case 'left':
                    newX -= sprite.speed * randomDuration;
                    break;
                case 'right':
                    newX += sprite.speed * randomDuration;
                    break;
            }

            if (newX >= area.x && newX <= area.x + area.width - sprite.width &&
                newY >= area.y && newY <= area.y + area.height - sprite.height) {
                sprite.addDirection(randomDirection);
                setTimeout(() => {
                    sprite.removeDirection(randomDirection);
                }, randomDuration * 1000);
            }
        }, 500);
    }
};