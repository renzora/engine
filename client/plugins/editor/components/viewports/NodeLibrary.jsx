// Minimal Node Library - Starting with essentials
export const NodeLibrary = {
  // Input/Constant nodes
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

  // Basic Math nodes
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

  // Output nodes (these create property sections)
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

// Port type colors for visual distinction
export const PortTypeColors = {
  'float': '#3b82f6',      // Blue
  'vector2': '#10b981',    // Green  
  'vector3': '#06b6d4',    // Cyan
  'color': '#f59e0b',      // Yellow
  'texture': '#8b5cf6',    // Purple
  'boolean': '#ef4444',    // Red
  'material': '#ec4899',   // Pink
  'matrix4': '#14b8a6'     // Teal
};

// Node type colors
export const NodeTypeColors = {
  'input': '#10b981',      // Green
  'math': '#f59e0b',       // Yellow
  'vector': '#06b6d4',     // Cyan
  'utility': '#84cc16',    // Lime
  'output': '#ef4444',     // Red
  'master': '#8b5cf6'      // Purple
};