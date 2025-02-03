collision = {
    cornerBuffer: 16,

    pointInPolygon(px, py, polygon) {
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

    check(x, y, sprite, previousX, previousY) {
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
    
        const nudgeFactor = 0.1;
    
        for (const item of game.roomData.items) {
            const itemData = assets.use('objectData')[item.id]?.[0];
            if (!itemData?.w && itemData.w !== 0) continue;
            if (itemData.w === 1) continue;
    
            if (Array.isArray(itemData.w)) {
                const polygon = itemData.w.map(point => ({
                    x: point.x + item.x[0] * 16,
                    y: point.y + item.y[0] * 16
                }));
    
                if (this.pointInPolygon(centerX, centerY, polygon)) {
                    const slideVector = this.calculateSlideVector(previousX, previousY, x, y, polygon);
                    sprite.x = previousX + ((slideVector?.x || 0) * nudgeFactor);
                    sprite.y = previousY + ((slideVector?.y || 0) * nudgeFactor);
                    return { collisionDetected: true };
                }
    
                for (const point of pointsToCheck) {
                    if (this.pointInPolygon(point.px, point.py, polygon)) {
                        const slideVector = this.calculateSlideVector(previousX, previousY, x, y, polygon);
                        sprite.x = previousX + ((slideVector?.x || 0) * nudgeFactor);
                        sprite.y = previousY + ((slideVector?.y || 0) * nudgeFactor);
                        return { collisionDetected: true };
                    }
                }
            } else if (itemData.w === 0) {
                const slideVector = this.calculateSlideVector(previousX, previousY, x, y);
                sprite.x = previousX + ((slideVector?.x || 0) * nudgeFactor);
                sprite.y = previousY + ((slideVector?.y || 0) * nudgeFactor);
                return { collisionDetected: true };
            }
        }
    
        plugin.debug.tracker('collision.check');
        return { collisionDetected: false };
    },    

    calculateSlideVector(startX, startY, endX, endY, polygon) {
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

    checkPath(startX, startY, dirX, dirY, polygon) {
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
    }
}