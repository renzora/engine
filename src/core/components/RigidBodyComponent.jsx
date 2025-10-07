import { BaseComponent } from './BaseComponent.jsx';

export class RigidBodyComponent extends BaseComponent {
  getDefaultData() {
    return {
      mass: 1.0,
      friction: 0.5,
      restitution: 0.0,
      isKinematic: false,
      shape: 'box'
    };
  }

  static getSchema() {
    return {
      type: 'object',
      properties: {
        mass: { type: 'number', minimum: 0 },
        friction: { type: 'number', minimum: 0, maximum: 1 },
        restitution: { type: 'number', minimum: 0, maximum: 1 },
        isKinematic: { type: 'boolean' },
        shape: { type: 'string', enum: ['box', 'sphere', 'cylinder', 'mesh'] }
      }
    };
  }
}