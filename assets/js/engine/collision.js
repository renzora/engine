var collision = {
    walkableGridCache: null, // Cache for the walkable grid
    cacheTime: 0, // Timestamp to manage cache invalidation

    // Check if a point is inside a polygon
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

    // Check if a line (sprite's movement path) intersects a polygon
    lineIntersectsPolygon: function(x1, y1, x2, y2, polygon) {
        for (let i = 0; i < polygon.length; i++) {
            const p1 = polygon[i];
            const p2 = polygon[(i + 1) % polygon.length]; // Next point or wrap around

            if (this.lineIntersectsLine(x1, y1, x2, y2, p1.x, p1.y, p2.x, p2.y)) {
                return true; // Collision detected with the polygon's edge
            }
        }
        return false;
    },

    // Helper function to check if two line segments intersect
    lineIntersectsLine: function(x1, y1, x2, y2, x3, y3, x4, y4) {
        const denom = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
        if (denom === 0) return false; // Lines are parallel

        const ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denom;
        const ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / denom;

        return ua >= 0 && ua <= 1 && ub >= 0 && ub <= 1;
    },

    // Enhanced collision check that uses swept collision detection
    check: function(x, y, sprite) {
        if (!game.roomData?.items) {
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
            const itemData = assets.load('objectData')[item.id]?.[0];
            if (!itemData?.w && itemData.w !== 0) continue;

            if (Array.isArray(itemData.w)) {
                const polygon = itemData.w.map(point => ({
                    x: point.x + item.x[0] * 16,
                    y: point.y + item.y[0] * 16
                }));

                // Perform swept collision detection: Check the entire movement path for collision
                for (let i = 0; i < pointsToCheck.length - 1; i++) {
                    if (this.lineIntersectsPolygon(pointsToCheck[i].px, pointsToCheck[i].py, pointsToCheck[i + 1].px, pointsToCheck[i + 1].py, polygon)) {
                        return { collisionDetected: true };
                    }
                }

                // Also check if any point is inside the polygon (in case the sprite starts inside)
                for (const point of pointsToCheck) {
                    if (this.pointInPolygon(point.px, point.py, polygon)) {
                        return { collisionDetected: true };
                    }
                }
            } else {
                if (itemData.w === 0) {
                    return { collisionDetected: true };
                }
            }
        }

        return { collisionDetected: false };
    },

    // Optimized createWalkableGrid with caching
    createWalkableGrid: function() {
        const now = Date.now();
        if (this.walkableGridCache && now - this.cacheTime < 5000) {
            return this.walkableGridCache;
        }

        const width = game.worldWidth / 16;
        const height = game.worldHeight / 16;
        const grid = Array.from({ length: width }, () => Array(height).fill(1));

        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.load('objectData')[item.id];
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

                            for (const point of pointsToCheck) {
                                if (this.pointInPolygon(point.px, point.py, polygon)) {
                                    grid[gridX][gridY] = 0;
                                    break;
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
        this.cacheTime = now;
        return grid;
    },

    isTileWalkable: function(gridX, gridY) {
        const grid = this.createWalkableGrid();
        return grid[gridX]?.[gridY] === 1;
    }
};
