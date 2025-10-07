import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader.js';
import { GLTFFileLoader } from '@babylonjs/loaders/glTF/index.js';
import { OBJFileLoader } from '@babylonjs/loaders/OBJ/index.js';
const getBabylonScene = () => window._cleanBabylonScene;

SceneLoader.RegisterPlugin(new GLTFFileLoader());
SceneLoader.RegisterPlugin(new OBJFileLoader());

export class SceneImporter {
  static async importFile(file) {
    if (!file) {
      throw new Error('No file provided');
    }

    const fileName = file.name.toLowerCase();
    const fileExtension = fileName.split('.').pop();

    switch (fileExtension) {
      case 'gltf':
      case 'glb':
        return await this.importGLTF(file);
      case 'obj':
        return await this.importOBJ(file);
      case 'babylon':
        return await this.importBabylon(file);
      case 'json':
        return await this.importJSON(file);
      default:
        throw new Error(`Unsupported file format: ${fileExtension}`);
    }
  }

  static async importGLTF(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      
      reader.onload = async (e) => {
        try {
          const data = e.target.result;
          const scene = getBabylonScene();
          
          if (!scene) {
            throw new Error('No active scene');
          }

          const blob = new Blob([data], { type: file.type });
          const url = URL.createObjectURL(blob);
          
          await SceneLoader.AppendAsync(url, '', scene, undefined, '.glb');
          
          URL.revokeObjectURL(url);
          resolve({ success: true, message: 'GLTF/GLB file imported successfully' });
        } catch (error) {
          reject(error);
        }
      };
      
      reader.onerror = reject;
      reader.readAsArrayBuffer(file);
    });
  }

  static async importOBJ(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      
      reader.onload = async (e) => {
        try {
          const data = e.target.result;
          const scene = getBabylonScene();
          
          if (!scene) {
            throw new Error('No active scene');
          }

          const blob = new Blob([data], { type: 'text/plain' });
          const url = URL.createObjectURL(blob);
          
          await SceneLoader.AppendAsync(url, '', scene, undefined, '.obj');
          
          URL.revokeObjectURL(url);
          resolve({ success: true, message: 'OBJ file imported successfully' });
        } catch (error) {
          reject(error);
        }
      };
      
      reader.onerror = reject;
      reader.readAsText(file);
    });
  }

  static async importBabylon(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      
      reader.onload = async (e) => {
        try {
          const data = e.target.result;
          const scene = getBabylonScene();
          
          if (!scene) {
            throw new Error('No active scene');
          }

          JSON.parse(data);
          
          await SceneLoader.AppendAsync('', 'data:' + data, scene, undefined, '.babylon');
          
          resolve({ success: true, message: 'Babylon file imported successfully' });
        } catch (error) {
          reject(error);
        }
      };
      
      reader.onerror = reject;
      reader.readAsText(file);
    });
  }

  static async importJSON(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      
      reader.onload = (e) => {
        try {
          const data = JSON.parse(e.target.result);
          
          resolve({ 
            success: true, 
            message: 'JSON scene data loaded', 
            data: data 
          });
        } catch (error) {
          reject(error);
        }
      };
      
      reader.onerror = reject;
      reader.readAsText(file);
    });
  }

  static createFileInput(accept = '.gltf,.glb,.obj,.babylon,.json') {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.style.display = 'none';
    return input;
  }

  static async selectAndImportFile() {
    return new Promise((resolve, reject) => {
      const input = this.createFileInput();
      
      input.onchange = async (e) => {
        const file = e.target.files[0];
        if (file) {
          try {
            const result = await this.importFile(file);
            resolve(result);
          } catch (error) {
            reject(error);
          }
        } else {
          reject(new Error('No file selected'));
        }
        
        document.body.removeChild(input);
      };
      
      document.body.appendChild(input);
      input.click();
    });
  }
}