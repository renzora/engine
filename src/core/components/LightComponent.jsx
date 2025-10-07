import { BaseComponent } from './BaseComponent.jsx';
export class LightComponent extends BaseComponent {
  getDefaultData() {
    return { lightType: 'directional', intensity: 1.0, color: [1, 1, 1] };
  }
  static getSchema() {
    return { type: 'object', properties: { lightType: { type: 'string' }, intensity: { type: 'number' } } };
  }
}