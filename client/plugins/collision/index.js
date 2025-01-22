window[id] = {
    id: id,
    walkableGridCache: null,
    lastKnownPosition: null,
    cornerBuffer: 16,

    start: function() {

    },

    unmount: function() {

    },

    pointInPolygon: function(px, py, polygon) {
        let isInside = false;
        for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
            const xi = polygon[i].x, yi = polygon[i].y;
            const xj = polygon[j].x, yj = polygon[j].y;

            const intersect = ((yi > py) !== (yj > py)) && 
                              (px < (xj - xi) * (py - yi) / (yj - yi) + xi);
            if (intersect) isInside = !isInside;
        }
        return isInside;
    },

    check: function(x, y, sprite, previousX, previousY) {
        if (!game.roomData?.items) {
            this.lastKnownPosition = null;
            return { collisionDetected: false };
        }
    
        const a = sprite.width / 2;
        const b = sprite.height / 4;
        const centerX = x + a;
        const centerY = y + sprite.height * 0.75;
    
        const numPoints = 8;
        const pointsToCheck = [];
    
        for (let i = 0; i < numPoints; i++) {
            const angle = (i / numPoints) * 2 * Math.PI;
            const px = centerX + a * Math.cos(angle);
            const py = centerY + b * Math.sin(angle);
            pointsToCheck.push({ px, py });
        }
    
        for (const item of game.roomData.items) {
            const itemData = assets.use('objectData')[item.id]?.[0];
            if (!itemData?.w && itemData.w !== 0) continue;
    
            if (itemData.w === 1) continue;
    
            if (Array.isArray(itemData.w)) {
                const polygon = itemData.w.map(point => ({
                    x: point.x + item.x[0] * 16,
                    y: point.y + item.y[0] * 16
                }));

                const isCurrentlyInside = this.pointInPolygon(centerX, centerY, polygon);
                const wasInsidePreviously = this.lastKnownPosition?.insidePolygon;
                
                this.lastKnownPosition = {
                    x: centerX,
                    y: centerY,
                    insidePolygon: isCurrentlyInside,
                    polygonId: isCurrentlyInside ? item.id : null
                };

                if (wasInsidePreviously && !isCurrentlyInside && 
                    this.lastKnownPosition.polygonId === item.id) {
                    return { 
                        collisionDetected: true,
                        slideVector: this.calculateSlideVector(previousX, previousY, x, y, polygon)
                    };
                }

                if (!wasInsidePreviously && !isCurrentlyInside) {
                    for (const point of pointsToCheck) {
                        if (this.pointInPolygon(point.px, point.py, polygon)) {
                            return { 
                                collisionDetected: true,
                                slideVector: this.calculateSlideVector(previousX, previousY, x, y, polygon)
                            };
                        }
                    }
                }
            } else if (itemData.w === 0) {
                return { collisionDetected: true };
            }
        }
    
        if(plugin.exists('debug')) debug.tracker('collision.check');
        return { collisionDetected: false };
    },

    calculateSlideVector: function(startX, startY, endX, endY, polygon) {
        const direction = {
            x: endX - startX,
            y: endY - startY
        };
        
        const length = Math.hypot(direction.x, direction.y);
        if (length === 0) return null;
        
        direction.x /= length;
        direction.y /= length;
        
        const perpendicular = {
            x: -direction.y,
            y: direction.x
        };
        
        const leftPath = this.checkPath(startX, startY, perpendicular.x, perpendicular.y, polygon);
        const rightPath = this.checkPath(startX, startY, -perpendicular.x, -perpendicular.y, polygon);
        const path = leftPath.distance < rightPath.distance ? leftPath : rightPath; 

        return {
            x: path.vector.x * length,
            y: path.vector.y * length
        };
    },

    checkPath: function(startX, startY, dirX, dirY, polygon) {
        let clear = false;
        let distance = 16;
        let maxDistance = 64;
        
        while (!clear && distance <= maxDistance) {
            const testX = startX + dirX * distance;
            const testY = startY + dirY * distance;
            
            if (!this.pointInPolygon(testX, testY, polygon)) {
                clear = true;
                break;
            }
            
            distance += 16;
        }
        
        return {
            distance: distance,
            vector: { x: dirX, y: dirY }
        };
    },

    createWalkableGrid: function() {
        if (this.walkableGridCache) {
            return this.walkableGridCache;
        }

        const width = game.worldWidth / 16;
        const height = game.worldHeight / 16;
        const grid = Array.from({ length: width }, () => Array(height).fill(1));

        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.use('objectData')[item.id];
                if (!itemData || itemData.length === 0) return;

                let polygonPoints = itemData[0].w;

                if (Array.isArray(polygonPoints)) {
                    const polygon = polygonPoints.map(point => ({
                        x: point.x + item.x[0] * 16,
                        y: point.y + item.y[0] * 16
                    }));

                    for (let gridX = 0; gridX < width; gridX++) {
                        for (let gridY = 0; gridY < height; gridY++) {
                            const tileX = gridX * 16;
                            const tileY = gridY * 16;

                            const pointsToCheck = [
                                { px: tileX, py: tileY }, 
                                { px: tileX + 16, py: tileY },
                                { px: tileX, py: tileY + 16 },
                                { px: tileX + 16, py: tileY + 16 },
                                { px: tileX + 8, py: tileY + 8 }
                            ];

                            if (!this.lastKnownPosition?.insidePolygon || 
                                this.lastKnownPosition.polygonId !== item.id) {
                                for (const point of pointsToCheck) {
                                    if (this.pointInPolygon(point.px, point.py, polygon)) {
                                        grid[gridX][gridY] = 0;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                } else if (polygonPoints === 0) {
                    const itemX = item.x[0];
                    const itemY = item.y[0];
                    const itemWidth = itemData[0].a;
                    const itemHeight = itemData[0].b;

                    for (let gridX = itemX; gridX < itemX + itemWidth; gridX++) {
                        for (let gridY = itemY; gridY < itemY + itemHeight; gridY++) {
                            grid[gridX][gridY] = 0;
                        }
                    }
                }
            });
        }

        this.walkableGridCache = grid;
        if(plugin.exists('debug')) debug.tracker('collision.createWalkableGrid');

        return grid;
    },

    isTileWalkable: function(gridX, gridY) {
        const grid = this.createWalkableGrid();
        return grid[gridX]?.[gridY] === 1;
    },

    reset: function() {
        this.lastKnownPosition = null;
        this.walkableGridCache = null;
    }
}