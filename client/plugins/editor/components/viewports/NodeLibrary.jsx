export const NodeLibrary = {
  'vector3-constant': {
    type: 'input',
    title: 'Vector3',
    color: '#10b981',
    outputs: [
      { id: 'value', name: 'XYZ', type: 'vector3', value: [0, 0, 0] }
    ],
    properties: [
      { id: 'x', name: 'X', type: 'float', value: 0.0 },
      { id: 'y', name: 'Y', type: 'float', value: 0.0 },
      { id: 'z', name: 'Z', type: 'float', value: 0.0 }
    ]
  },

  'float-constant': {
    type: 'input',
    title: 'Float',
    color: '#10b981',
    outputs: [
      { id: 'value', name: 'Value', type: 'float', value: 0.0 }
    ],
    properties: [
      { id: 'value', name: 'Value', type: 'float', value: 0.0 }
    ]
  },

  'color-constant': {
    type: 'input',
    title: 'Color',
    color: '#10b981',
    outputs: [
      { id: 'value', name: 'Color', type: 'color', value: '#ffffff' }
    ],
    properties: [
      { id: 'value', name: 'Color', type: 'color', value: '#ffffff' }
    ]
  },

  'add': {
    type: 'math',
    title: 'Add',
    color: '#f59e0b',
    inputs: [
      { id: 'a', name: 'A', type: 'float' },
      { id: 'b', name: 'B', type: 'float' }
    ],
    outputs: [
      { id: 'result', name: 'Result', type: 'float' }
    ]
  },

  'multiply': {
    type: 'math',
    title: 'Multiply',
    color: '#f59e0b',
    inputs: [
      { id: 'a', name: 'A', type: 'float' },
      { id: 'b', name: 'B', type: 'float' }
    ],
    outputs: [
      { id: 'result', name: 'Result', type: 'float' }
    ]
  },

  'material-output': {
    type: 'output',
    title: 'Material Output',
    color: '#ef4444',
    inputs: [
      { id: 'base-color', name: 'Base Color', type: 'color' },
      { id: 'roughness', name: 'Roughness', type: 'float' },
      { id: 'metallic', name: 'Metallic', type: 'float' }
    ]
  },

  'transform-output': {
    type: 'output',
    title: 'Transform Output',
    color: '#ef4444',
    inputs: [
      { id: 'position', name: 'Position', type: 'vector3' },
      { id: 'rotation', name: 'Rotation', type: 'vector3' },
      { id: 'scale', name: 'Scale', type: 'vector3' }
    ]
  }
};

export const PortTypeColors = {
  'float': '#3b82f6',
  'vector2': '#10b981',
  'vector3': '#06b6d4',
  'color': '#f59e0b',
  'texture': '#8b5cf6',
  'boolean': '#ef4444',
  'material': '#ec4899',
  'matrix4': '#14b8a6'
};

export const NodeTypeColors = {
  'input': '#10b981',
  'math': '#f59e0b',
  'vector': '#06b6d4',
  'utility': '#84cc16',
  'output': '#ef4444',
  'master': '#8b5cf6'
};