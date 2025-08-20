/**
 * Ray Casting Utilities for 3D picking
 */
export class RaycastUtils {
  
  /**
   * Create a ray from camera through screen coordinates
   */
  static createRayFromMouse(mouseX, mouseY, canvas, viewMatrix, projectionMatrix) {
    // Convert mouse coordinates to normalized device coordinates (-1 to 1)
    const rect = canvas.getBoundingClientRect();
    const x = ((mouseX - rect.left) / rect.width) * 2 - 1;
    const y = -(((mouseY - rect.top) / rect.height) * 2 - 1); // Flip Y
    
    // Create ray in clip space
    const rayClip = [x, y, -1, 1];
    
    // Convert to eye space
    const projMatrixInv = this.invertMatrix(projectionMatrix);
    const rayEye = this.multiplyMatrixVector(projMatrixInv, rayClip);
    rayEye[2] = -1; // Forward direction
    rayEye[3] = 0;  // Direction, not position
    
    // Convert to world space
    const viewMatrixInv = this.invertMatrix(viewMatrix);
    const rayWorld = this.multiplyMatrixVector(viewMatrixInv, rayEye);
    
    // Extract camera position from view matrix
    const cameraPos = this.getCameraPositionFromViewMatrix(viewMatrix);
    
    // Normalize ray direction
    const rayDir = [rayWorld[0], rayWorld[1], rayWorld[2]];
    const length = Math.sqrt(rayDir[0]*rayDir[0] + rayDir[1]*rayDir[1] + rayDir[2]*rayDir[2]);
    rayDir[0] /= length;
    rayDir[1] /= length;
    rayDir[2] /= length;
    
    return {
      origin: cameraPos,
      direction: rayDir
    };
  }
  
  /**
   * Test ray intersection with axis-aligned bounding box
   */
  static rayAABBIntersection(ray, aabbMin, aabbMax) {
    const invDir = [
      1.0 / ray.direction[0],
      1.0 / ray.direction[1], 
      1.0 / ray.direction[2]
    ];
    
    const t1 = (aabbMin[0] - ray.origin[0]) * invDir[0];
    const t2 = (aabbMax[0] - ray.origin[0]) * invDir[0];
    const t3 = (aabbMin[1] - ray.origin[1]) * invDir[1];
    const t4 = (aabbMax[1] - ray.origin[1]) * invDir[1];
    const t5 = (aabbMin[2] - ray.origin[2]) * invDir[2];
    const t6 = (aabbMax[2] - ray.origin[2]) * invDir[2];
    
    const tmin = Math.max(Math.max(Math.min(t1, t2), Math.min(t3, t4)), Math.min(t5, t6));
    const tmax = Math.min(Math.min(Math.max(t1, t2), Math.max(t3, t4)), Math.max(t5, t6));
    
    // If tmax < 0, ray is intersecting AABB but entire AABB is behind us
    if (tmax < 0) {
      return null;
    }
    
    // If tmin > tmax, ray doesn't intersect AABB
    if (tmin > tmax) {
      return null;
    }
    
    // Return intersection distance
    return tmin < 0 ? tmax : tmin;
  }
  
  /**
   * Test ray intersection with sphere
   */
  static raySphereIntersection(ray, sphereCenter, sphereRadius) {
    const oc = [
      ray.origin[0] - sphereCenter[0],
      ray.origin[1] - sphereCenter[1],
      ray.origin[2] - sphereCenter[2]
    ];
    
    const a = ray.direction[0]*ray.direction[0] + ray.direction[1]*ray.direction[1] + ray.direction[2]*ray.direction[2];
    const b = 2.0 * (oc[0]*ray.direction[0] + oc[1]*ray.direction[1] + oc[2]*ray.direction[2]);
    const c = oc[0]*oc[0] + oc[1]*oc[1] + oc[2]*oc[2] - sphereRadius*sphereRadius;
    
    const discriminant = b*b - 4*a*c;
    
    if (discriminant < 0) {
      return null; // No intersection
    }
    
    const t1 = (-b - Math.sqrt(discriminant)) / (2*a);
    const t2 = (-b + Math.sqrt(discriminant)) / (2*a);
    
    if (t1 > 0) return t1;
    if (t2 > 0) return t2;
    return null;
  }
  
  /**
   * Create bounding box for a cylinder (simplified as AABB)
   */
  static getCylinderAABB(position, radius, height) {
    const halfHeight = height / 2;
    return {
      min: [position.x - radius, position.y - halfHeight, position.z - radius],
      max: [position.x + radius, position.y + halfHeight, position.z + radius]
    };
  }
  
  /**
   * Get distance from point to line segment (for gizmo arrow picking)
   */
  static pointToLineDistance(point, lineStart, lineEnd) {
    const A = [point[0] - lineStart[0], point[1] - lineStart[1], point[2] - lineStart[2]];
    const B = [lineEnd[0] - lineStart[0], lineEnd[1] - lineStart[1], lineEnd[2] - lineStart[2]];
    
    const B_length_sq = B[0]*B[0] + B[1]*B[1] + B[2]*B[2];
    const dot_AB = A[0]*B[0] + A[1]*B[1] + A[2]*B[2];
    
    const t = Math.max(0, Math.min(1, dot_AB / B_length_sq));
    
    const closest = [
      lineStart[0] + t * B[0],
      lineStart[1] + t * B[1], 
      lineStart[2] + t * B[2]
    ];
    
    const distance_sq = 
      (point[0] - closest[0]) * (point[0] - closest[0]) +
      (point[1] - closest[1]) * (point[1] - closest[1]) +
      (point[2] - closest[2]) * (point[2] - closest[2]);
      
    return Math.sqrt(distance_sq);
  }
  
  // Matrix utilities
  static invertMatrix(matrix) {
    // Simplified 4x4 matrix inversion (for view/projection matrices)
    const inv = new Float32Array(16);
    const m = matrix;
    
    inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15] + 
             m[9] * m[7] * m[14] + m[13] * m[6] * m[11] - m[13] * m[7] * m[10];
    inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15] - 
             m[8] * m[7] * m[14] - m[12] * m[6] * m[11] + m[12] * m[7] * m[10];
    inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15] + 
             m[8] * m[7] * m[13] + m[12] * m[5] * m[11] - m[12] * m[7] * m[9];
    inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14] - 
              m[8] * m[6] * m[13] - m[12] * m[5] * m[10] + m[12] * m[6] * m[9];
    
    inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15] - 
             m[9] * m[3] * m[14] - m[13] * m[2] * m[11] + m[13] * m[3] * m[10];
    inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15] + 
             m[8] * m[3] * m[14] + m[12] * m[2] * m[11] - m[12] * m[3] * m[10];
    inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15] - 
             m[8] * m[3] * m[13] - m[12] * m[1] * m[11] + m[12] * m[3] * m[9];
    inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14] + 
              m[8] * m[2] * m[13] + m[12] * m[1] * m[10] - m[12] * m[2] * m[9];
    
    inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15] + 
             m[5] * m[3] * m[14] + m[13] * m[2] * m[7] - m[13] * m[3] * m[6];
    inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15] - 
             m[4] * m[3] * m[14] - m[12] * m[2] * m[7] + m[12] * m[3] * m[6];
    inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15] + 
              m[4] * m[3] * m[13] + m[12] * m[1] * m[7] - m[12] * m[3] * m[5];
    inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14] - 
              m[4] * m[2] * m[13] - m[12] * m[1] * m[6] + m[12] * m[2] * m[5];
    
    inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11] - 
             m[5] * m[3] * m[10] - m[9] * m[2] * m[7] + m[9] * m[3] * m[6];
    inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11] + 
             m[4] * m[3] * m[10] + m[8] * m[2] * m[7] - m[8] * m[3] * m[6];
    inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11] - 
              m[4] * m[3] * m[9] - m[8] * m[1] * m[7] + m[8] * m[3] * m[5];
    inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10] + 
              m[4] * m[2] * m[9] + m[8] * m[1] * m[6] - m[8] * m[2] * m[5];
    
    const det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
    
    if (det === 0) return null;
    
    const detInv = 1.0 / det;
    for (let i = 0; i < 16; i++) {
      inv[i] *= detInv;
    }
    
    return inv;
  }
  
  static multiplyMatrixVector(matrix, vector) {
    return [
      matrix[0] * vector[0] + matrix[4] * vector[1] + matrix[8] * vector[2] + matrix[12] * vector[3],
      matrix[1] * vector[0] + matrix[5] * vector[1] + matrix[9] * vector[2] + matrix[13] * vector[3],
      matrix[2] * vector[0] + matrix[6] * vector[1] + matrix[10] * vector[2] + matrix[14] * vector[3],
      matrix[3] * vector[0] + matrix[7] * vector[1] + matrix[11] * vector[2] + matrix[15] * vector[3]
    ];
  }
  
  static getCameraPositionFromViewMatrix(viewMatrix) {
    const invView = this.invertMatrix(viewMatrix);
    return [invView[12], invView[13], invView[14]];
  }
}