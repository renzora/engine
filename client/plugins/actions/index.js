actions = {
    audioCooldown: 0.5,
    lastPlayedTimesByType: {},
    throttleInterval: 2000,
    lastExecutionTime: 0,
    proximityThreshold: 42,
    signalBus: new Map(),
    nodeLinks: new Map(),

    start() {
        this.initNodeSystem();
    },

    onRender() {
        this.checkForNearbyItems();
    },

    initNodeSystem() {
        this.nodeValues = new Map();
        this.nodeStates = new Map();
        this.executionCache = new Map();
    },

    broadcastSignal(signalName, value) {
        this.signalBus.set(signalName, value);
    },
    
    getSignal(signalName) {
        return this.signalBus.get(signalName);
    },

    linkNodes(sourceNodeId, targetNodeId, signal) {
        const linkKey = `${sourceNodeId}->${targetNodeId}`;
        this.nodeLinks.set(linkKey, signal);
    },

    executeNodeGraph(item, startNodeId, hasOverlap) {
        if (!game.roomData?.nodeData?.[`item_${item.layer_id}`]) {
            return;
        }
        
        const nodeData = game.roomData.nodeData[`item_${item.layer_id}`];
        this.executionCache.clear();
        this.nodeValues.set(item.layer_id, new Map());
        
        const getNodeValue = (nodeId, output = 'output') => {
            const values = this.nodeValues.get(item.layer_id);
            return values?.get(`${nodeId}.${output}`);
        };
    
        const setNodeValue = (nodeId, value, output = 'output') => {
            const values = this.nodeValues.get(item.layer_id);
            values?.set(`${nodeId}.${output}`, value);
        };
    
        const processNode = (nodeId, inputValues = {}, visited = new Set()) => {
            if (visited.has(nodeId)) {
                return;
            }
            visited.add(nodeId);
            
            const executionKey = `${item.layer_id}.${nodeId}`;
            if (this.executionCache.get(executionKey)) {
                return;
            }
            
            this.executionCache.set(executionKey, true);
            
            const node = nodeData.nodes.find(n => n.id === nodeId);
            if (!node) {
                return null;
            }
        
            let outputs = this.executeNode(node, inputValues, item, hasOverlap);
            if (!outputs) return null;
        
            if (typeof outputs === 'object') {
                Object.entries(outputs).forEach(([key, value]) => {
                    setNodeValue(nodeId, value, key);
                    this.broadcastSignal(`${nodeId}.${key}`, value);
                });
            } else {
                setNodeValue(nodeId, outputs);
                this.broadcastSignal(`${nodeId}.output`, outputs);
            }
        
            const connections = nodeData.connections.filter(c => c.startNode === nodeId);
            
            connections.forEach(conn => {
                const nextInputs = {};
                const value = getNodeValue(nodeId, conn.startOutput);
        
                if (value !== undefined || node.type === 'initial' || node.type === 'gamepad') {
                    nextInputs[conn.endInput] = value;
                    
                    const additionalInputs = nodeData.connections
                        .filter(c => c.endNode === conn.endNode && c.startNode !== nodeId)
                        .map(c => ({
                            input: c.endInput,
                            value: getNodeValue(c.startNode, c.startOutput)
                        }));
                    
                    additionalInputs.forEach(({input, value}) => {
                        if (value !== undefined) {
                            nextInputs[input] = value;
                        }
                    });

                    const linkedInputs = Array.from(this.nodeLinks.entries())
                        .filter(([key]) => key.endsWith(`->${conn.endNode}`))
                        .map(([_, signal]) => signal);
                    
                    linkedInputs.forEach(signal => {
                        const signalValue = this.getSignal(signal);
                        if (signalValue !== undefined) {
                            nextInputs[signal] = signalValue;
                        }
                    });
                    
                    processNode(conn.endNode, nextInputs, new Set(visited));
                }
            });
        };
    
        return processNode(startNodeId);
    },

    executeNode(node, inputs, item, hasOverlap) {
        if (node.type === 'initial' || node.type === 'lighting' || node.type === 'color' || node.type === 'colortransition') {
            switch (node.type) {
                case 'initial':
                    return { output: true };
                case 'lighting':
                    return this.executeLightingNode(node, item, inputs);
                case 'color':
                    return this.executeColorNode(node, inputs);
                case 'colortransition':
                    return this.executeColorTransitionNode(node, inputs);
            }
        }
    
        if (!hasOverlap) return null;
    
        switch (node.type) {
            case 'gamepad':
                const buttonPressed = this.executeGamepadNode(node);
                return { output: buttonPressed };
            case 'scene':
                return this.executeSceneNode(node, inputs);
            case 'condition':
                const result = this.executeConditionNode(node, inputs);
                return { 
                    'true': result === true ? true : undefined,
                    'false': result === false ? true : undefined
                };
            case 'timer':
                return this.executeTimerNode(node, inputs);
            case 'switch':
                return this.executeSwitch(node, inputs);
            case 'direction':
                return this.executeDirectionNode(node, inputs);
            case 'move':
                return this.executeMoveNode(node, inputs, item);
            case 'plugin':
                return this.executePluginNode(node, inputs);
            default:
                return null;
        }
    },

    executeGamepadNode(node) {
        const buttonType = node.fields?.button_type;
        const throttleDelay = node.fields?.throttle_delay || 1000;
        if (!buttonType) return false;
    
        const throttleKey = `gamepad_${buttonType}`;
        
        const throttledCheck = this.throttle(() => {
            switch (buttonType) {
                case 'aButton':
                    return gamepad.buttons.includes('a');
                case 'bButton':
                    return gamepad.buttons.includes('b');
                case 'xButton':
                    return gamepad.buttons.includes('x');
                case 'yButton':
                    return gamepad.buttons.includes('y');
                case 'l1':
                    return gamepad.buttons.includes('l1');
                case 'r1':
                    return gamepad.buttons.includes('r1');
                case 'l2':
                    return gamepad.buttons.includes('l2');
                case 'r2':
                    return gamepad.buttons.includes('r2');
                case 'select':
                    return gamepad.buttons.includes('select');
                case 'start':
                    return gamepad.buttons.includes('start');
                case 'leftStick':
                    return gamepad.buttons.includes('leftStick');
                case 'rightStick':
                    return gamepad.buttons.includes('rightStick');
                case 'up':
                    return gamepad.buttons.includes('up');
                case 'down':
                    return gamepad.buttons.includes('down');
                case 'left':
                    return gamepad.buttons.includes('left');
                case 'right':
                    return gamepad.buttons.includes('right');
                case 'aPressed':
                    return gamepad.buttonPressures[0] > 0;
                case 'bPressed':
                    return gamepad.buttonPressures[1] > 0;
                case 'xPressed':
                    return gamepad.buttonPressures[2] > 0;
                case 'yPressed':
                    return gamepad.buttonPressures[3] > 0;
                case 'l1Pressed':
                    return gamepad.buttonPressures[4] > 0;
                case 'r1Pressed':
                    return gamepad.buttonPressures[5] > 0;
                case 'l2Pressed':
                    return gamepad.buttonPressures[6] > 0;
                case 'r2Pressed':
                    return gamepad.buttonPressures[7] > 0;
                case 'selectPressed':
                    return gamepad.buttonPressures[8] > 0;
                case 'startPressed':
                    return gamepad.buttonPressures[9] > 0;
                case 'leftStickPressed':
                    return gamepad.buttonPressures[10] > 0;
                case 'rightStickPressed':
                    return gamepad.buttonPressures[11] > 0;
                case 'aReleased':
                    return !gamepad.buttons.includes('a');
                case 'bReleased':
                    return !gamepad.buttons.includes('b');
                case 'xReleased':
                    return !gamepad.buttons.includes('x');
                case 'yReleased':
                    return !gamepad.buttons.includes('y');
                case 'l1Released':
                    return !gamepad.buttons.includes('l1');
                case 'r1Released':
                    return !gamepad.buttons.includes('r1');
                case 'l2Released':
                    return !gamepad.buttons.includes('l2');
                case 'r2Released':
                    return !gamepad.buttons.includes('r2');
                case 'selectReleased':
                    return !gamepad.buttons.includes('select');
                case 'startReleased':
                    return !gamepad.buttons.includes('start');
                case 'leftStickReleased':
                    return !gamepad.buttons.includes('leftStick');
                case 'rightStickReleased':
                    return !gamepad.buttons.includes('rightStick');
                case 'leftStickMove':
                    return gamepad.axesPressures.leftStickX !== 0 || 
                           gamepad.axesPressures.leftStickY !== 0;
                case 'rightStickMove':
                    return gamepad.axesPressures.rightStickX !== 0 || 
                           gamepad.axesPressures.rightStickY !== 0;
                case 'anyAxis':
                    return gamepad.axesPressures.leftStickX !== 0 || 
                           gamepad.axesPressures.leftStickY !== 0 || 
                           gamepad.axesPressures.rightStickX !== 0 || 
                           gamepad.axesPressures.rightStickY !== 0;
                case 'l2Analog':
                    return gamepad.buttonPressures[6];
                case 'r2Analog':
                    return gamepad.buttonPressures[7];
                default:
                    return false;
            }
        }, throttleDelay, throttleKey);
    
        return throttledCheck();
    },

    executeSceneNode(node, inputs) {
        if (!inputs.input) {
            return null;
        }
        const sceneId = node.fields?.id;
        const startX = node.fields?.x;
        const startY = node.fields?.y;
        if (sceneId) {
            game.scene(sceneId, startX, startY);
            return { output: true };
        }
        return null;
    },

    executeConditionNode(node, inputs) {
        if (!inputs.input) {
            return null;
        }

        const { variable, operator, value } = node.fields;
        const inputValue = inputs.input;
        const compareValue = parseFloat(value);

        switch (operator) {
            case 'equals':
                return inputValue === compareValue;
            case 'not_equals':
                return inputValue !== compareValue;
            case 'greater':
                return inputValue > compareValue;
            case 'less':
                return inputValue < compareValue;
            case 'greater_equals':
                return inputValue >= compareValue;
            case 'less_equals':
                return inputValue <= compareValue;
            default:
                return false;
        }
    },

    executeLightingNode(node, item, inputs) {
        const lightId = `${item.layer_id}_light_${node.id}`;
    
        if ('input' in inputs && !inputs.input) {
            const existingLight = plugin.lighting.lights.find(l => l.id === lightId);
            if (existingLight) {
                const lightIndex = plugin.lighting.lights.indexOf(existingLight);
                if (lightIndex > -1) {
                    plugin.lighting.lights.splice(lightIndex, 1);
                }
            }
            return { output: false };
        }
    
        const x = parseInt(node.fields?.x) || 0;
        const y = parseInt(node.fields?.y) || 0;
        const radius = parseInt(node.fields?.radius) || 200;
        const intensity = parseFloat(node.fields?.intensity) || 1;
        const speed = parseFloat(node.fields?.speed) || 0;
        const amount = parseFloat(node.fields?.amount) || 0;
        
        const baseX = Math.min(...item.x) * 16;
        const baseY = Math.min(...item.y) * 16;
        const lightX = baseX + x;
        const lightY = baseY + y;
    
        let color = { r: 255, g: 255, b: 255 };
        if (inputs.color && typeof inputs.color === 'string' && inputs.color.startsWith('#')) {
            const hexColor = inputs.color;
            color = {
                r: parseInt(hexColor.slice(1,3), 16),
                g: parseInt(hexColor.slice(3,5), 16),
                b: parseInt(hexColor.slice(5,7), 16)
            };
        }
    
        let light = plugin.lighting.lights.find(l => l.id === lightId);
        
        if (light) {
            light.x = lightX;
            light.y = lightY;
            light.baseRadius = radius;
            light.radius = radius;
            light.maxIntensity = intensity;
            light.initialMaxIntensity = intensity;
            light.currentIntensity = intensity;
            light.color = color;
            light.flickerSpeed = speed;
            light.flickerAmount = amount;
        } else {
            plugin.lighting.addLight(
                lightId,
                lightX,
                lightY,
                radius,
                color,
                intensity,
                'lamp',
                speed > 0,
                speed,
                amount
            );
        }
    
        return { output: true };
    },

    executeColorNode(node, inputs) {
        if (!inputs.input) return null;
        const color = node.fields?.color || '#FFFFFF';
        return { output: color };
    },

    executeColorTransitionNode(node, inputs) {
        if (!inputs.input) return null;
        
        const speed = parseFloat(node.fields?.speed) || 0.1;
        const time = (performance.now() / 1000) * speed;
        
        const colors = Object.entries(node.fields)
            .filter(([key]) => key.startsWith('color'))
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([_, value]) => value || "#FFFFFF");
        
        if (colors.length < 2) return { output: colors[0] };
        
        const totalColors = colors.length;
        const position = (time % totalColors) / totalColors * totalColors;
        const index = Math.floor(position);
        const t = position - index;
        
        const color1 = colors[index];
        const color2 = colors[(index + 1) % totalColors];
        
        const r1 = parseInt(color1.slice(1,3), 16);
        const g1 = parseInt(color1.slice(3,5), 16);
        const b1 = parseInt(color1.slice(5,7), 16);
        
        const r2 = parseInt(color2.slice(1,3), 16);
        const g2 = parseInt(color2.slice(3,5), 16);
        const b2 = parseInt(color2.slice(5,7), 16);
        
        const r = Math.round(r1 + (r2 - r1) * t);
        const g = Math.round(g1 + (g2 - g1) * t);
        const b = Math.round(b1 + (b2 - b1) * t);
        
        const color = `#${r.toString(16).padStart(2,'0')}${g.toString(16).padStart(2,'0')}${b.toString(16).padStart(2,'0')}`;
        
        return { output: color };
    },

    executeTimerNode(node, inputs) {
        if (!inputs.input) {
            return null;
        }
    
        const nodeId = node.id;
        const delay = parseFloat(node.fields?.delay || 1) * 1000;
        const loop = node.fields?.loop === true;
        const now = Date.now();
    
        if (!this.timerStates) {
            this.timerStates = new Map();
        }
    
        let timerState = this.timerStates.get(nodeId);
        if (!timerState) {
            timerState = {
                startTime: now,
                triggered: false
            };
            this.timerStates.set(nodeId, timerState);
        }
    
        const elapsed = now - timerState.startTime;
        
        if (elapsed >= delay) {
            if (loop) {
                timerState.startTime = now;
                return { output: true };
            } else if (!timerState.triggered) {
                timerState.triggered = true;
                return { output: true };
            }
        }
        
        return { output: false };
    },

    executeSwitch(node, inputs) {
        if (!inputs.input) {
            return { output: this.switchStates?.get(node.id) || false };
        }
    
        const nodeId = node.id;
        if (!this.switchStates) {
            this.switchStates = new Map();
        }
    
        let currentState = this.switchStates.get(nodeId);
        if (currentState === undefined) {
            currentState = node.fields?.initialState || false;
        }
    
        currentState = !currentState;
        this.switchStates.set(nodeId, currentState);
        
        return { output: currentState };
    },

    executeDirectionNode(node, inputs) {
        if (!inputs.input) {
            return null;
        }
    
        const inputType = node.fields?.input_type || 'leftstick';
        
        if (inputType === 'leftstick') {
            const dirs = gamepad.directions || {};
            const output = {
                up: dirs.up || false,
                down: dirs.down || false,
                left: dirs.left || false,
                right: dirs.right || false
            };
            return { 
                output: output,
                input: inputs.input
            };
        }
        return { output: null };
    },
    
    executeMoveNode(node, inputs, item) {
        const hasInput = inputs.input || (inputs.direction && inputs.direction.input);
        
        if (!hasInput) {
            return null;
        }
    
        const speed = parseFloat(node.fields?.speed || 100);
        const pixelsToMove = (speed * game.deltaTime) / 1000;
        
        const positions = item.x.map((x, i) => ({
            x: x,
            y: item.y[i]
        }));
    
        if (inputs.direction && typeof inputs.direction === 'object') {
            if (inputs.direction.up) {
                positions.forEach(pos => pos.y -= pixelsToMove / 16);
            }
            if (inputs.direction.down) {
                positions.forEach(pos => pos.y += pixelsToMove / 16);
            }
            if (inputs.direction.left) {
                positions.forEach(pos => pos.x -= pixelsToMove / 16);
            }
            if (inputs.direction.right) {
                positions.forEach(pos => pos.x += pixelsToMove / 16);
            }
        }
    
        item.x = positions.map(p => p.x);
        item.y = positions.map(p => p.y);
    
        return { output: true };
    },

    executePluginNode(node, inputs) {
        if (!inputs.input) {
            return null;
        }
    
        const pluginId = node.fields?.plugin_id;
        const path = node.fields?.path || '';
        const ext = node.fields?.ext || 'js';
        const reload = node.fields?.reload === true;
        const hidden = node.fields?.hidden === true;
    
        if (pluginId) {
            plugin.load(pluginId, { 
                path,
                ext,
                reload, 
                hidden 
            });
            return { output: true };
        }
        return null;
    },

    throttle(func, delay, key) {
        return () => {
            const now = Date.now();
            if (!this.throttledEvents) {
                this.throttledEvents = {};
            }
            
            if (!this.throttledEvents[key] || now - this.throttledEvents[key] >= delay) {
                const result = func();
                if (result) {
                    this.throttledEvents[key] = now;
                }
                return result;
            }
            return false;
        };
    },

    checkForNearbyItems() {
        if (!game.roomData?.items) return;
        const sprite = game.mainSprite;
        if (!sprite) return;
    
        game.roomData.items.forEach(item => {
            if (!item.layer_id || !game.roomData.nodeData?.[`item_${item.layer_id}`]) return;
    
            const itemX = Math.min(...item.x) * 16;
            const itemY = Math.min(...item.y) * 16;
            const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16;
            const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16;
    
            const spriteBoundary = {
                left: sprite.x,
                right: sprite.x + sprite.width,
                top: sprite.y,
                bottom: sprite.y + sprite.height
            };
    
            const objectBoundary = {
                left: itemX,
                right: itemX + itemWidth,
                top: itemY,
                bottom: itemY + itemHeight
            };
    
            const hasOverlap = !(
                spriteBoundary.left >= objectBoundary.right ||
                spriteBoundary.right <= objectBoundary.left ||
                spriteBoundary.top >= objectBoundary.bottom ||
                spriteBoundary.bottom <= objectBoundary.top
            );
    
            const nodeData = game.roomData.nodeData[`item_${item.layer_id}`];
            const initialNode = nodeData.nodes.find(n => n.type === 'initial');
            if (initialNode) {
                this.executeNodeGraph(item, initialNode.id, hasOverlap);
            }
        });
    }
};