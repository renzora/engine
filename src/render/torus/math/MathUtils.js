/**
 * Math Utilities for Torus Engine
 * Optimized matrix and vector operations
 */
export class MathUtils {
  
  // ============= Matrix Operations =============

  static perspective(out, fovy, aspect, near, far) {
    const f = 1.0 / Math.tan(fovy / 2);
    const nf = 1 / (near - far);
    
    out[0] = f / aspect; out[1] = 0; out[2] = 0; out[3] = 0;
    out[4] = 0; out[5] = f; out[6] = 0; out[7] = 0;
    out[8] = 0; out[9] = 0; out[10] = (far + near) * nf; out[11] = -1;
    out[12] = 0; out[13] = 0; out[14] = 2 * far * near * nf; out[15] = 0;
    
    return out;
  }

  static lookAt(out, eye, center, up) {
    const eyeX = eye[0], eyeY = eye[1], eyeZ = eye[2];
    const centerX = center[0], centerY = center[1], centerZ = center[2];
    const upX = up[0], upY = up[1], upZ = up[2];
    
    let x0, x1, x2, y0, y1, y2, z0, z1, z2, len;
    
    if (Math.abs(eyeX - centerX) < 0.000001 &&
        Math.abs(eyeY - centerY) < 0.000001 &&
        Math.abs(eyeZ - centerZ) < 0.000001) {
      // Eye and center are the same, return identity
      return MathUtils.identity(out);
    }
    
    z0 = eyeX - centerX;
    z1 = eyeY - centerY;
    z2 = eyeZ - centerZ;
    
    len = 1 / Math.sqrt(z0 * z0 + z1 * z1 + z2 * z2);
    z0 *= len;
    z1 *= len;
    z2 *= len;
    
    x0 = upY * z2 - upZ * z1;
    x1 = upZ * z0 - upX * z2;
    x2 = upX * z1 - upY * z0;
    len = Math.sqrt(x0 * x0 + x1 * x1 + x2 * x2);
    if (!len) {
      x0 = 0; x1 = 0; x2 = 0;
    } else {
      len = 1 / len;
      x0 *= len; x1 *= len; x2 *= len;
    }
    
    y0 = z1 * x2 - z2 * x1;
    y1 = z2 * x0 - z0 * x2;
    y2 = z0 * x1 - z1 * x0;
    
    out[0] = x0; out[1] = y0; out[2] = z0; out[3] = 0;
    out[4] = x1; out[5] = y1; out[6] = z1; out[7] = 0;
    out[8] = x2; out[9] = y2; out[10] = z2; out[11] = 0;
    out[12] = -(x0 * eyeX + x1 * eyeY + x2 * eyeZ);
    out[13] = -(y0 * eyeX + y1 * eyeY + y2 * eyeZ);
    out[14] = -(z0 * eyeX + z1 * eyeY + z2 * eyeZ);
    out[15] = 1;
    
    return out;
  }

  static identity(out) {
    out[0] = 1; out[1] = 0; out[2] = 0; out[3] = 0;
    out[4] = 0; out[5] = 1; out[6] = 0; out[7] = 0;
    out[8] = 0; out[9] = 0; out[10] = 1; out[11] = 0;
    out[12] = 0; out[13] = 0; out[14] = 0; out[15] = 1;
    return out;
  }

  static translation(out, x, y, z) {
    MathUtils.identity(out);
    out[12] = x;
    out[13] = y;
    out[14] = z;
    return out;
  }

  static scale(out, sx, sy, sz) {
    MathUtils.identity(out);
    out[0] = sx;
    out[5] = sy;
    out[10] = sz;
    return out;
  }

  static rotationX(out, angle) {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    
    MathUtils.identity(out);
    out[5] = c;
    out[6] = s;
    out[9] = -s;
    out[10] = c;
    return out;
  }

  static rotationY(out, angle) {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    
    MathUtils.identity(out);
    out[0] = c;
    out[2] = -s;
    out[8] = s;
    out[10] = c;
    return out;
  }

  static rotationZ(out, angle) {
    const c = Math.cos(angle);
    const s = Math.sin(angle);
    
    MathUtils.identity(out);
    out[0] = c;
    out[1] = s;
    out[4] = -s;
    out[5] = c;
    return out;
  }

  static multiply(out, a, b) {
    const a00 = a[0], a01 = a[1], a02 = a[2], a03 = a[3];
    const a10 = a[4], a11 = a[5], a12 = a[6], a13 = a[7];
    const a20 = a[8], a21 = a[9], a22 = a[10], a23 = a[11];
    const a30 = a[12], a31 = a[13], a32 = a[14], a33 = a[15];

    const b00 = b[0], b01 = b[1], b02 = b[2], b03 = b[3];
    const b10 = b[4], b11 = b[5], b12 = b[6], b13 = b[7];
    const b20 = b[8], b21 = b[9], b22 = b[10], b23 = b[11];
    const b30 = b[12], b31 = b[13], b32 = b[14], b33 = b[15];

    out[0] = a00 * b00 + a01 * b10 + a02 * b20 + a03 * b30;
    out[1] = a00 * b01 + a01 * b11 + a02 * b21 + a03 * b31;
    out[2] = a00 * b02 + a01 * b12 + a02 * b22 + a03 * b32;
    out[3] = a00 * b03 + a01 * b13 + a02 * b23 + a03 * b33;

    out[4] = a10 * b00 + a11 * b10 + a12 * b20 + a13 * b30;
    out[5] = a10 * b01 + a11 * b11 + a12 * b21 + a13 * b31;
    out[6] = a10 * b02 + a11 * b12 + a12 * b22 + a13 * b32;
    out[7] = a10 * b03 + a11 * b13 + a12 * b23 + a13 * b33;

    out[8] = a20 * b00 + a21 * b10 + a22 * b20 + a23 * b30;
    out[9] = a20 * b01 + a21 * b11 + a22 * b21 + a23 * b31;
    out[10] = a20 * b02 + a21 * b12 + a22 * b22 + a23 * b32;
    out[11] = a20 * b03 + a21 * b13 + a22 * b23 + a23 * b33;

    out[12] = a30 * b00 + a31 * b10 + a32 * b20 + a33 * b30;
    out[13] = a30 * b01 + a31 * b11 + a32 * b21 + a33 * b31;
    out[14] = a30 * b02 + a31 * b12 + a32 * b22 + a33 * b32;
    out[15] = a30 * b03 + a31 * b13 + a32 * b23 + a33 * b33;

    return out;
  }

  static normalMatrix(out, worldMatrix) {
    // Extract 3x3 upper matrix (simplified - should be inverse transpose)
    out[0] = worldMatrix[0]; out[1] = worldMatrix[1]; out[2] = worldMatrix[2];
    out[3] = worldMatrix[4]; out[4] = worldMatrix[5]; out[5] = worldMatrix[6];
    out[6] = worldMatrix[8]; out[7] = worldMatrix[9]; out[8] = worldMatrix[10];
    return out;
  }

  // ============= Vector Operations =============

  static vec3Normalize(out, a) {
    const x = a[0], y = a[1], z = a[2];
    let len = x * x + y * y + z * z;
    
    if (len > 0) {
      len = 1 / Math.sqrt(len);
      out[0] = x * len;
      out[1] = y * len;
      out[2] = z * len;
    }
    
    return out;
  }

  static vec3Cross(out, a, b) {
    const ax = a[0], ay = a[1], az = a[2];
    const bx = b[0], by = b[1], bz = b[2];

    out[0] = ay * bz - az * by;
    out[1] = az * bx - ax * bz;
    out[2] = ax * by - ay * bx;
    
    return out;
  }

  static vec3Dot(a, b) {
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  }

  // ============= Utilities =============

  static createMatrix() {
    return new Float32Array(16);
  }

  static createVector3() {
    return new Float32Array(3);
  }

  static createVector2() {
    return new Float32Array(2);
  }

  static toRadians(degrees) {
    return degrees * Math.PI / 180;
  }

  static toDegrees(radians) {
    return radians * 180 / Math.PI;
  }

  static clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }

  static lerp(a, b, t) {
    return a + (b - a) * t;
  }
}