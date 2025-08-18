import { GLTF2Export } from '@babylonjs/serializers/glTF/index.js';
import { OBJExport } from '@babylonjs/serializers/OBJ/index.js';
import { SceneSerializer } from '@babylonjs/core/Misc/sceneSerializer.js';
const getBabylonScene = () => window._cleanBabylonScene;

export class SceneExporter {
  static async exportGLTF(scene, fileName = 'scene') {
    if (!scene) {
      throw new Error('No scene to export');
    }

    return new Promise((resolve, reject) => {
      GLTF2Export.GLTFAsync(scene, fileName, {
        shouldExportNode: (node) => {
          return !node.name?.startsWith('__helper');
        }
      }).then((gltf) => {
        gltf.downloadFiles();
        resolve({ success: true, message: 'Scene exported as glTF' });
      }).catch(reject);
    });
  }

  static async exportGLB(scene, fileName = 'scene') {
    if (!scene) {
      throw new Error('No scene to export');
    }

    return new Promise((resolve, reject) => {
      GLTF2Export.GLBAsync(scene, fileName, {
        shouldExportNode: (node) => {
          return !node.name?.startsWith('__helper');
        }
      }).then((glb) => {
        glb.downloadFiles();
        resolve({ success: true, message: 'Scene exported as GLB' });
      }).catch(reject);
    });
  }

  static async exportOBJ(scene, fileName = 'scene') {
    if (!scene) {
      throw new Error('No scene to export');
    }

    try {
      const objText = OBJExport.OBJ(scene.meshes, true, fileName);
      const blob = new Blob([objText], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${fileName}.obj`;
      a.click();
      URL.revokeObjectURL(url);
      
      return { success: true, message: 'Scene exported as OBJ' };
    } catch (error) {
      throw error;
    }
  }

  static async exportBabylon(scene, fileName = 'scene') {
    if (!scene) {
      throw new Error('No scene to export');
    }

    try {
      const serializedScene = SceneSerializer.Serialize(scene);
      const strScene = JSON.stringify(serializedScene, null, 2);
      const blob = new Blob([strScene], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${fileName}.babylon`;
      a.click();
      URL.revokeObjectURL(url);
      
      return { success: true, message: 'Scene exported as Babylon format' };
    } catch (error) {
      throw error;
    }
  }

  static async exportJSON(sceneData, fileName = 'scene') {
    try {
      const blob = new Blob([JSON.stringify(sceneData, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${fileName}.json`;
      a.click();
      URL.revokeObjectURL(url);
      
      return { success: true, message: 'Scene exported as JSON' };
    } catch (error) {
      throw error;
    }
  }

  static getSupportedFormats() {
    return [
      { id: 'gltf', name: 'glTF 2.0', extension: '.gltf', description: 'GL Transmission Format' },
      { id: 'glb', name: 'GLB (Binary)', extension: '.glb', description: 'Binary glTF' },
      { id: 'obj', name: 'Wavefront OBJ', extension: '.obj', description: 'Wavefront OBJ format' },
      { id: 'babylon', name: 'Babylon', extension: '.babylon', description: 'Babylon.js native format' },
      { id: 'json', name: 'JSON', extension: '.json', description: 'Raw scene data JSON' }
    ];
  }
}