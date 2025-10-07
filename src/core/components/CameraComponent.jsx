import { BaseComponent } from './BaseComponent.jsx';
export class CameraComponent extends BaseComponent {
  getDefaultData() {
    return { cameraType: 'arcRotate', fov: 45, near: 0.1, far: 1000 };
  }
  static getSchema() {
    return { type: 'object', properties: { cameraType: { type: 'string' }, fov: { type: 'number' } } };
  }
}