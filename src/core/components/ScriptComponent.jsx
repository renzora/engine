import { BaseComponent } from './BaseComponent.jsx';

export class ScriptComponent extends BaseComponent {
  getDefaultData() {
    return {
      scriptPath: null,
      enabled: true,
      variables: {}
    };
  }

  static getSchema() {
    return {
      type: 'object',
      properties: {
        scriptPath: { type: ['string', 'null'] },
        enabled: { type: 'boolean' },
        variables: { type: 'object' }
      }
    };
  }
}