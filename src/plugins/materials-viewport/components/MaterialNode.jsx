import { Show, For } from 'solid-js';
import { IconX } from '@tabler/icons-solidjs';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';
import TexturePreview from './TexturePreview.jsx';

export default function MaterialNode(props) {
  const { 
    node, 
    selectedNode, 
    draggedNode, 
    draggedNodeTransform, 
    handleNodeMouseDown, 
    handleSocketMouseDown, 
    handleSocketMouseEnter, 
    handleSocketMouseLeave, 
    removeNode,
    hoveredSocket,
    nodes,
    setNodes,
    createMaterialFromNodes
  } = props;

  const isDragged = () => draggedNode()?.id === node.id;
  const getPosition = () => isDragged() ? draggedNodeTransform() : node.position;

  return (
    <div
      class={`absolute bg-base-200 border shadow-lg min-w-44 rounded-lg overflow-hidden ${
        selectedNode() === node ? 'border-primary ring-2 ring-primary/30' : 'border-base-300 hover:border-base-content/20'
      } ${isDragged() ? '' : 'transition-all duration-200'}`}
      style={{
        left: `${getPosition().x}px`,
        top: `${getPosition().y}px`,
        'z-index': isDragged() ? '1000' : 'auto'
      }}
    >
      {/* Node Header */}
      <div 
        class={`px-3 py-1.5 text-sm font-medium cursor-grab select-none transition-all flex items-center gap-2 ${
          selectedNode() === node ? 'text-primary bg-primary/20' : 'text-base-content/80 hover:text-base-content bg-base-300 hover:bg-base-300/80'
        }`}
        onMouseDown={(e) => handleNodeMouseDown(e, node)}
      >
        {/* Type indicator */}
        <div class={`w-2 h-2 rounded-sm ${
          node.type === 'MaterialOutput' ? 'bg-success' :
          node.type === 'TextureSample' ? 'bg-info' :
          node.type === 'Constant' ? 'bg-warning' :
          'bg-neutral'
        }`}></div>
        <span class="truncate flex-1">{node.title}</span>
        
        {/* Delete button - only show for non-output nodes */}
        <Show when={node.id !== 'material-output'}>
          <button
            class="w-4 h-4 flex items-center justify-center rounded hover:bg-error/20 hover:text-error transition-colors relative z-10"
            style="pointer-events: auto;"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
            }}
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              removeNode(node.id);
            }}
            onPointerDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
            }}
            title="Delete Node"
          >
            <IconX class="w-3 h-3" />
          </button>
        </Show>
        
        {/* Active indicator line */}
        <Show when={selectedNode() === node}>
          <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"></div>
        </Show>
      </div>
      
      {/* Node Body */}
      <div class="bg-base-100 p-2 space-y-1">
        {/* Texture Preview */}
        <Show when={node.type === 'TextureSample' && node.asset}>
          <TexturePreview asset={node.asset} />
        </Show>
        
        {/* Inputs */}
        <Show when={node.inputs?.length > 0}>
          <For each={node.inputs}>
            {(input) => (
              <div class="flex items-center justify-between py-0.5 group">
                <div class="flex items-center gap-1.5 flex-1 min-w-0">
                  <div 
                    class={`w-2.5 h-2.5 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                      hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === input.id
                        ? 'border-primary scale-110'
                        : 'border-base-400 hover:border-primary'
                    }`}
                    style={{
                      'background-color': 
                        input.type === 'color' ? '#ff6b6b' :
                        input.type === 'float' ? '#51cf66' :
                        input.type === 'vector' ? '#339af0' :
                        input.type === 'texture' ? '#ffd43b' :
                        '#6c757d',
                      position: 'relative',
                      'z-index': '10'
                    }}
                    data-socket={`${node.id}-${input.id}-input`}
                    onMouseDown={(e) => handleSocketMouseDown(e, node.id, input, 'input')}
                    onMouseEnter={() => handleSocketMouseEnter(node.id, input, 'input')}
                    onMouseLeave={handleSocketMouseLeave}
                    onClick={(e) => {
                      e.stopPropagation();
                    }}
                  ></div>
                  <span class="text-xs text-base-content/80 truncate">{input.name}</span>
                </div>
                <Show when={input.type === 'float' && input.value !== null}>
                  <input
                    type="number"
                    class="input input-xs w-14 text-right text-xs border-base-300 bg-base-200"
                    value={input.value}
                    step="0.1"
                    onChange={(e) => {
                      const newNodes = nodes().map(n => 
                        n.id === node.id 
                          ? {
                              ...n,
                              inputs: n.inputs.map(i =>
                                i.id === input.id 
                                  ? { ...i, value: parseFloat(e.target.value) }
                                  : i
                              )
                            }
                          : n
                      );
                      setNodes(newNodes);
                      createMaterialFromNodes();
                    }}
                  />
                </Show>
                <Show when={input.type === 'color' && input.value !== null && input.value instanceof Color3}>
                  <input
                    type="color"
                    class="w-8 h-6 rounded border border-base-300 cursor-pointer"
                    value={`#${Math.round(input.value.r * 255).toString(16).padStart(2, '0')}${Math.round(input.value.g * 255).toString(16).padStart(2, '0')}${Math.round(input.value.b * 255).toString(16).padStart(2, '0')}`}
                    onChange={(e) => {
                      const hex = e.target.value;
                      const r = parseInt(hex.slice(1, 3), 16) / 255;
                      const g = parseInt(hex.slice(3, 5), 16) / 255;
                      const b = parseInt(hex.slice(5, 7), 16) / 255;
                      const color = new Color3(r, g, b);
                      
                      const newNodes = nodes().map(n => 
                        n.id === node.id 
                          ? {
                              ...n,
                              inputs: n.inputs.map(i =>
                                i.id === input.id 
                                  ? { ...i, value: color }
                                  : i
                              )
                            }
                          : n
                      );
                      setNodes(newNodes);
                      createMaterialFromNodes();
                    }}
                  />
                </Show>
              </div>
            )}
          </For>
        </Show>
        
        {/* Outputs */}
        <Show when={node.outputs?.length > 0}>
          <For each={node.outputs}>
            {(output) => (
              <div class="flex items-center justify-between py-0.5 group">
                <div class="flex items-center gap-1.5 flex-1 min-w-0">
                  <span class="text-xs text-base-content/80 truncate">{output.name}</span>
                </div>
                <div 
                  class={`w-3 h-3 rounded-full border cursor-pointer transition-all duration-200 pointer-events-auto flex-shrink-0 ${
                    hoveredSocket()?.nodeId === node.id && hoveredSocket()?.socketId === output.id
                      ? 'border-primary scale-125 shadow-lg shadow-primary/50'
                      : 'border-base-400 hover:border-primary hover:scale-110'
                  }`}
                  style={{
                    'background-color': 
                      output.type === 'color' ? '#ff6b6b' :
                      output.type === 'float' ? '#51cf66' :
                      output.type === 'vector' ? '#339af0' :
                      output.type === 'texture' ? '#ffd43b' :
                      '#6c757d',
                    position: 'relative',
                    'z-index': '10'
                  }}
                  data-socket={`${node.id}-${output.id}-output`}
                  onMouseDown={(e) => handleSocketMouseDown(e, node.id, output, 'output')}
                  onMouseEnter={() => handleSocketMouseEnter(node.id, output, 'output')}
                  onMouseLeave={handleSocketMouseLeave}
                  onClick={(e) => {
                    e.stopPropagation();
                  }}
                ></div>
              </div>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}