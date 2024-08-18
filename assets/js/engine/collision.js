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

    // Check if the sprite's position collides with any non-walkable objects
    check: function(x, y, sprite) {
        // Return early if roomData or items are not defined
        if (!game.roomData?.items) {
            //console.error("roomData or items is not defined.");
            return { collisionDetected: false };
        }

        // Define the points to check around the sprite (points along the oval's perimeter)
        const pointsToCheck = [];
        const a = sprite.width / 2; // Semi-major axis (horizontal radius)
        const b = sprite.height / 4; // Semi-minor axis (vertical radius, half of the bottom half of the sprite)
        const centerX = x + a;
        const centerY = y + sprite.height * 0.75; // Start halfway down the sprite

        const numPoints = 8; // Reduced number of points to check around the oval for optimization

        for (let i = 0; i < numPoints; i++) {
            const angle = (i / numPoints) * 2 * Math.PI;
            const px = centerX + a * Math.cos(angle);
            const py = centerY + b * Math.sin(angle);
            pointsToCheck.push({ px, py });
        }

        // Iterate through all the items in the room
        for (const item of game.roomData.items) {
            const itemData = assets.load('objectData')[item.id]?.[0];
            if (!itemData?.w) continue;

            // Get the absolute positions of the polygon points
            const polygon = itemData.w.map(point => ({
                x: point.x + item.x[0] * 16,
                y: point.y + item.y[0] * 16
            }));

            // Check each point around the oval
            for (const point of pointsToCheck) {
                if (this.pointInPolygon(point.px, point.py, polygon)) {
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
            // Reuse the cached grid if it's still valid
            return this.walkableGridCache;
        }

        const width = game.worldWidth / 16;
        const height = game.worldHeight / 16;
        const grid = Array.from({ length: width }, () => Array(height).fill(1)); // Initialize all tiles as walkable

        if (game.roomData && game.roomData.items) {
            game.roomData.items.forEach(item => {
                const itemData = assets.load('objectData')[item.id];
                if (!itemData || itemData.length === 0) return;

                const polygonPoints = itemData[0].w;
                if (!polygonPoints) return;

                // Calculate the polygon's absolute position
                const polygon = polygonPoints.map(point => ({
                    x: point.x + item.x[0] * 16,
                    y: point.y + item.y[0] * 16
                }));

                // Iterate over each tile in the grid
                for (let gridX = 0; gridX < width; gridX++) {
                    for (let gridY = 0; gridY < height; gridY++) {
                        const tileX = gridX * 16;
                        const tileY = gridY * 16;

                        // Define points around the tile's perimeter to check for collision
                        const pointsToCheck = [
                            { px: tileX, py: tileY },                  // Top-left
                            { px: tileX + 16, py: tileY },            // Top-right
                            { px: tileX, py: tileY + 16 },            // Bottom-left
                            { px: tileX + 16, py: tileY + 16 },       // Bottom-right
                            { px: tileX + 8, py: tileY + 8 }          // Center
                        ];

                        // Check if any of these points are inside the polygon
                        for (const point of pointsToCheck) {
                            if (this.pointInPolygon(point.px, point.py, polygon)) {
                                grid[gridX][gridY] = 0; // Mark as non-walkable
                                break; // No need to check further points for this tile
                            }
                        }
                    }
                }
            });
        }

        this.walkableGridCache = grid; // Cache the computed grid
        this.cacheTime = now; // Update the cache timestamp
        return grid;
    },

    // Check if a tile is walkable based on the walkable grid
    isTileWalkable: function(gridX, gridY) {
        const grid = this.createWalkableGrid();
        if (!grid[gridX] || grid[gridX][gridY] !== 1) {
            return false; // Tile is not walkable based on grid data
        }

        // Check for collision using the polygon boundary
        const tileCenterX = gridX * 16 + 8; // Center of the tile in pixels
        const tileCenterY = gridY * 16 + 8; // Center of the tile in pixels
        const pointsToCheck = [{ px: tileCenterX, py: tileCenterY }];

        for (const item of game.roomData.items) {
            const itemData = assets.load('objectData')[item.id]?.[0];
            if (!itemData?.w) continue;

            // Get the absolute positions of the polygon points
            const polygon = itemData.w.map(point => ({
                x: point.x + item.x[0] * 16,
                y: point.y + item.y[0] * 16
            }));

            // Check each point around the tile's center
            for (const point of pointsToCheck) {
                if (this.pointInPolygon(point.px, point.py, polygon)) {
                    return false; // Collision detected, tile is not walkable
                }
            }
        }

        return true; // Tile is walkable
    }
};
