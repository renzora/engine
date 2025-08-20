/**
 * Shader Manager for Torus Engine
 * Handles shader compilation, linking, and management
 */
export class ShaderManager {
  constructor() {
    this.programs = new Map();
    this.shaders = new Map();
    this.gl = null;
  }

  async initialize(gl) {
    this.gl = gl;
    
    // Compile default shaders
    await this.loadDefaultShaders();
    
    console.log('[Torus Shaders] Manager initialized');
  }

  async loadDefaultShaders() {
    // Basic vertex shader
    const basicVertex = `#version 300 es
      in vec3 a_position;
      in vec3 a_normal;
      in vec2 a_texCoord;
      
      uniform mat4 u_worldMatrix;
      uniform mat4 u_viewMatrix;
      uniform mat4 u_projectionMatrix;
      uniform mat3 u_normalMatrix;
      
      out vec3 v_normal;
      out vec2 v_texCoord;
      out vec3 v_worldPosition;
      
      void main() {
        vec4 worldPosition = u_worldMatrix * vec4(a_position, 1.0);
        v_worldPosition = worldPosition.xyz;
        v_normal = u_normalMatrix * a_normal;
        v_texCoord = a_texCoord;
        
        gl_Position = u_projectionMatrix * u_viewMatrix * worldPosition;
      }
    `;

    // Basic fragment shader
    const basicFragment = `#version 300 es
      precision highp float;
      
      in vec3 v_normal;
      in vec2 v_texCoord;
      in vec3 v_worldPosition;
      
      uniform vec3 u_color;
      uniform vec3 u_lightDirection;
      uniform vec3 u_lightColor;
      uniform vec3 u_ambientLight;
      uniform vec3 u_cameraPosition;
      
      out vec4 fragColor;
      
      void main() {
        vec3 normal = normalize(v_normal);
        vec3 lightDir = normalize(-u_lightDirection);
        
        // Diffuse lighting
        float diff = max(dot(normal, lightDir), 0.0);
        vec3 diffuse = diff * u_lightColor;
        
        // Ambient lighting
        vec3 ambient = u_ambientLight;
        
        // Final color
        vec3 result = (ambient + diffuse) * u_color;
        fragColor = vec4(result, 1.0);
      }
    `;

    // Line shader for grids and wireframes
    const lineVertex = `#version 300 es
      in vec3 a_position;
      
      uniform mat4 u_worldMatrix;
      uniform mat4 u_viewMatrix;
      uniform mat4 u_projectionMatrix;
      
      void main() {
        gl_Position = u_projectionMatrix * u_viewMatrix * u_worldMatrix * vec4(a_position, 1.0);
      }
    `;

    const lineFragment = `#version 300 es
      precision highp float;
      
      uniform vec3 u_color;
      uniform float u_opacity;
      
      out vec4 fragColor;
      
      void main() {
        fragColor = vec4(u_color, u_opacity);
      }
    `;

    // Compile and link programs
    this.createProgram('basic', basicVertex, basicFragment);
    this.createProgram('line', lineVertex, lineFragment);
  }

  createProgram(name, vertexSource, fragmentSource) {
    const gl = this.gl;
    
    try {
      // Compile shaders
      const vertexShader = this.compileShader(gl.VERTEX_SHADER, vertexSource);
      const fragmentShader = this.compileShader(gl.FRAGMENT_SHADER, fragmentSource);
      
      // Create and link program
      const program = gl.createProgram();
      gl.attachShader(program, vertexShader);
      gl.attachShader(program, fragmentShader);
      gl.linkProgram(program);
      
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        throw new Error(`Program linking failed: ${gl.getProgramInfoLog(program)}`);
      }
      
      // Store program
      this.programs.set(name, program);
      
      // Clean up shaders (they're now linked into program)
      gl.deleteShader(vertexShader);
      gl.deleteShader(fragmentShader);
      
      console.log(`[Torus Shaders] Compiled program: ${name}`);
      return program;
      
    } catch (error) {
      console.error(`[Torus Shaders] Failed to create program ${name}:`, error);
      throw error;
    }
  }

  compileShader(type, source) {
    const gl = this.gl;
    const shader = gl.createShader(type);
    
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const error = gl.getShaderInfoLog(shader);
      gl.deleteShader(shader);
      throw new Error(`Shader compilation failed: ${error}`);
    }
    
    return shader;
  }

  getProgram(name) {
    return this.programs.get(name);
  }

  useProgram(name) {
    const program = this.programs.get(name);
    if (program) {
      this.gl.useProgram(program);
      return program;
    }
    throw new Error(`Program not found: ${name}`);
  }

  // Uniform helpers
  setUniform1f(program, name, value) {
    const location = this.gl.getUniformLocation(program, name);
    if (location) this.gl.uniform1f(location, value);
  }

  setUniform3f(program, name, x, y, z) {
    const location = this.gl.getUniformLocation(program, name);
    if (location) this.gl.uniform3f(location, x, y, z);
  }

  setUniformMatrix4fv(program, name, matrix) {
    const location = this.gl.getUniformLocation(program, name);
    if (location) this.gl.uniformMatrix4fv(location, false, matrix);
  }

  setUniformMatrix3fv(program, name, matrix) {
    const location = this.gl.getUniformLocation(program, name);
    if (location) this.gl.uniformMatrix3fv(location, false, matrix);
  }

  // Attribute helpers
  enableAttribute(program, name) {
    const location = this.gl.getAttribLocation(program, name);
    if (location >= 0) {
      this.gl.enableVertexAttribArray(location);
      return location;
    }
    return -1;
  }

  setAttributePointer(location, size, type, normalized, stride, offset) {
    if (location >= 0) {
      this.gl.vertexAttribPointer(location, size, type, normalized, stride, offset);
    }
  }

  async dispose() {
    // Delete all programs
    this.programs.forEach(program => this.gl.deleteProgram(program));
    this.programs.clear();
    
    // Delete individual shaders
    this.shaders.forEach(shader => this.gl.deleteShader(shader));
    this.shaders.clear();
    
    console.log('[Torus Shaders] Disposed');
  }
}