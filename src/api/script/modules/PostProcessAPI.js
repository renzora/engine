// === POST PROCESS API MODULE ===

import {
  PostProcess,
  PassPostProcess,
  BlurPostProcess,
  FxaaPostProcess,
  HighlightsPostProcess,
  ImageProcessingPostProcess,
  ColorCorrectionPostProcess,
  ConvolutionPostProcess,
  FilterPostProcess,
  VolumetricLightScatteringPostProcess,
  DepthOfFieldEffect,
  DefaultRenderingPipeline,
  PostProcessRenderPipeline,
  PostProcessRenderEffect,
  Vector2,
  Vector3,
  Color3,
  Color4,
  Camera,
  RenderTargetTexture,
  Effect,
  Texture
} from '@babylonjs/core';

// Advanced post processes
import { AsciiArtPostProcess } from '@babylonjs/post-processes/asciiArt/asciiArtPostProcess.js';
import { DigitalRainPostProcess } from '@babylonjs/post-processes/digitalRain/digitalRainPostProcess.js';

export class PostProcessAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === BASIC POST PROCESSES ===

  createBlurPostProcess(name, direction, blurWidth, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const directionVector = new Vector2(...direction);
    return new BlurPostProcess(name, directionVector, blurWidth, ratio, targetCamera);
  }

  createFXAAPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new FxaaPostProcess(name, ratio, targetCamera);
  }

  createHighlightsPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new HighlightsPostProcess(name, ratio, targetCamera);
  }

  createImageProcessingPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new ImageProcessingPostProcess(name, ratio, targetCamera);
  }

  createTonemappingPostProcess(name, operator = 0, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    // Use ImageProcessingPostProcess for tone mapping instead
    const postProcess = new ImageProcessingPostProcess(name, ratio, targetCamera);
    postProcess.toneMappingEnabled = true;
    postProcess.toneMappingType = operator;
    return postProcess;
  }

  createColorCorrectionPostProcess(name, colorTableUrl, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new ColorCorrectionPostProcess(name, colorTableUrl, ratio, targetCamera);
  }

  // === ADVANCED POST PROCESSES ===

  createDepthOfFieldEffect(name, options = {}) {
    const camera = options.camera || this.scene.activeCamera;
    if (!camera) return null;
    
    return new DepthOfFieldEffect(this.scene, options.depthTexture, {
      focusDistance: options.focusDistance || 10,
      focalLength: options.focalLength || 50,
      fStop: options.fStop || 1.4,
      blurLevel: options.blurLevel || 1,
      ...options
    });
  }

  createVolumetricLightScattering(name, ratio = 1.0, camera = null, mesh = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new VolumetricLightScatteringPostProcess(name, ratio, targetCamera, mesh);
  }

  createAsciiArtPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new AsciiArtPostProcess(name, ratio, targetCamera);
  }

  createDigitalRainPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new DigitalRainPostProcess(name, ratio, targetCamera);
  }

  // === CUSTOM POST PROCESSES ===

  createCustomPostProcess(name, fragmentShader, uniforms = [], samplers = [], ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    return new PostProcess(name, fragmentShader, uniforms, samplers, ratio, targetCamera);
  }

  createGrayscalePostProcess(name, ratio = 1.0, camera = null) {
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        float gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
        gl_FragColor = vec4(gray, gray, gray, color.a);
      }
    `;
    
    return this.createCustomPostProcess(name, fragmentShader, [], [], ratio, camera);
  }

  createSepiaPostProcess(name, ratio = 1.0, camera = null) {
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        float r = dot(color.rgb, vec3(0.393, 0.769, 0.189));
        float g = dot(color.rgb, vec3(0.349, 0.686, 0.168));
        float b = dot(color.rgb, vec3(0.272, 0.534, 0.131));
        gl_FragColor = vec4(r, g, b, color.a);
      }
    `;
    
    return this.createCustomPostProcess(name, fragmentShader, [], [], ratio, camera);
  }

  createInvertPostProcess(name, ratio = 1.0, camera = null) {
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        gl_FragColor = vec4(1.0 - color.rgb, color.a);
      }
    `;
    
    return this.createCustomPostProcess(name, fragmentShader, [], [], ratio, camera);
  }

  createVignettePostProcess(name, intensity = 0.5, ratio = 1.0, camera = null) {
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float intensity;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        vec2 center = vec2(0.5, 0.5);
        float distance = length(vUV - center);
        float vignette = 1.0 - smoothstep(0.0, 1.0, distance * intensity);
        gl_FragColor = vec4(color.rgb * vignette, color.a);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['intensity'], [], ratio, camera);
    if (postProcess) {
      postProcess.setFloat('intensity', intensity);
    }
    
    return postProcess;
  }

  createPixelatePostProcess(name, pixelSize = 8, ratio = 1.0, camera = null) {
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float pixelSize;
      uniform vec2 screenSize;
      
      void main() {
        vec2 pixelCoord = floor(vUV * screenSize / pixelSize) * pixelSize / screenSize;
        gl_FragColor = texture2D(textureSampler, pixelCoord);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['pixelSize', 'screenSize'], [], ratio, camera);
    if (postProcess) {
      postProcess.setFloat('pixelSize', pixelSize);
      postProcess.setVector2('screenSize', new Vector2(1920, 1080)); // Default screen size
    }
    
    return postProcess;
  }

  // === RENDERING PIPELINE ===

  createDefaultRenderingPipeline(name, hdr = false, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const pipeline = new DefaultRenderingPipeline(name, hdr, this.scene, [targetCamera]);
    return pipeline;
  }

  createCustomRenderingPipeline(name, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const pipeline = new PostProcessRenderPipeline(this.scene.getEngine(), name);
    return pipeline;
  }

  addPostProcessToPipeline(pipeline, renderEffect, insertAt = -1) {
    if (!pipeline || !renderEffect) return false;
    pipeline.addEffect(renderEffect, insertAt);
    return true;
  }

  // === POST PROCESS EFFECTS ===

  createRenderEffect(name, postProcesses) {
    if (!postProcesses || postProcesses.length === 0) return null;
    return new PostProcessRenderEffect(this.scene.getEngine(), name, () => postProcesses);
  }

  enablePostProcess(postProcess, camera = null) {
    if (!postProcess) return false;
    
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return false;
    
    if (targetCamera.attachPostProcess) {
      targetCamera.attachPostProcess(postProcess);
    }
    return true;
  }

  disablePostProcess(postProcess, camera = null) {
    if (!postProcess) return false;
    
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return false;
    
    if (targetCamera.detachPostProcess) {
      targetCamera.detachPostProcess(postProcess);
    }
    return true;
  }

  // === POST PROCESS PROPERTIES ===

  setPostProcessUniforms(postProcess, uniforms = {}) {
    if (!postProcess) return false;
    
    Object.entries(uniforms).forEach(([name, value]) => {
      if (typeof value === 'number') {
        postProcess.setFloat(name, value);
      } else if (Array.isArray(value)) {
        if (value.length === 2) {
          postProcess.setVector2(name, new Vector2(...value));
        } else if (value.length === 3) {
          postProcess.setVector3(name, new Vector3(...value));
        } else if (value.length === 4) {
          postProcess.setColor4(name, new Color4(...value));
        }
      }
    });
    
    return true;
  }

  setPostProcessTexture(postProcess, uniformName, texture) {
    if (!postProcess || !texture) return false;
    postProcess.setTexture(uniformName, texture);
    return true;
  }

  // === SCREEN SPACE EFFECTS ===

  createScreenSpaceReflections(name, ratio = 0.5, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform sampler2D depthSampler;
      uniform float intensity;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        float depth = texture2D(depthSampler, vUV).r;
        
        // Simple reflection calculation
        vec2 reflectUV = vUV;
        reflectUV.y = 1.0 - reflectUV.y;
        
        vec4 reflection = texture2D(textureSampler, reflectUV);
        gl_FragColor = mix(color, reflection, intensity * (1.0 - depth));
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['intensity'], ['depthSampler'], ratio, targetCamera);
    if (postProcess) {
      postProcess.setFloat('intensity', 0.3);
    }
    
    return postProcess;
  }

  createMotionBlur(name, strength = 1.0, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform sampler2D velocityTexture;
      uniform float strength;
      
      void main() {
        vec2 velocity = texture2D(velocityTexture, vUV).xy * strength;
        vec4 color = vec4(0.0);
        
        for(int i = 0; i < 8; i++) {
          vec2 offset = velocity * (float(i) / 8.0 - 0.5);
          color += texture2D(textureSampler, vUV + offset);
        }
        
        gl_FragColor = color / 8.0;
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['strength'], ['velocityTexture'], ratio, targetCamera);
    if (postProcess) {
      postProcess.setFloat('strength', strength);
    }
    
    return postProcess;
  }

  // === COLOR GRADING ===

  createColorGrading(name, options = {}, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float contrast;
      uniform float brightness;
      uniform float saturation;
      uniform vec3 colorFilter;
      
      void main() {
        vec4 color = texture2D(textureSampler, vUV);
        
        // Apply brightness
        color.rgb += brightness;
        
        // Apply contrast
        color.rgb = (color.rgb - 0.5) * contrast + 0.5;
        
        // Apply saturation
        float gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
        color.rgb = mix(vec3(gray), color.rgb, saturation);
        
        // Apply color filter
        color.rgb *= colorFilter;
        
        gl_FragColor = color;
      }
    `;
    
    const postProcess = this.createCustomPostProcess(
      name, 
      fragmentShader, 
      ['contrast', 'brightness', 'saturation', 'colorFilter'], 
      [], 
      ratio, 
      targetCamera
    );
    
    if (postProcess) {
      postProcess.setFloat('contrast', options.contrast || 1.0);
      postProcess.setFloat('brightness', options.brightness || 0.0);
      postProcess.setFloat('saturation', options.saturation || 1.0);
      postProcess.setVector3('colorFilter', new Vector3(...(options.colorFilter || [1, 1, 1])));
    }
    
    return postProcess;
  }

  // === ARTISTIC EFFECTS ===

  createCartoonPostProcess(name, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform vec2 screenSize;
      
      void main() {
        vec2 texel = 1.0 / screenSize;
        vec4 color = texture2D(textureSampler, vUV);
        
        // Quantize colors for cartoon effect
        color.rgb = floor(color.rgb * 8.0) / 8.0;
        
        // Edge detection for outlines
        vec4 n = texture2D(textureSampler, vUV + vec2(0.0, texel.y));
        vec4 s = texture2D(textureSampler, vUV + vec2(0.0, -texel.y));
        vec4 e = texture2D(textureSampler, vUV + vec2(texel.x, 0.0));
        vec4 w = texture2D(textureSampler, vUV + vec2(-texel.x, 0.0));
        
        float edge = length((n - s) + (e - w));
        edge = step(0.3, edge);
        
        gl_FragColor = color * (1.0 - edge);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['screenSize'], [], ratio, targetCamera);
    if (postProcess) {
      postProcess.setVector2('screenSize', new Vector2(1920, 1080));
    }
    
    return postProcess;
  }

  createOilPaintingPostProcess(name, radius = 4, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform vec2 screenSize;
      uniform float radius;
      
      void main() {
        vec2 texel = 1.0 / screenSize;
        vec3 color = vec3(0.0);
        float weight = 0.0;
        
        for(float x = -radius; x <= radius; x += 1.0) {
          for(float y = -radius; y <= radius; y += 1.0) {
            vec2 offset = vec2(x, y) * texel;
            vec3 sample = texture2D(textureSampler, vUV + offset).rgb;
            
            float w = 1.0 / (1.0 + length(offset) * 10.0);
            color += sample * w;
            weight += w;
          }
        }
        
        gl_FragColor = vec4(color / weight, 1.0);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['screenSize', 'radius'], [], ratio, targetCamera);
    if (postProcess) {
      postProcess.setVector2('screenSize', new Vector2(1920, 1080));
      postProcess.setFloat('radius', radius);
    }
    
    return postProcess;
  }

  // === SCREEN DISTORTION ===

  createWaveDistortionPostProcess(name, amplitude = 0.02, frequency = 10, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float time;
      uniform float amplitude;
      uniform float frequency;
      
      void main() {
        vec2 distortedUV = vUV;
        distortedUV.x += sin(vUV.y * frequency + time) * amplitude;
        distortedUV.y += cos(vUV.x * frequency + time) * amplitude;
        
        gl_FragColor = texture2D(textureSampler, distortedUV);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['time', 'amplitude', 'frequency'], [], ratio, targetCamera);
    if (postProcess) {
      postProcess.setFloat('amplitude', amplitude);
      postProcess.setFloat('frequency', frequency);
      
      // Animate time uniform
      this.scene.registerBeforeRender(() => {
        postProcess.setFloat('time', this.scene.getEngine().getDeltaTime() * 0.001);
      });
    }
    
    return postProcess;
  }

  createChromaticAberrationPostProcess(name, aberration = 0.002, ratio = 1.0, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    const fragmentShader = `
      precision highp float;
      varying vec2 vUV;
      uniform sampler2D textureSampler;
      uniform float aberration;
      
      void main() {
        vec2 center = vec2(0.5, 0.5);
        vec2 offset = (vUV - center) * aberration;
        
        float r = texture2D(textureSampler, vUV + offset).r;
        float g = texture2D(textureSampler, vUV).g;
        float b = texture2D(textureSampler, vUV - offset).b;
        
        gl_FragColor = vec4(r, g, b, 1.0);
      }
    `;
    
    const postProcess = this.createCustomPostProcess(name, fragmentShader, ['aberration'], [], ratio, targetCamera);
    if (postProcess) {
      postProcess.setFloat('aberration', aberration);
    }
    
    return postProcess;
  }

  // === POST PROCESS MANAGEMENT ===

  getPostProcessInfo(postProcess) {
    if (!postProcess) return null;
    
    return {
      name: postProcess.name,
      enabled: postProcess.isEnabled,
      ratio: postProcess._ratio,
      type: postProcess.getClassName()
    };
  }

  getAllPostProcesses(camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera || !targetCamera._postProcesses) return [];
    
    return targetCamera._postProcesses.map(pp => ({
      name: pp.name,
      enabled: pp.isEnabled,
      type: pp.getClassName()
    }));
  }

  setPostProcessEnabled(postProcess, enabled) {
    if (!postProcess) return false;
    postProcess.isEnabled = enabled;
    return true;
  }

  disposePostProcess(postProcess) {
    if (!postProcess) return false;
    postProcess.dispose();
    return true;
  }

  clearAllPostProcesses(camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera || !targetCamera._postProcesses) return false;
    
    const postProcesses = [...targetCamera._postProcesses];
    postProcesses.forEach(pp => pp.dispose());
    return true;
  }

  // === PIPELINE PRESETS ===

  enableBasicPipeline(camera = null) {
    const pipeline = this.createDefaultRenderingPipeline('basic_pipeline', false, camera);
    if (pipeline) {
      pipeline.fxaaEnabled = true;
      pipeline.imageProcessingEnabled = true;
      return pipeline;
    }
    return null;
  }

  enableCinematicPipeline(camera = null) {
    const pipeline = this.createDefaultRenderingPipeline('cinematic_pipeline', true, camera);
    if (pipeline) {
      pipeline.fxaaEnabled = true;
      pipeline.imageProcessingEnabled = true;
      pipeline.bloomEnabled = true;
      pipeline.depthOfFieldEnabled = true;
      pipeline.grainEnabled = true;
      pipeline.chromaticAberrationEnabled = true;
      
      // Configure settings
      pipeline.bloom.bloomWeight = 0.15;
      pipeline.bloom.bloomKernel = 64;
      pipeline.depthOfField.focusDistance = 10;
      pipeline.depthOfField.focalLength = 50;
      
      return pipeline;
    }
    return null;
  }

  enableRetroPixelPipeline(camera = null) {
    const pixelate = this.createPixelatePostProcess('retro_pixel', 6, 1.0, camera);
    const colorGrade = this.createColorGrading('retro_grade', {
      contrast: 1.2,
      saturation: 0.7,
      colorFilter: [1.1, 0.9, 0.8]
    }, 1.0, camera);
    
    return { pixelate, colorGrade };
  }
}