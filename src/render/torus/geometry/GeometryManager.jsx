/**
 * Geometry Manager - Handles primitive creation and mesh buffers
 */
export class GeometryManager {
  constructor() {
    this.gl = null;
    this.geometries = new Map();
    this.buffers = new Map();
    this.totalTriangles = 0;
  }

  async initialize(gl) {
    this.gl = gl;
    console.log('[Torus Geometry] Manager initialized');
  }

  createPrimitive(type, options = {}) {
    switch (type) {
      case 'box':
        return this.createBoxGeometry(options);
      case 'sphere':
        return this.createSphereGeometry(options);
      case 'cylinder':
        return this.createCylinderGeometry(options);
      case 'plane':
        return this.createPlaneGeometry(options);
      case 'torus':
        return this.createTorusGeometry(options);
      case 'cone':
        return this.createConeGeometry(options);
      default:
        console.warn(`[Torus Geometry] Unknown primitive: ${type}, using box`);
        return this.createBoxGeometry(options);
    }
  }

  createBoxGeometry(options = {}) {
    const width = options.width || 1;
    const height = options.height || 1; 
    const depth = options.depth || 1;
    const smooth = options.smooth || false;
    
    if (smooth) {
      return this.createSmoothBoxGeometry(width, height, depth);
    }
    
    const w = width / 2;
    const h = height / 2;
    const d = depth / 2;
    
    // Full cube vertices (position, normal, texCoord)
    const vertices = new Float32Array([
      // Front face
      -w, -h,  d,  0,  0,  1,  0, 0,
       w, -h,  d,  0,  0,  1,  1, 0,
       w,  h,  d,  0,  0,  1,  1, 1,
      -w,  h,  d,  0,  0,  1,  0, 1,
      
      // Back face
      -w, -h, -d,  0,  0, -1,  1, 0,
      -w,  h, -d,  0,  0, -1,  1, 1,
       w,  h, -d,  0,  0, -1,  0, 1,
       w, -h, -d,  0,  0, -1,  0, 0,
      
      // Top face
      -w,  h, -d,  0,  1,  0,  0, 1,
      -w,  h,  d,  0,  1,  0,  0, 0,
       w,  h,  d,  0,  1,  0,  1, 0,
       w,  h, -d,  0,  1,  0,  1, 1,
      
      // Bottom face
      -w, -h, -d,  0, -1,  0,  1, 1,
       w, -h, -d,  0, -1,  0,  0, 1,
       w, -h,  d,  0, -1,  0,  0, 0,
      -w, -h,  d,  0, -1,  0,  1, 0,
      
      // Right face
       w, -h, -d,  1,  0,  0,  1, 0,
       w,  h, -d,  1,  0,  0,  1, 1,
       w,  h,  d,  1,  0,  0,  0, 1,
       w, -h,  d,  1,  0,  0,  0, 0,
      
      // Left face
      -w, -h, -d, -1,  0,  0,  0, 0,
      -w, -h,  d, -1,  0,  0,  1, 0,
      -w,  h,  d, -1,  0,  0,  1, 1,
      -w,  h, -d, -1,  0,  0,  0, 1
    ]);
    
    // Cube indices
    const indices = new Uint16Array([
      0,  1,  2,    0,  2,  3,    // Front
      4,  5,  6,    4,  6,  7,    // Back
      8,  9, 10,    8, 10, 11,    // Top
      12, 13, 14,   12, 14, 15,   // Bottom
      16, 17, 18,   16, 18, 19,   // Right
      20, 21, 22,   20, 22, 23    // Left
    ]);
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices,
      indices,
      vertexCount: indices.length
    };
  }

  createSmoothBoxGeometry(width = 1, height = 1, depth = 1) {
    const w = width / 2;
    const h = height / 2;
    const d = depth / 2;
    
    // 8 unique vertices with averaged normals for smooth shading
    const vertices = new Float32Array([
      // Vertex 0: (-w, -h, -d)
      -w, -h, -d,  -0.577, -0.577, -0.577,  0, 0,
      // Vertex 1: (w, -h, -d)
       w, -h, -d,   0.577, -0.577, -0.577,  1, 0,
      // Vertex 2: (w, h, -d)
       w,  h, -d,   0.577,  0.577, -0.577,  1, 1,
      // Vertex 3: (-w, h, -d)
      -w,  h, -d,  -0.577,  0.577, -0.577,  0, 1,
      // Vertex 4: (-w, -h, d)
      -w, -h,  d,  -0.577, -0.577,  0.577,  1, 0,
      // Vertex 5: (w, -h, d)
       w, -h,  d,   0.577, -0.577,  0.577,  0, 0,
      // Vertex 6: (w, h, d)
       w,  h,  d,   0.577,  0.577,  0.577,  0, 1,
      // Vertex 7: (-w, h, d)
      -w,  h,  d,  -0.577,  0.577,  0.577,  1, 1
    ]);
    
    // Indices for the 12 triangles (2 per face)
    const indices = new Uint16Array([
      // Front face (z+)
      4, 5, 6,  4, 6, 7,
      // Back face (z-)
      1, 0, 3,  1, 3, 2,
      // Top face (y+)
      3, 7, 6,  3, 6, 2,
      // Bottom face (y-)
      0, 1, 5,  0, 5, 4,
      // Right face (x+)
      1, 2, 6,  1, 6, 5,
      // Left face (x-)
      0, 4, 7,  0, 7, 3
    ]);
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices,
      indices,
      vertexCount: indices.length
    };
  }

  createSphereGeometry(options = {}) {
    const radius = options.radius || 0.5;
    const widthSegments = options.widthSegments || 32;  // Increased from 16
    const heightSegments = options.heightSegments || 24; // Increased from 12
    
    const vertices = [];
    const indices = [];
    
    // Generate vertices
    for (let lat = 0; lat <= heightSegments; lat++) {
      const theta = lat * Math.PI / heightSegments;
      const sinTheta = Math.sin(theta);
      const cosTheta = Math.cos(theta);
      
      for (let lon = 0; lon <= widthSegments; lon++) {
        const phi = lon * 2 * Math.PI / widthSegments;
        const sinPhi = Math.sin(phi);
        const cosPhi = Math.cos(phi);
        
        const x = cosPhi * sinTheta;
        const y = cosTheta;
        const z = sinPhi * sinTheta;
        
        const u = 1 - (lon / widthSegments);
        const v = 1 - (lat / heightSegments);
        
        // Position
        vertices.push(radius * x, radius * y, radius * z);
        // Normal (same as position for unit sphere)
        vertices.push(x, y, z);
        // UV
        vertices.push(u, v);
      }
    }
    
    // Generate indices
    for (let lat = 0; lat < heightSegments; lat++) {
      for (let lon = 0; lon < widthSegments; lon++) {
        const first = (lat * (widthSegments + 1)) + lon;
        const second = first + widthSegments + 1;
        
        // First triangle
        indices.push(first, second, first + 1);
        // Second triangle
        indices.push(second, second + 1, first + 1);
      }
    }
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices: new Float32Array(vertices),
      indices: new Uint16Array(indices),
      vertexCount: indices.length
    };
  }

  createCylinderGeometry(options = {}) {
    const radiusTop = options.radiusTop || 0.5;
    const radiusBottom = options.radiusBottom || 0.5;
    const height = options.height || 1;
    const radialSegments = options.radialSegments || 32;  // Increased from 16
    const heightSegments = options.heightSegments || 1;
    
    const vertices = [];
    const indices = [];
    
    const halfHeight = height / 2;
    let index = 0;
    
    // Generate side vertices
    for (let y = 0; y <= heightSegments; y++) {
      const v = y / heightSegments;
      const yPos = -halfHeight + v * height;
      const radius = radiusBottom + v * (radiusTop - radiusBottom);
      
      for (let x = 0; x <= radialSegments; x++) {
        const u = x / radialSegments;
        const theta = u * Math.PI * 2;
        
        const sinTheta = Math.sin(theta);
        const cosTheta = Math.cos(theta);
        
        // Position
        vertices.push(radius * cosTheta, yPos, radius * sinTheta);
        // Normal (simplified - pointing outward)
        vertices.push(cosTheta, 0, sinTheta);
        // UV
        vertices.push(u, v);
      }
    }
    
    // Generate side indices (proper counter-clockwise winding)
    for (let y = 0; y < heightSegments; y++) {
      for (let x = 0; x < radialSegments; x++) {
        const first = (y * (radialSegments + 1)) + x;
        const second = first + radialSegments + 1;
        
        // First triangle (counter-clockwise when viewed from outside)
        indices.push(first, second, first + 1);
        // Second triangle (counter-clockwise when viewed from outside)
        indices.push(second, second + 1, first + 1);
      }
    }
    
    // Store side vertex count for cap indexing
    const sideVertexCount = vertices.length / 8;
    
    // Add top cap if radiusTop > 0
    if (radiusTop > 0) {
      const topCenterIndex = sideVertexCount;
      
      // Top cap center
      vertices.push(0, halfHeight, 0, 0, 1, 0, 0.5, 0.5);
      
      // Top cap ring vertices
      for (let x = 0; x < radialSegments; x++) {
        const u = x / radialSegments;
        const theta = u * Math.PI * 2;
        const cosTheta = Math.cos(theta);
        const sinTheta = Math.sin(theta);
        
        vertices.push(radiusTop * cosTheta, halfHeight, radiusTop * sinTheta, 0, 1, 0, u, 0);
      }
      
      // Top cap triangles (looking down from above, counter-clockwise)
      for (let x = 0; x < radialSegments; x++) {
        const ringStart = topCenterIndex + 1;
        const next = (x + 1) % radialSegments;
        indices.push(topCenterIndex, ringStart + next, ringStart + x);
      }
    }
    
    // Add bottom cap if radiusBottom > 0
    if (radiusBottom > 0) {
      const bottomCenterIndex = vertices.length / 8;
      
      // Bottom cap center
      vertices.push(0, -halfHeight, 0, 0, -1, 0, 0.5, 0.5);
      
      // Bottom cap ring vertices
      for (let x = 0; x < radialSegments; x++) {
        const u = x / radialSegments;
        const theta = u * Math.PI * 2;
        const cosTheta = Math.cos(theta);
        const sinTheta = Math.sin(theta);
        
        vertices.push(radiusBottom * cosTheta, -halfHeight, radiusBottom * sinTheta, 0, -1, 0, u, 1);
      }
      
      // Bottom cap triangles (looking up from below, counter-clockwise)
      for (let x = 0; x < radialSegments; x++) {
        const ringStart = bottomCenterIndex + 1;
        const next = (x + 1) % radialSegments;
        indices.push(bottomCenterIndex, ringStart + x, ringStart + next);
      }
    }
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices: new Float32Array(vertices),
      indices: new Uint16Array(indices),
      vertexCount: indices.length
    };
  }

  createTorusGeometry(options = {}) {
    const majorRadius = options.majorRadius || 0.75;    // Distance from center to tube center
    const minorRadius = options.minorRadius || 0.25;    // Tube thickness
    const majorSegments = options.majorSegments || 32;  // Segments around the major circle
    const minorSegments = options.minorSegments || 20;  // Segments around the tube
    
    const vertices = [];
    const indices = [];
    
    // Generate vertices
    for (let i = 0; i <= majorSegments; i++) {
      const u = i / majorSegments;
      const theta = u * Math.PI * 2; // Angle around major circle
      
      for (let j = 0; j <= minorSegments; j++) {
        const v = j / minorSegments;
        const phi = v * Math.PI * 2; // Angle around minor circle (tube)
        
        // Calculate position
        const cosPhi = Math.cos(phi);
        const sinPhi = Math.sin(phi);
        const cosTheta = Math.cos(theta);
        const sinTheta = Math.sin(theta);
        
        // Torus surface position
        const x = (majorRadius + minorRadius * cosPhi) * cosTheta;
        const y = minorRadius * sinPhi;
        const z = (majorRadius + minorRadius * cosPhi) * sinTheta;
        
        // Normal vector (pointing outward from tube surface)
        const nx = cosPhi * cosTheta;
        const ny = sinPhi;
        const nz = cosPhi * sinTheta;
        
        // UV coordinates
        const texU = u;
        const texV = v;
        
        // Add vertex: position + normal + UV
        vertices.push(x, y, z, nx, ny, nz, texU, texV);
      }
    }
    
    // Generate indices
    for (let i = 0; i < majorSegments; i++) {
      for (let j = 0; j < minorSegments; j++) {
        const first = (i * (minorSegments + 1)) + j;
        const second = first + minorSegments + 1;
        
        // First triangle (counter-clockwise)
        indices.push(first, second, first + 1);
        // Second triangle (counter-clockwise)
        indices.push(second, second + 1, first + 1);
      }
    }
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices: new Float32Array(vertices),
      indices: new Uint16Array(indices),
      vertexCount: indices.length
    };
  }

  createConeGeometry(options = {}) {
    const radius = options.radius || 0.5;
    const height = options.height || 1;
    const radialSegments = options.radialSegments || 16;
    const heightSegments = options.heightSegments || 1;
    
    const vertices = [];
    const indices = [];
    
    const halfHeight = height / 2;
    
    // Apex vertex (tip of cone)
    vertices.push(0, halfHeight, 0, 0, 1, 0, 0.5, 0); // position, normal, UV
    const apexIndex = 0;
    
    // Base center vertex  
    vertices.push(0, -halfHeight, 0, 0, -1, 0, 0.5, 0.5);
    const baseCenterIndex = 1;
    
    // Generate base ring vertices
    for (let i = 0; i <= radialSegments; i++) {
      const angle = (i / radialSegments) * Math.PI * 2;
      const x = Math.cos(angle) * radius;
      const z = Math.sin(angle) * radius;
      
      // Calculate normal for smooth shading
      const sideLength = Math.sqrt(radius * radius + height * height);
      const normalY = radius / sideLength;
      const normalXZ = height / sideLength;
      const nx = Math.cos(angle) * normalXZ;
      const nz = Math.sin(angle) * normalXZ;
      
      // Side vertex (for cone surface)
      vertices.push(x, -halfHeight, z, nx, normalY, nz, i / radialSegments, 1);
      
      // Base vertex (for bottom cap)
      vertices.push(x, -halfHeight, z, 0, -1, 0, (x/radius + 1) * 0.5, (z/radius + 1) * 0.5);
    }
    
    // Generate side triangles (cone surface)
    for (let i = 0; i < radialSegments; i++) {
      const baseStart = 2; // After apex and base center
      const current = baseStart + i * 2; // Side vertices
      const next = baseStart + ((i + 1) % radialSegments) * 2;
      
      // Triangle from apex to base edge
      indices.push(apexIndex, current, next);
    }
    
    // Generate base triangles
    for (let i = 0; i < radialSegments; i++) {
      const baseStart = 2; // After apex and base center  
      const current = baseStart + i * 2 + 1; // Base vertices (offset by 1)
      const next = baseStart + ((i + 1) % radialSegments) * 2 + 1;
      
      // Triangle for base (reversed winding for bottom face)
      indices.push(baseCenterIndex, next, current);
    }
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices: new Float32Array(vertices),
      indices: new Uint16Array(indices),
      vertexCount: indices.length
    };
  }

  createPlaneGeometry(options = {}) {
    const width = options.width || 1;
    const height = options.height || 1;
    
    const w = width / 2;
    const h = height / 2;
    
    const vertices = new Float32Array([
      // Position       Normal      UV
      -w, 0, -h,       0, 1, 0,    0, 1,
       w, 0, -h,       0, 1, 0,    1, 1,
       w, 0,  h,       0, 1, 0,    1, 0,
      -w, 0,  h,       0, 1, 0,    0, 0
    ]);
    
    const indices = new Uint16Array([
      0, 1, 2,  0, 2, 3
    ]);
    
    this.totalTriangles += indices.length / 3;
    
    return {
      vertices,
      indices,
      vertexCount: indices.length
    };
  }

  createGrid(options = {}) {
    // Create a wireframe grid using line geometry
    const size = options.size || 10;
    const divisions = options.divisions || 10;
    
    const vertices = [];
    const indices = [];
    
    const halfSize = size / 2;
    const step = size / divisions;
    
    let vertexIndex = 0;
    
    // Create vertical lines
    for (let i = 0; i <= divisions; i++) {
      const x = -halfSize + i * step;
      
      // Start point
      vertices.push(x, 0, -halfSize, 0, 1, 0, 0, 0);
      // End point  
      vertices.push(x, 0, halfSize, 0, 1, 0, 1, 0);
      
      indices.push(vertexIndex, vertexIndex + 1);
      vertexIndex += 2;
    }
    
    // Create horizontal lines
    for (let i = 0; i <= divisions; i++) {
      const z = -halfSize + i * step;
      
      // Start point
      vertices.push(-halfSize, 0, z, 0, 1, 0, 0, 0);
      // End point
      vertices.push(halfSize, 0, z, 0, 1, 0, 1, 0);
      
      indices.push(vertexIndex, vertexIndex + 1);
      vertexIndex += 2;
    }
    
    return {
      vertices: new Float32Array(vertices),
      indices: new Uint16Array(indices),
      vertexCount: indices.length,
      primitive: 'lines' // Special flag for line rendering
    };
  }

  getTotalTriangles() {
    return this.totalTriangles;
  }

  async dispose() {
    console.log('[Torus Geometry] Disposed');
  }
}