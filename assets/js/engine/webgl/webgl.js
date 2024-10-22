// webgl.js

const webglUtils = {
    init: function() {
        // Initialize WebGL context and shaders
        const canvas = game.canvas;
        this.gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    
        if (!this.gl) {
            console.error('WebGL not supported, falling back to canvas.');
            game.useWebGL = false;
            return;
        }
    
        // Set clear color to transparent
        this.gl.clearColor(0.0, 0.0, 0.0, 0.0);
    
        // Enable blending for transparency
        this.gl.enable(this.gl.BLEND);
        this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);
    
        // Enable depth testing
        this.gl.enable(this.gl.DEPTH_TEST);
        // Near things obscure far things
        this.gl.depthFunc(this.gl.LEQUAL);
    
        // Initialize shaders
        const vsSource = `
            attribute vec4 aVertexPosition;
            attribute vec2 aTextureCoord;
    
            uniform mat4 uModelViewMatrix;
            uniform mat4 uProjectionMatrix;
    
            varying highp vec2 vTextureCoord;
    
            void main(void) {
              gl_Position = uProjectionMatrix * uModelViewMatrix * aVertexPosition;
              vTextureCoord = aTextureCoord;
            }
        `;
    
        const fsSource = `
            varying highp vec2 vTextureCoord;
    
            uniform sampler2D uSampler;
    
            void main(void) {
              gl_FragColor = texture2D(uSampler, vTextureCoord);
            }
        `;
    
        const shaderProgram = this.initShaderProgram(this.gl, vsSource, fsSource);
    
        this.programInfo = {
            program: shaderProgram,
            attribLocations: {
                vertexPosition: this.gl.getAttribLocation(shaderProgram, 'aVertexPosition'),
                textureCoord: this.gl.getAttribLocation(shaderProgram, 'aTextureCoord'),
            },
            uniformLocations: {
                projectionMatrix: this.gl.getUniformLocation(shaderProgram, 'uProjectionMatrix'),
                modelViewMatrix: this.gl.getUniformLocation(shaderProgram, 'uModelViewMatrix'),
                uSampler: this.gl.getUniformLocation(shaderProgram, 'uSampler'),
            },
        };
    
        this.initBuffers();
    
        // Prepare texture storage
        this.textures = {};
    
        // Load textures and then start the game loop
        this.initTextures(() => {
            game.loop();
        });
    },
    

    initShaderProgram: function(gl, vsSource, fsSource) {
        const vertexShader = this.loadShader(gl, gl.VERTEX_SHADER, vsSource);
        const fragmentShader = this.loadShader(gl, gl.FRAGMENT_SHADER, fsSource);

        // Create the shader program
        const shaderProgram = gl.createProgram();
        gl.attachShader(shaderProgram, vertexShader);
        gl.attachShader(shaderProgram, fragmentShader);
        gl.linkProgram(shaderProgram);

        // If creating the shader program failed, alert
        if (!gl.getProgramParameter(shaderProgram, gl.LINK_STATUS)) {
            console.error('Unable to initialize the shader program: ' + gl.getProgramInfoLog(shaderProgram));
            return null;
        }

        return shaderProgram;
    },

    loadShader: function(gl, type, source) {
        const shader = gl.createShader(type);

        // Send the source to the shader object
        gl.shaderSource(shader, source);

        // Compile the shader program
        gl.compileShader(shader);

        // See if it compiled successfully
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
            console.error('An error occurred compiling the shaders: ' + gl.getShaderInfoLog(shader));
            gl.deleteShader(shader);
            return null;
        }

        return shader;
    },

    initBuffers: function() {
        // Create a buffer for the square's positions.
        const positionBuffer = this.gl.createBuffer();

        // Create a buffer for the texture coordinates.
        const textureCoordBuffer = this.gl.createBuffer();

        // Build the element array buffer.
        const indexBuffer = this.gl.createBuffer();

        // This array defines two triangles forming a rectangle.
        const indices = [0, 1, 2, 1, 2, 3];

        this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
        this.gl.bufferData(this.gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(indices), this.gl.STATIC_DRAW);

        this.buffers = {
            position: positionBuffer,
            textureCoord: textureCoordBuffer,
            indices: indexBuffer,
        };
    },

    initTextures: function(callback) {
        const texturesToLoad = [];

        // Load textures for all images used in game.objectData
        for (let objectId in game.objectData) {
            const objectEntries = game.objectData[objectId];
            objectEntries.forEach(entry => {
                const imageKey = entry.t;
                if (!this.textures[imageKey]) {
                    const image = assets.load(imageKey);
                    if (image) {
                        if (image.complete) {
                            // Image is already loaded
                            this.textures[imageKey] = this.loadTexture(image);
                        } else {
                            // Image is not loaded yet, wait for it
                            texturesToLoad.push(new Promise((resolve) => {
                                image.onload = () => {
                                    this.textures[imageKey] = this.loadTexture(image);
                                    resolve();
                                };
                                image.onerror = () => {
                                    console.error(`Failed to load image ${imageKey}`);
                                    resolve(); // Resolve to continue even if there's an error
                                };
                            }));
                        }
                    } else {
                        console.error(`Image asset ${imageKey} not found in assets.`);
                    }
                }
            });
        }

        // Load textures for sprite layers
        const spriteLayers = ['head', 'eyes', 'hair', 'hands', 'hats', 'glasses', 'facial', 'outfit', 'horse'];
        spriteLayers.forEach(layer => {
            if (!this.textures[layer]) {
                const image = assets.load(layer);
                if (image) {
                    if (image.complete) {
                        // Image is already loaded
                        this.textures[layer] = this.loadTexture(image);
                    } else {
                        // Image is not loaded yet, wait for it
                        texturesToLoad.push(new Promise((resolve) => {
                            image.onload = () => {
                                this.textures[layer] = this.loadTexture(image);
                                resolve();
                            };
                            image.onerror = () => {
                                console.error(`Failed to load image for layer ${layer}`);
                                resolve(); // Resolve to continue even if there's an error
                            };
                        }));
                    }
                } else {
                    console.error(`Image asset for layer ${layer} not found in assets.`);
                }
            }
        });

        // Wait for all textures to load before proceeding
        if (texturesToLoad.length > 0) {
            Promise.all(texturesToLoad).then(() => {
                if (callback) callback();
            });
        } else {
            // All textures are already loaded
            if (callback) callback();
        }
    },

    loadTexture: function(image) {
        const texture = this.gl.createTexture();
        this.gl.bindTexture(this.gl.TEXTURE_2D, texture);

        // Flip the image's Y axis to match the WebGL texture coordinate space
        this.gl.pixelStorei(this.gl.UNPACK_FLIP_Y_WEBGL, true);

        // Upload the image into the texture.
        this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, this.gl.RGBA, this.gl.UNSIGNED_BYTE, image);

        // Set the parameters so we can render any size image
        this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_S, this.gl.CLAMP_TO_EDGE); // Clamp to edge to prevent wrapping
        this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_T, this.gl.CLAMP_TO_EDGE); // Clamp to edge to prevent wrapping
        this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MIN_FILTER, this.gl.NEAREST); // Use nearest neighbor filtering
        this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MAG_FILTER, this.gl.NEAREST); // Use nearest neighbor filtering

        return texture;
    },

    drawTile: function(options) {
        const {
            texture,
            tileFrameIndex,
            tilesetImageKey,
            tilesetWidth,
            tilesetHeight,
            tileSize,
            posX,
            posY,
            offsetX,
            offsetY,
            rotation,
        } = options;
    
        // Round the positions to the nearest whole number
        const roundedPosX = Math.round(posX + offsetX);
        const roundedPosY = Math.round(posY + offsetY);
    
        // The number of tiles per row in the tileset image
        const tilesPerRow = Math.floor(tilesetWidth / 16); // Assuming each tile is 16 pixels wide
    
        // Calculate the tile's position in the tileset
        const tileXIndex = tileFrameIndex % tilesPerRow;
        const tileYIndex = Math.floor(tileFrameIndex / tilesPerRow);
    
        // Calculate the texture coordinates
        const s0 = tileXIndex * 16 / tilesetWidth;
        const s1 = (tileXIndex + 1) * 16 / tilesetWidth;
    
        // Adjust the t-coordinates to account for the Y-axis flip
        const t0 = 1 - ((tileYIndex + 1) * 16 / tilesetHeight);
        const t1 = 1 - (tileYIndex * 16 / tilesetHeight);
    
        const gl = this.gl;
        const programInfo = this.programInfo;
        const buffers = this.buffers;
    
        gl.useProgram(programInfo.program);
    
        // Set up the projection matrix
        const projectionMatrix = this.mat4.create();
        this.mat4.ortho(projectionMatrix, 0, game.canvas.width, game.canvas.height, 0, -1, 1);
    
        gl.uniformMatrix4fv(
            programInfo.uniformLocations.projectionMatrix,
            false,
            projectionMatrix
        );
    
        // Set alpha to 1.0 for tiles
        gl.uniform1f(programInfo.uniformLocations.uAlpha, 1.0);
    
        // Bind buffers
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
        gl.enableVertexAttribArray(programInfo.attribLocations.vertexPosition);
    
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.textureCoord);
        gl.enableVertexAttribArray(programInfo.attribLocations.textureCoord);
    
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.indices);
    
        // Bind the tileset texture
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.uniform1i(programInfo.uniformLocations.uSampler, 0);
    
        // Update the texture coordinates buffer
        const textureCoordinates = [
            s0, t1,  // Bottom-left
            s1, t1,  // Bottom-right
            s0, t0,  // Top-left
            s1, t0,  // Top-right
        ];
    
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.textureCoord);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(textureCoordinates), gl.STATIC_DRAW);
        gl.vertexAttribPointer(
            programInfo.attribLocations.textureCoord,
            2,          // numComponents
            gl.FLOAT,   // type
            false,      // normalize
            0,          // stride
            0           // offset
        );
    
        // Set up the modelViewMatrix
        const modelViewMatrix = this.mat4.create();
        this.mat4.translate(modelViewMatrix, modelViewMatrix, [roundedPosX, roundedPosY, 0]);
        this.mat4.rotate(modelViewMatrix, modelViewMatrix, rotation, [0, 0, 1]);
    
        gl.uniformMatrix4fv(
            programInfo.uniformLocations.modelViewMatrix,
            false,
            modelViewMatrix
        );
    
        // Set up positions
        const positions = [
            0, 0, 0,
            tileSize, 0, 0,
            0, tileSize, 0,
            tileSize, tileSize, 0,
        ];
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(positions), gl.STATIC_DRAW);
        gl.vertexAttribPointer(
            programInfo.attribLocations.vertexPosition,
            3,          // numComponents
            gl.FLOAT,   // type
            false,      // normalize
            0,          // stride
            0           // offset
        );
    
        // Draw the tile
        gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_SHORT, 0);
    },
    
    

    renderSprite: function(sprite) {
        const gl = this.gl;
        const programInfo = this.programInfo;
        const buffers = this.buffers;

        gl.useProgram(programInfo.program);

        // Set up the projection matrix
        const projectionMatrix = this.mat4.create();
        this.mat4.ortho(projectionMatrix, 0, game.canvas.width, game.canvas.height, 0, -1, 1);

        gl.uniformMatrix4fv(
            programInfo.uniformLocations.projectionMatrix,
            false,
            projectionMatrix
        );

        // Set up positions
        const posX = (sprite.x - camera.cameraX) * game.zoomLevel;
        const posY = (sprite.y - camera.cameraY) * game.zoomLevel;

        // Set the modelViewMatrix
        const modelViewMatrix = this.mat4.create();
        this.mat4.translate(modelViewMatrix, modelViewMatrix, [posX, posY, 0]);

        // Flip sprite if necessary
        if (['W', 'NW', 'SW'].includes(sprite.direction)) {
            this.mat4.scale(modelViewMatrix, modelViewMatrix, [-1, 1, 1]);
            this.mat4.translate(modelViewMatrix, modelViewMatrix, [-sprite.width * game.zoomLevel, 0, 0]);
        }

        gl.uniformMatrix4fv(
            programInfo.uniformLocations.modelViewMatrix,
            false,
            modelViewMatrix
        );

        // Bind buffers
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
        gl.enableVertexAttribArray(programInfo.attribLocations.vertexPosition);

        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.textureCoord);
        gl.enableVertexAttribArray(programInfo.attribLocations.textureCoord);

        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.indices);

        // Adjust positions and sizes
        const spriteWidth = sprite.width * game.zoomLevel;
        const spriteHeight = sprite.height * game.zoomLevel;

        if (sprite.type === 'horse') {
            // For horse sprites, use the single image asset
            const texture = this.textures['horse'];
            if (!texture) {
                console.error(`Texture for horse sprite not found.`);
                return;
            }

            // Bind the texture
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, texture);
            gl.uniform1i(programInfo.uniformLocations.uSampler, 0);

            // Calculate texture coordinates
            // Assuming the horse image fits the sprite size
            const textureCoordinates = [
                0, 1,
                1, 1,
                0, 0,
                1, 0,
            ];

            gl.bindBuffer(gl.ARRAY_BUFFER, buffers.textureCoord);
            gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(textureCoordinates), gl.STATIC_DRAW);
            gl.vertexAttribPointer(
                programInfo.attribLocations.textureCoord,
                2,          // numComponents
                gl.FLOAT,   // type
                false,      // normalize
                0,          // stride
                0           // offset
            );

            // Set up positions
            const positions = [
                0, 0, 0,
                spriteWidth, 0, 0,
                0, spriteHeight, 0,
                spriteWidth, spriteHeight, 0,
            ];
            gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
            gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(positions), gl.STATIC_DRAW);
            gl.vertexAttribPointer(
                programInfo.attribLocations.vertexPosition,
                3,          // numComponents
                gl.FLOAT,   // type
                false,      // normalize
                0,          // stride
                0           // offset
            );

            // Draw the horse sprite
            gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_SHORT, 0);

        } else {
            // Layers to render
            const layers = ['outfit', 'head', 'eyes', 'hair', 'facial', 'hats', 'glasses', 'hands'];
            for (let layer of layers) {
                // Check if the sprite has this layer
                if (sprite[layer] !== 0) {
                    const texture = this.textures[layer];
                    if (!texture) {
                        console.error(`Texture for sprite layer ${layer} not found.`);
                        continue;
                    }

                    // Bind the texture
                    gl.activeTexture(gl.TEXTURE0);
                    gl.bindTexture(gl.TEXTURE_2D, texture);
                    gl.uniform1i(programInfo.uniformLocations.uSampler, 0);

                    // Calculate texture coordinates based on the sprite's current frame and direction
                    const textureCoordinates = this.getSpriteTextureCoordinates(sprite, layer);
                    if (!textureCoordinates) continue;

                    gl.bindBuffer(gl.ARRAY_BUFFER, buffers.textureCoord);
                    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(textureCoordinates), gl.STATIC_DRAW);
                    gl.vertexAttribPointer(
                        programInfo.attribLocations.textureCoord,
                        2,          // numComponents
                        gl.FLOAT,   // type
                        false,      // normalize
                        0,          // stride
                        0           // offset
                    );

                    // Set up positions
                    const positions = [
                        0, 0, 0,
                        spriteWidth, 0, 0,
                        0, spriteHeight, 0,
                        spriteWidth, spriteHeight, 0,
                    ];
                    gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
                    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(positions), gl.STATIC_DRAW);
                    gl.vertexAttribPointer(
                        programInfo.attribLocations.vertexPosition,
                        3,          // numComponents
                        gl.FLOAT,   // type
                        false,      // normalize
                        0,          // stride
                        0           // offset
                    );

                    // Draw the sprite layer
                    gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_SHORT, 0);
                }
            }
        }
    },

    getSpriteTextureCoordinates: function(sprite, layer) {
        // Load the appropriate image to get dimensions
        const image = assets.load(layer);
        if (!image) {
            console.error(`Image for sprite layer ${layer} not found.`);
            return null;
        }
    
        const frameWidth = sprite.width; // Adjust as necessary
        const frameHeight = sprite.height; // Adjust as necessary
        const textureWidth = image.width;
        const textureHeight = image.height;
    
        // Calculate columns and rows in the sprite sheet
        const cols = Math.floor(textureWidth / frameWidth);
        const rows = Math.floor(textureHeight / frameHeight);
    
        // Determine the frame index based on sprite's currentFrame and direction
        const directionMap = {
            'S': 0,
            'E': 8,
            'N': 16,
            'SE': 24,
            'NE': 32,
            'SW': 24,
            'NW': 32,
            'W': 8, // Flipped during rendering
        };
    
        let frameIndex = directionMap[sprite.direction] + Math.floor(sprite.currentFrame) % 8;
    
        // For layers like 'hair', 'hats', 'glasses', adjust frameIndex based on direction
        if (['hair', 'hats', 'glasses', 'facial'].includes(layer)) {
            const layerDirectionMap = {
                'S': 0,
                'E': 1,
                'N': 2,
                'SE': 3,
                'NE': 4,
                'SW': 3,
                'NW': 4,
                'W': 1, // Flipped during rendering
            };
            frameIndex = layerDirectionMap[sprite.direction];
        }
    
        // Calculate texture coordinates
        const col = frameIndex % cols;
        const row = Math.floor(frameIndex / cols);
    
        const s0 = col * frameWidth / textureWidth;
        const s1 = (col + 1) * frameWidth / textureWidth;
    
        // Adjust the t-coordinates to account for the Y-axis flip
        const t0 = 1 - ((row + 1) * frameHeight / textureHeight);
        const t1 = 1 - (row * frameHeight / textureHeight);
    
        // Return texture coordinates in the correct order
        return [
            s0, t1,  // Bottom-left
            s1, t1,  // Bottom-right
            s0, t0,  // Top-left
            s1, t0,  // Top-right
        ];
    },    

    // Helper function to parse tile indices from ranges like "4-7"
    getTileFrameFromRange: function(rangeString) {
        if (typeof rangeString === 'string' && rangeString.includes('-')) {
            const [startStr, endStr] = rangeString.split('-');
            const start = parseInt(startStr, 10);
            const end = parseInt(endStr, 10);
            // For simplicity, return the start index
            return start;
        } else {
            return parseInt(rangeString, 10);
        }
    },

    // Expand tile data to handle ranges and other formats
    expandTileData: function(tileData) {
        const expandedTileData = { ...tileData };
        if (tileData.i && typeof tileData.i[0] === 'string' && tileData.i[0].includes('-')) {
            // Expand the ranges into individual indices
            const indices = [];
            tileData.i.forEach(rangeStr => {
                const [startStr, endStr] = rangeStr.split('-');
                const start = parseInt(startStr, 10);
                const end = parseInt(endStr, 10);
                for (let i = start; i <= end; i++) {
                    indices.push(i);
                }
            });
            expandedTileData.i = indices;
        }
        return expandedTileData;
    },

    // Matrix utility functions
    mat4: {
        create: function() {
            let out = new Float32Array(16);
            out[0] = 1;  out[5] = 1;  out[10] = 1;  out[15] = 1;
            return out;
        },

        ortho: function(out, left, right, bottom, top, near, far) {
            let lr = 1 / (left - right);
            let bt = 1 / (bottom - top);
            let nf = 1 / (near - far);

            out[0]  = -2 * lr;     out[1]  = 0;           out[2]  = 0;          out[3]  = 0;
            out[4]  = 0;           out[5]  = -2 * bt;     out[6]  = 0;          out[7]  = 0;
            out[8]  = 0;           out[9]  = 0;           out[10] = 2 * nf;     out[11] = 0;
            out[12] = (left + right) * lr;
            out[13] = (top + bottom) * bt;
            out[14] = (far + near) * nf;
            out[15] = 1;
            return out;
        },

        translate: function(out, a, v) {
            let x = v[0], y = v[1], z = v[2];
            if (a === out) {
                out[12] = a[0] * x + a[4] * y + a[8]  * z + a[12];
                out[13] = a[1] * x + a[5] * y + a[9]  * z + a[13];
                out[14] = a[2] * x + a[6] * y + a[10] * z + a[14];
                out[15] = a[3] * x + a[7] * y + a[11] * z + a[15];
            } else {
                out[0] = a[0];  out[1] = a[1];  out[2] = a[2];   out[3] = a[3];
                out[4] = a[4];  out[5] = a[5];  out[6] = a[6];   out[7] = a[7];
                out[8] = a[8];  out[9] = a[9];  out[10] = a[10]; out[11] = a[11];
                out[12] = a[0] * x + a[4] * y + a[8]  * z + a[12];
                out[13] = a[1] * x + a[5] * y + a[9]  * z + a[13];
                out[14] = a[2] * x + a[6] * y + a[10] * z + a[14];
                out[15] = a[3] * x + a[7] * y + a[11] * z + a[15];
            }
            return out;
        },

        rotate: function(out, a, rad, axis) {
            let x = axis[0], y = axis[1], z = axis[2];
            let len = Math.hypot(x, y, z);
            if (len < 0.000001) { return null; }
            len = 1 / len;
            x *= len;
            y *= len;
            z *= len;
            let s = Math.sin(rad);
            let c = Math.cos(rad);
            let t = 1 - c;

            // Construct the rotation matrix components
            let b00 = x * x * t + c,     b01 = y * x * t + z * s, b02 = z * x * t - y * s;
            let b10 = x * y * t - z * s, b11 = y * y * t + c,     b12 = z * y * t + x * s;
            let b20 = x * z * t + y * s, b21 = y * z * t - x * s, b22 = z * z * t + c;

            // Perform rotation-specific matrix multiplication
            let a00 = a[0], a01 = a[1], a02 = a[2],  a03 = a[3];
            let a10 = a[4], a11 = a[5], a12 = a[6],  a13 = a[7];
            let a20 = a[8], a21 = a[9], a22 = a[10], a23 = a[11];

            out[0] = a00 * b00 + a10 * b01 + a20 * b02;
            out[1] = a01 * b00 + a11 * b01 + a21 * b02;
            out[2] = a02 * b00 + a12 * b01 + a22 * b02;
            out[3] = a03 * b00 + a13 * b01 + a23 * b02;
            out[4] = a00 * b10 + a10 * b11 + a20 * b12;
            out[5] = a01 * b10 + a11 * b11 + a21 * b12;
            out[6] = a02 * b10 + a12 * b11 + a22 * b12;
            out[7] = a03 * b10 + a13 * b11 + a23 * b12;
            out[8] = a00 * b20 + a10 * b21 + a20 * b22;
            out[9] = a01 * b20 + a11 * b21 + a21 * b22;
            out[10] = a02 * b20 + a12 * b21 + a22 * b22;
            out[11] = a03 * b20 + a13 * b21 + a23 * b22;

            // Copy the last row
            out[12] = a[12];
            out[13] = a[13];
            out[14] = a[14];
            out[15] = a[15];

            return out;
        },

        scale: function(out, a, v) {
            let x = v[0], y = v[1], z = v[2];

            out[0] = a[0] * x;
            out[1] = a[1] * x;
            out[2] = a[2] * x;
            out[3] = a[3] * x;
            out[4] = a[4] * y;
            out[5] = a[5] * y;
            out[6] = a[6] * y;
            out[7] = a[7] * y;
            out[8] = a[8] * z;
            out[9] = a[9] * z;
            out[10] = a[10] * z;
            out[11] = a[11] * z;
            out[12] = a[12];
            out[13] = a[13];
            out[14] = a[14];
            out[15] = a[15];

            return out;
        },
    },
};
