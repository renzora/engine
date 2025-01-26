pathfinding = {
    start() {
        console.log("[pathfinding] => start");
    },

    unmount() {
        console.log("[pathfinding] => unmount");
    },

    calculatePath(sprite, startX, startY, endX, endY) {
        console.log(`[pathfinding.calculatePath] => sprite=${sprite.id}, start=(${startX}, ${startY}), end=(${endX}, ${endY})`);

        if (!plugin.exists('collision')) {
            console.log("[pathfinding.calculatePath] => No collision plugin, returning fallback path...");
            return [{ x: endX, y: endY, alpha: 1 }];
        }

        console.log("[pathfinding.calculatePath] => Collision plugin found, using A*...");

        const grid = collision.walkableGridCache;
        if (!grid) {
            console.log("[pathfinding.calculatePath] => No walkableGridCache found, returning empty path.");
            return [];
        }

        const graph = new Graph(grid, { diagonal: true });

        if (
            startX < 0 || startY < 0 ||
            startX >= grid.length ||
            startY >= grid[0].length ||
            endX < 0   || endY < 0 ||
            endX >= grid.length ||
            endY >= grid[0].length
        ) {
            console.log(`[pathfinding.calculatePath] => Out of grid bounds: start=(${startX},${startY}), end=(${endX},${endY}).`);
            return [];
        }

        if (grid[startX][startY] === 0 || grid[endX][endY] === 0) {
            console.log("[pathfinding.calculatePath] => Start or end tile not walkable. No path.");
            return [];
        }

        console.log("[pathfinding.calculatePath] => Running A*...");
        const result = astar.search(
            graph,
            graph.grid[startX][startY],
            graph.grid[endX][endY]
        );

        if (!result || result.length === 0) {
            console.log("[pathfinding.calculatePath] => No path found.");
            return [];
        }

        const finalPath = result.map(node => ({ x: node.x, y: node.y, alpha: 1 }));
        console.log("[pathfinding.calculatePath] => Final path:", finalPath);

        return finalPath;
    },

    walkToClickedTile(sprite, e, tileX, tileY) {
        // 1) Check if the click target is inside an element with class="window"
        if (e.target.closest('.window')) {
            console.log("[pathfinding.walkToClickedTile] => Clicked on a .window; ignoring walk.");
            return;
        }

        console.log(`[pathfinding.walkToClickedTile] => sprite=${sprite.id}, tileX=${tileX}, tileY=${tileY}`);

        const boundary = sprite.boundary;
        if (boundary) {
            console.log(`[pathfinding.walkToClickedTile] => Checking boundary=(${boundary.x}, ${boundary.y})`);
            if (tileX > boundary.x || tileY > boundary.y) {
                console.log("[pathfinding.walkToClickedTile] => Target out of boundary, stopping.");
                return;
            }
        }

        const currentX = Math.floor(sprite.x / 16);
        const currentY = Math.floor(sprite.y / 16);

        sprite.path = this.calculatePath(sprite, currentX, currentY, tileX, tileY);
        console.log("[pathfinding.walkToClickedTile] => Path returned:", sprite.path);

        if (sprite.path && sprite.path.length > 0) {
            sprite.pathIndex = 0;
            sprite.isMovingToTarget = true;
            sprite.moving = true;
            sprite.stopping = false;
            // plugin.audio.playAudio("footsteps1", assets.use('footsteps1'), 'sfx', true);
            sprite.changeAnimation('speed_1');
        } else {
            console.log("[pathfinding.walkToClickedTile] => No valid path found, not moving sprite.");
        }
    },

    moveAlongPath(sprite) {
        if (!sprite.path || sprite.pathIndex >= sprite.path.length) {
            sprite.isMovingToTarget = false;
            sprite.moving = false;
            sprite.stopping = true;
            sprite.currentFrame = 0;
            sprite.path = [];
            plugin.audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
            return;
        }

        const nextStep = sprite.path[sprite.pathIndex];
        const targetX = nextStep.x * 16;
        const targetY = nextStep.y * 16;
        const deltaX = targetX - sprite.x;
        const deltaY = targetY - sprite.y;
        const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
        const walkingPaceFactor = 0.6;
        sprite.speed = Math.max(10, sprite.topSpeed * walkingPaceFactor);

        if (distance < sprite.speed * (game.deltaTime / 1000)) {
            sprite.x = targetX;
            sprite.y = targetY;
            sprite.pathIndex++;

            if (sprite.pathIndex > 1) {
                sprite.path.shift(); 
                sprite.pathIndex--;
            }
        } else {
            const angle = Math.atan2(deltaY, deltaX);
            sprite.x += Math.cos(angle) * sprite.speed * (game.deltaTime / 1000);
            sprite.y += Math.sin(angle) * sprite.speed * (game.deltaTime / 1000);

            if (Math.abs(deltaX) > Math.abs(deltaY)) {
                sprite.direction = deltaX > 0 ? 'E' : 'W';
            } else {
                sprite.direction = deltaY > 0 ? 'S' : 'N';
            }
            if (Math.abs(deltaX) > 0 && Math.abs(deltaY) > 0) {
                if (deltaX > 0 && deltaY > 0) sprite.direction = 'SE';
                else if (deltaX > 0 && deltaY < 0) sprite.direction = 'NE';
                else if (deltaX < 0 && deltaY > 0) sprite.direction = 'SW';
                else if (deltaX < 0 && deltaY < 0) sprite.direction = 'NW';
            }
        }
    },

    cancelPathfinding(sprite) {
        if (sprite && sprite.isMovingToTarget) {
            console.log(`[pathfinding.cancelPathfinding] => Canceling path for sprite=${sprite.id}`);
            sprite.isMovingToTarget = false;
            sprite.path = [];
            sprite.moving = false;
            plugin.audio.stopLoopingAudio('footsteps1', 'sfx', 0.5);
        }
    }
};
