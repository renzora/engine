import { BaseComponent } from './BaseComponent.jsx';
export class AnimationComponent extends BaseComponent {
  getDefaultData() {
    return { animationClipId: null, autoPlay: false, loop: true, speed: 1.0 };
  }
  static getSchema() {
    return { type: 'object', properties: { animationClipId: { type: ['string', 'null'] }, autoPlay: { type: 'boolean' } } };
  }
}