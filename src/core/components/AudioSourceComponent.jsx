import { BaseComponent } from './BaseComponent.jsx';

export class AudioSourceComponent extends BaseComponent {
  getDefaultData() {
    return {
      audioClipId: null,
      volume: 1.0,
      pitch: 1.0,
      loop: false,
      playOnStart: false,
      is3D: true
    };
  }

  static getSchema() {
    return {
      type: 'object',
      properties: {
        audioClipId: { type: ['string', 'null'] },
        volume: { type: 'number', minimum: 0, maximum: 1 },
        pitch: { type: 'number', minimum: 0.1, maximum: 3 },
        loop: { type: 'boolean' },
        playOnStart: { type: 'boolean' },
        is3D: { type: 'boolean' }
      }
    };
  }
}