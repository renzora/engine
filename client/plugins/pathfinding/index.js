pathfinding = {
    worker: new Worker('plugins/pathfinding/worker.js'),
    pendingPaths: new Map(),
    gridSize: 16,
    grid: null,

    start() {
        this.worker.onmessage = (e) => {
            const { requestId, path, type, grid } = e.data;
            
            if (type === 'gridCreated') {
                this.grid = grid;
                return;
            }

            const pending = this.pendingPaths.get(requestId);
            if (pending) {
                const { sprite } = pending;
                if (path.length > 0) {
                    sprite.path = path;
                    sprite.pathIndex = 0;
                    sprite.isMovingToTarget = true;
                }
                this.pendingPaths.delete(requestId);
            }
        };

        // Initialize grid
        this.worker.postMessage({
            type: 'initGrid',
            worldWidth: game.worldWidth,
            worldHeight: game.worldHeight,
            roomData: game.roomData,
            gridSize: this.gridSize,
            objectData: assets.use('objectData')
        });
    },

    onRender() {
        this.renderPathfinderLine();
    },

    cancelPathfinding(sprite) {
        if (!sprite) return;
        sprite.path = [];
        sprite.pathIndex = 0;
        sprite.isMovingToTarget = false;
    },

    walkToClickedTile(sprite, event, tileX, tileY) {
        if (!sprite) return;
        
        if (!sprite.speed) {
            sprite.speed = 85;
        }
        
        const requestId = Date.now() + Math.random();
        const targetX = tileX * 16 + 8;
        const targetY = tileY * 16 + 8;
        
        this.pendingPaths.set(requestId, { sprite, targetX, targetY });
        
        this.worker.postMessage({
            type: 'findPath',
            requestId,
            startX: sprite.x + sprite.width/2,
            startY: sprite.y + sprite.height/2,
            endX: targetX,
            endY: targetY
        });
    },

    moveAlongPath(sprite) {
        if (!sprite.path || sprite.path.length === 0) {
            if (sprite.isMovingToTarget) {
                sprite.moving = false;
                sprite.stopping = true;
                if (!sprite.overrideAnimation) {
                    sprite.changeAnimation('idle');
                }
            }
            sprite.isMovingToTarget = false;
            return;
        }

        sprite.moving = true;
        sprite.stopping = false;
        const target = sprite.path[sprite.pathIndex];
        const dx = target.x - (sprite.x + sprite.width/2);
        const dy = target.y - (sprite.y + sprite.height/2);
        const dist = Math.sqrt(dx * dx + dy * dy);

        if (dist < 2) {
            sprite.pathIndex++;
            if (sprite.pathIndex >= sprite.path.length) {
                sprite.isMovingToTarget = false;
                sprite.path = [];
                sprite.directions = {};
                sprite.moving = false;
                sprite.stopping = true;
                if (!sprite.overrideAnimation) {
                    sprite.changeAnimation('idle');
                }
                return;
            }
        }

        const speed = sprite.speed * (game.deltaTime / 1000);
        const moveX = (dx / dist) * speed;
        const moveY = (dy / dist) * speed;
        const newX = sprite.x + moveX;
        const newY = sprite.y + moveY;

        if (plugin.exists('collision')) {
            const collisionResult = collision.check(newX, newY, sprite, sprite.x, sprite.y);
            if (!collisionResult.collisionDetected) {
                sprite.x = newX;
                sprite.y = newY;
            } else if (collisionResult.slideVector) {
                sprite.x += collisionResult.slideVector.x;
                sprite.y += collisionResult.slideVector.y;
            }
        } else {
            sprite.x = newX;
            sprite.y = newY;
        }

        this.updateSpriteDirection(sprite, dx, dy);
        this.updateSpriteAnimation(sprite);
    },

    updateSpriteDirection(sprite, dx, dy) {
        sprite.directions = {};
        
        if (Math.abs(dx) > Math.abs(dy)) {
            sprite.directions[dx > 0 ? 'right' : 'left'] = true;
        } else {
            sprite.directions[dy > 0 ? 'down' : 'up'] = true;
        }

        if (Math.abs(dx) > 0.5 && Math.abs(dy) > 0.5) {
            sprite.directions[dx > 0 ? 'right' : 'left'] = true;
            sprite.directions[dy > 0 ? 'down' : 'up'] = true;
        }

        if (sprite.directions.up && sprite.directions.right) sprite.direction = 'NE';
        else if (sprite.directions.down && sprite.directions.right) sprite.direction = 'SE';
        else if (sprite.directions.down && sprite.directions.left) sprite.direction = 'SW';
        else if (sprite.directions.up && sprite.directions.left) sprite.direction = 'NW';
        else if (sprite.directions.up) sprite.direction = 'N';
        else if (sprite.directions.down) sprite.direction = 'S';
        else if (sprite.directions.left) sprite.direction = 'W';
        else if (sprite.directions.right) sprite.direction = 'E';
    },

    updateSpriteAnimation(sprite) {
        if (!sprite.overrideAnimation) {
            if (sprite.speed < 50) sprite.changeAnimation('speed_1');
            else if (sprite.speed < 140) sprite.changeAnimation('speed_2');
            else if (sprite.speed <= 170) sprite.changeAnimation('speed_3');
            else sprite.changeAnimation('speed_4');
        }
    },

    renderPathfinderLine() {
        if (!game.mainSprite || !game.mainSprite.path || game.mainSprite.path.length === 0) return;
        
        const ctx = game.ctx;
        const last = game.mainSprite.path[game.mainSprite.path.length - 1];
        const el = Date.now() % 1000;
        const p1 = (el % 1000) / 1000;
        const p2 = ((el + 500) % 1000) / 1000;
        const r1 = 3 + p1 * 10;
        const r2 = 3 + p2 * 12;
        const o1 = 0.4 - p1 * 0.4;
        const o2 = 0.4 - p2 * 0.4;
        const ps = 2;
        const r1p = Math.floor(r1 / ps) * ps;
    
        for (let y = -r1p; y <= r1p; y += ps) {
            for (let x = -r1p; x <= r1p; x += ps) {
                const d = Math.sqrt(x * x + y * y);
                if (d >= r1p - ps && d <= r1p) {
                    ctx.fillStyle = `rgba(0,102,255,${o1})`;
                    ctx.fillRect(last.x * 16 + 8 + x - ps / 2, last.y * 16 + 8 + y - ps / 2, ps, ps);
                }
            }
        }
    
        const r2p = Math.floor(r2 / ps) * ps;
        for (let y = -r2p; y <= r2p; y += ps) {
            for (let x = -r2p; x <= r2p; x += ps) {
                const d = Math.sqrt(x * x + y * y);
                if (d >= r2p - ps && d <= r2p) {
                    ctx.fillStyle = `rgba(0,102,255,${o2})`;
                    ctx.fillRect(last.x * 16 + 8 + x - ps / 2, last.y * 16 + 8 + y - ps / 2, ps, ps);
                }
            }
        }
    }
};