pathfinding = {
    gridSize: 16,
    grid: null,

    start() {
        this.grid = this.createWalkableGrid(game.worldWidth, game.worldHeight);
    },

    cancelPathfinding(sprite) {
        if (!sprite) return;
        sprite.path = [];
        sprite.pathIndex = 0;
        sprite.isMovingToTarget = false;
    },

    walkToClickedTile(sprite, event, tileX, tileY) {
    
        if (!sprite || !this.grid) {
            return;
        }
        
        if (!sprite.speed) {
            sprite.speed = 85;
        }
        
        const targetX = tileX * 16 + 8;
        const targetY = tileY * 16 + 8;
    
        const path = this.findPath(
            sprite.x + sprite.width/2,
            sprite.y + sprite.height/2,
            targetX,
            targetY,
            this.grid
        );
    
        if (path.length > 0) {
            sprite.path = path;
            sprite.pathIndex = 0;
            sprite.isMovingToTarget = true;
        }
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
        sprite.directions = {};
        if (Math.abs(dx) > Math.abs(dy)) {
            if (dx > 0) {
                sprite.directions.right = true;
            } else {
                sprite.directions.left = true;
            }
        } else {
            if (dy > 0) {
                sprite.directions.down = true;
            } else {
                sprite.directions.up = true;
            }
        }
        if (Math.abs(dx) > 0.5 && Math.abs(dy) > 0.5) {
            if (dx > 0) {
                sprite.directions.right = true;
            } else {
                sprite.directions.left = true;
            }
            if (dy > 0) {
                sprite.directions.down = true;
            } else {
                sprite.directions.up = true;
            }
        }
        if (sprite.directions.up && sprite.directions.right) sprite.direction = 'NE';
        else if (sprite.directions.down && sprite.directions.right) sprite.direction = 'SE';
        else if (sprite.directions.down && sprite.directions.left) sprite.direction = 'SW';
        else if (sprite.directions.up && sprite.directions.left) sprite.direction = 'NW';
        else if (sprite.directions.up) sprite.direction = 'N';
        else if (sprite.directions.down) sprite.direction = 'S';
        else if (sprite.directions.left) sprite.direction = 'W';
        else if (sprite.directions.right) sprite.direction = 'E';
        if (!sprite.overrideAnimation) {
            if (sprite.speed < 50) {
                sprite.changeAnimation('speed_1');
            } else if (sprite.speed < 140) {
                sprite.changeAnimation('speed_2');
            } else if (sprite.speed <= 170) {
                sprite.changeAnimation('speed_3');
            } else {
                sprite.changeAnimation('speed_4');
            }
        }
    },    

    createWalkableGrid(roomWidth, roomHeight) {

        const cols = Math.ceil(roomWidth / this.gridSize);
        const rows = Math.ceil(roomHeight / this.gridSize);
        const grid = Array(cols).fill().map(() => Array(rows).fill(1));
        
        if (game.roomData?.items) {

            game.roomData.items.forEach(item => {
                const itemData = assets.use('objectData')[item.id]?.[0];
                if (!itemData?.w || itemData.w.length === 0) return;

                if (Array.isArray(itemData.w)) {
                    const baseX = Math.min(...item.x) * this.gridSize;
                    const baseY = Math.min(...item.y) * this.gridSize;
                    
                    const polygonPoints = itemData.w.map(point => ({
                        x: point.x + baseX,
                        y: point.y + baseY
                    }));

                    let unwalkableCells = 0;
                    for (let x = 0; x < cols; x++) {
                        for (let y = 0; y < rows; y++) {
                            const cellX = x * this.gridSize;
                            const cellY = y * this.gridSize;
                            
                            if (this.pointInPolygon(
                                cellX + this.gridSize/2,
                                cellY + this.gridSize/2,
                                polygonPoints
                            )) {
                                grid[x][y] = 0;
                                unwalkableCells++;
                            }
                        }
                    }
                }
            });
        }
        
        return grid;
    },

    pointInPolygon(x, y, polygon) {
        let inside = false;
        for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
            const xi = polygon[i].x, yi = polygon[i].y;
            const xj = polygon[j].x, yj = polygon[j].y;
            
            const intersect = ((yi > y) !== (yj > y))
                && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
            if (intersect) inside = !inside;
        }
        return inside;
    },

    findPath(startX, startY, endX, endY, grid) {

        const start = {
            x: Math.floor(startX / this.gridSize),
            y: Math.floor(startY / this.gridSize)
        };
        const end = {
            x: Math.floor(endX / this.gridSize),
            y: Math.floor(endY / this.gridSize)
        };

        if (start.x < 0 || start.y < 0 || end.x < 0 || end.y < 0 ||
            start.x >= grid.length || start.y >= grid[0].length ||
            end.x >= grid.length || end.y >= grid[0].length) {
            return [];
        }

        if (grid[start.x][start.y] === 0 || grid[end.x][end.y] === 0) {
            return [];
        }

        const openSet = new Set();
        const closedSet = new Set();
        const cameFrom = new Map();
        const gScore = new Map();
        const fScore = new Map();
        
        const startKey = `${start.x},${start.y}`;
        openSet.add(startKey);
        gScore.set(startKey, 0);
        fScore.set(startKey, this.heuristic(start, end));

        while (openSet.size > 0) {
            let current = null;
            let lowestF = Infinity;
            
            for (const key of openSet) {
                const f = fScore.get(key);
                if (f < lowestF) {
                    lowestF = f;
                    current = key;
                }
            }

            const [currentX, currentY] = current.split(',').map(Number);
            
            if (currentX === end.x && currentY === end.y) {
                const path = this.reconstructPath(cameFrom, current);
                return path;
            }

            openSet.delete(current);
            closedSet.add(current);

            const neighbors = this.getNeighbors(currentX, currentY, grid);
            
            for (const neighbor of neighbors) {
                const neighborKey = `${neighbor.x},${neighbor.y}`;
                
                if (closedSet.has(neighborKey)) continue;
                
                const tentativeG = gScore.get(current) + 1;
                
                if (!openSet.has(neighborKey)) {
                    openSet.add(neighborKey);
                } else if (tentativeG >= gScore.get(neighborKey)) {
                    continue;
                }

                cameFrom.set(neighborKey, current);
                gScore.set(neighborKey, tentativeG);
                fScore.set(neighborKey, tentativeG + this.heuristic(neighbor, end));
            }
        }
        return [];
    },

    getNeighbors(x, y, grid) {
        const neighbors = [];
        const directions = [
            {x: -1, y: 0}, {x: 1, y: 0}, {x: 0, y: -1}, {x: 0, y: 1},
            {x: -1, y: -1}, {x: -1, y: 1}, {x: 1, y: -1}, {x: 1, y: 1}
        ];

        for (const dir of directions) {
            const newX = x + dir.x;
            const newY = y + dir.y;

            if (newX >= 0 && newX < grid.length &&
                newY >= 0 && newY < grid[0].length &&
                grid[newX][newY] === 1) {
                neighbors.push({x: newX, y: newY});
            }
        }

        return neighbors;
    },

    heuristic(a, b) {
        return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
    },

    reconstructPath(cameFrom, current) {
        const path = [];
        while (cameFrom.has(current)) {
            const [x, y] = current.split(',').map(Number);
            path.unshift({
                x: x * this.gridSize + this.gridSize/2,
                y: y * this.gridSize + this.gridSize/2
            });
            current = cameFrom.get(current);
        }
        return path;
    }
};