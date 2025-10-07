import { BaseComponent } from './BaseComponent.jsx';
export class ParticleSystemComponent extends BaseComponent {
  getDefaultData() {
    return { maxParticles: 1000, emitRate: 100, lifetime: 5.0 };
  }
  static getSchema() {
    return { type: 'object', properties: { maxParticles: { type: 'integer' }, emitRate: { type: 'number' } } };
  }
}