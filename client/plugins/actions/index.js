actions = {
    audioCooldown: 0.5,
    lastPlayedTimesByType: {},
    throttleInterval: 2000,
    lastExecutionTime: 0,
    proximityThreshold: 42,

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

    executeNodeGraph(item, startNodeId) {
        if (!game.roomData?.nodeData?.[`item_${item.layer_id}`]) {
            return;
        }
        
        const nodeData = game.roomData.nodeData[`item_${item.layer_id}`];
        this.executionCache.clear();
        this.nodeValues.set(item.layer_id, new Map());
        
        const getNodeValue = (nodeId, output = 'output') => {
            const values = this.nodeValues.get(item.layer_id);
            const value = values?.get(`${nodeId}.${output}`);
            return value;
        };
    
        const setNodeValue = (nodeId, value, output = 'output') => {
            const values = this.nodeValues.get(item.layer_id);
            values?.set(`${nodeId}.${output}`, value);
        };
    
        const processNode = (nodeId, inputValues = {}) => {
            const executionKey = `${item.layer_id}.${nodeId}`;
            if (this.executionCache.get(executionKey)) {
                return;
            }
            
            this.executionCache.set(executionKey, true);
            
            const node = nodeData.nodes.find(n => n.id === nodeId);
            if (!node) {
                return null;
            }
        
            let outputs = this.executeNode(node, inputValues, item);
            if (!outputs) return null;
        
            if (typeof outputs === 'object') {
                Object.entries(outputs).forEach(([key, value]) => {
                    setNodeValue(nodeId, value, key);
                });
            } else {
                setNodeValue(nodeId, outputs);
            }
        
            const connections = nodeData.connections.filter(c => c.startNode === nodeId);
            
            connections.forEach(conn => {
                const nextInputs = {};
                const value = getNodeValue(nodeId, conn.startOutput);
        
                if (value !== undefined || node.type === 'initial' || node.type === 'gamepad') {
                    nextInputs[conn.endInput] = value;
                    processNode(conn.endNode, nextInputs);
                }
            });
        }
    
        return processNode(startNodeId);
    },

    executeNode(node, inputs, item) {
        switch (node.type) {
            case 'initial':
                return { output: true };

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

            case 'lighting':
                return this.executeLightingNode(node, item, inputs);

            case 'color':
                return this.executeColorNode(node, inputs);

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
        const x = parseInt(node.fields?.x) || 0;
        const y = parseInt(node.fields?.y) || 0;
        const radius = parseInt(node.fields?.radius) || 200;
        const intensity = parseFloat(node.fields?.intensity) || 1;
        const speed = parseInt(node.fields?.speed) || 50;
        const amount = parseInt(node.fields?.amount) || 50;
        const flicker = node.fields?.flicker === 'true' || false;
        const baseX = Math.min(...item.x) * 16;
        const baseY = Math.min(...item.y) * 16;
        const lightX = baseX + x;
        const lightY = baseY + y;
        const lightId = item.layer_id + '_light_' + node.id;
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
        if (!light) {
            plugin.lighting.addLight(
                lightId,
                lightX,
                lightY,
                radius,
                color,
                intensity,
                'lamp',
                flicker,
                speed,
                amount,
                null
            );
        } else {
            light.x = lightX;
            light.y = lightY;
            light.baseRadius = radius;
            light.radius = radius;
            light.maxIntensity = intensity;
            light.initialMaxIntensity = intensity;
            light.currentIntensity = intensity;
            light.color = color;
            light.flicker = flicker;
            light.flickerSpeed = speed;
            light.flickerAmount = amount;
            light.flickerOffset = light.flickerOffset || Math.random() * 1000;
        }
        return { output: true };
    },    

     executeColorNode(node, inputs) {
        if (!inputs.input) return null;
        const color = node.fields?.color || '#FFFFFF';
        return { output: color };
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
    
        const spriteBoundary = {
            left: sprite.x,
            right: sprite.x + sprite.width,
            top: sprite.y,
            bottom: sprite.y + sprite.height
        };
    
        game.roomData.items.forEach(item => {
            if (!item.layer_id || !game.roomData.nodeData?.[`item_${item.layer_id}`]) return;
    
            const itemX = Math.min(...item.x) * 16;
            const itemY = Math.min(...item.y) * 16;
            const itemWidth = (Math.max(...item.x) - Math.min(...item.x) + 1) * 16;
            const itemHeight = (Math.max(...item.y) - Math.min(...item.y) + 1) * 16;
    
            const objectBoundary = {
                left: itemX,
                right: itemX + itemWidth,
                top: itemY,
                bottom: itemY + itemHeight
            };
    
            const inViewport = !(
                objectBoundary.left >= game.viewportXEnd * 16 ||
                objectBoundary.right <= game.viewportXStart * 16 ||
                objectBoundary.top >= game.viewportYEnd * 16 ||
                objectBoundary.bottom <= game.viewportYStart * 16
            );
    
            const nodeData = game.roomData.nodeData[`item_${item.layer_id}`];
            const hasLightingNode = nodeData.nodes.some(n => n.type === 'lighting');

            const hasOverlap = !(
                spriteBoundary.left >= objectBoundary.right ||
                spriteBoundary.right <= objectBoundary.left ||
                spriteBoundary.top >= objectBoundary.bottom ||
                spriteBoundary.bottom <= objectBoundary.top
            );
    
            if (hasLightingNode && inViewport) {
                const initialNode = nodeData.nodes.find(n => n.type === 'initial');
                if (initialNode) {
                    this.executeNodeGraph(item, initialNode.id);
                }
            } else if (hasOverlap) {
                const initialNode = nodeData.nodes.find(n => n.type === 'initial');
                if (initialNode) {
                    this.executeNodeGraph(item, initialNode.id);
                }
            }
        });
    }
};