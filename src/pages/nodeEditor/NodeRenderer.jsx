import { For } from 'solid-js';
import { PortTypeColors, NodeTypeColors } from './NodeLibrary';
import { editorActions } from '@/layout/stores/EditorStore';

const NodeRenderer = (props) => {
  const { node, selectedNodes, objectId } = props;
  
  const isSelected = selectedNodes().has(node.id);
  const nodeWidth = 200;
  const inputCount = node.inputs?.length || 0;
  const outputCount = node.outputs?.length || 0;
  const nodeHeight = 40 + Math.max(inputCount, outputCount) * 25;
  const nodeTypeColor = NodeTypeColors[node.type] || '#6b7280';

  return (
    <g>
      <rect
        className="node"
        data-node-id={node.id}
        x={node.position.x}
        y={node.position.y}
        width={nodeWidth}
        height={nodeHeight}
        rx={8}
        fill={isSelected ? "#374151" : "#1f2937"}
        stroke={isSelected ? "#3b82f6" : nodeTypeColor}
        stroke-width={isSelected ? 2 : 2}
        style={{ cursor: 'move' }}
      />
      
      <rect
        x={node.position.x}
        y={node.position.y}
        width={nodeWidth}
        height={30}
        rx={8}
        fill={nodeTypeColor}
        style={{ 'pointer-events': 'none' }}
      />
      <rect
        x={node.position.x}
        y={node.position.y + 15}
        width={nodeWidth}
        height={15}
        fill={nodeTypeColor}
        style={{ 'pointer-events': 'none' }}
      />
      
      <text
        x={node.position.x + nodeWidth / 2}
        y={node.position.y + 20}
        text-anchor="middle"
        fill="#ffffff"
        font-size="13"
        font-weight="bold"
        pointer-events="none"
      >
        {node.title}
      </text>

      <circle
        cx={node.position.x + nodeWidth - 12}
        cy={node.position.y + 12}
        r={8}
        fill="#ef4444"
        stroke="#ffffff"
        stroke-width={1}
        className="node-close-button"
        style={{ cursor: 'pointer' }}
        onClick={(e) => {
          e.stopPropagation();
          console.log('Deleting node:', node.id);
          editorActions.deleteNodeAndCleanupProperties(objectId, node.id);
        }}
      />
      <text
        x={node.position.x + nodeWidth - 12}
        y={node.position.y + 16}
        text-anchor="middle"
        fill="#ffffff"
        font-size="11"
        font-weight="bold"
        pointer-events="none"
      >
        ×
      </text>

      <For each={node.inputs || []}>
        {(input, index) => {
          const portColor = PortTypeColors[input.type] || '#6b7280';
          return (
            <g>
              <circle
                className="node-port"
                data-node-id={node.id}
                data-port-id={input.id}
                data-is-input="true"
                cx={node.position.x}
                cy={node.position.y + 40 + index() * 25 + 12}
                r={6}
                fill={portColor}
                stroke="#ffffff"
                stroke-width={2}
                style={{ cursor: 'crosshair' }}
              />
              <text
                x={node.position.x + 15}
                y={node.position.y + 40 + index() * 25 + 17}
                fill="#d1d5db"
                font-size="12"
                pointer-events="none"
              >
                {input.name}
              </text>
            </g>
          );
        }}
      </For>

      <For each={node.outputs || []}>
        {(output, index) => {
          const portColor = PortTypeColors[output.type] || '#6b7280';
          return (
            <g>
              <circle
                className="node-port"
                data-node-id={node.id}
                data-port-id={output.id}
                data-is-input="false"
                cx={node.position.x + nodeWidth}
                cy={node.position.y + 40 + index() * 25 + 12}
                r={6}
                fill={portColor}
                stroke="#ffffff"
                stroke-width={2}
                style={{ cursor: 'crosshair' }}
              />
              <text
                x={node.position.x + nodeWidth - 15}
                y={node.position.y + 40 + index() * 25 + 17}
                text-anchor="end"
                fill="#d1d5db"
                font-size="12"
                pointer-events="none"
              >
                {output.name}
              </text>
            </g>
          );
        }}
      </For>
    </g>
  );
};

export default NodeRenderer;
