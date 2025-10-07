import { BaseComponent } from './BaseComponent.jsx';

export class MaterialComponent extends BaseComponent {
  getDefaultData() {
    return {
      materialType: 'standard',
      diffuseColor: [1, 1, 1],
      specularColor: [1, 1, 1],
      emissiveColor: [0, 0, 0],
      ambientColor: [0, 0, 0]
    };
  }

  static getSchema() {
    return {
      type: 'object',
      properties: {
        materialType: { type: 'string', enum: ['standard', 'pbr', 'unlit'] },
        diffuseColor: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 }
      }
    };
  }
}