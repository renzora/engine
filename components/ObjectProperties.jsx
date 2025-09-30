import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { Show, createMemo, For } from 'solid-js';

const ObjectProperties = (props) => {
  const { objectProperties } = editorStore;
  const { updateObjectProperty, bindNodeToProperty, unbindNodeFromProperty } = editorActions;
  
  const objectProps = createMemo(() => objectProperties.objects[props.objectId]);
  
  const handleDrop = (e, propertyPath) => {
    e.preventDefault();
    const droppedData = e.dataTransfer.getData('text/plain');
    
    try {
      const data = JSON.parse(droppedData);
      if (data.type === 'asset' && data.fileType === 'texture') {
        updateObjectProperty(props.objectId, propertyPath, data.path);
      } else if (data.type === 'asset' && data.fileType === 'script') {
        updateObjectProperty(props.objectId, propertyPath, data.path);
      }
    } catch (err) {
      console.warn('Invalid drop data:', droppedData);
    }
  };

  const handleDragOver = (e) => {
    e.preventDefault();
  };

  const isNodeControlled = (propertyPath) => {
    return objectProps()?.nodeBindings && objectProps().nodeBindings[propertyPath];
  };

  const renderVector3Input = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div className="grid grid-cols-3 gap-1">
        <For each={['X', 'Y', 'Z']}>
          {(axis, index) => (
            <div className="relative">
              <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">
                {axis}
              </span>
              <input
                type="number"
                step="0.1"
                value={value[index()] || 0}
                onChange={(e) => {
                  const newValue = [...value];
                  newValue[index()] = parseFloat(e.target.value) || 0;
                  updateObjectProperty(props.objectId, propertyPath, newValue);
                }}
                className={`w-full text-xs p-1.5 pl-7 pr-1.5 rounded text-center focus:outline-none focus:ring-1 focus:ring-blue-500 ${
                  isNodeControlled(`${propertyPath}.${index()}`) 
                    ? 'border-blue-500 bg-blue-900/20 text-blue-200' 
                    : 'border-gray-600 bg-gray-800 text-white'
                } border`}
                disabled={isNodeControlled(`${propertyPath}.${index()}`)}
              />
            </div>
          )}
        </For>
      </div>
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      </Show>
    </div>
  );

  const renderColorInput = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div className="flex items-center gap-1">
        <input
          type="color"
          value={value || '#ffffff'}
          onChange={(e) => updateObjectProperty(props.objectId, propertyPath, e.target.value)}
          className="w-6 h-6 rounded border border-gray-600 bg-gray-800 cursor-pointer"
          disabled={isNodeControlled(propertyPath)}
        />
        <div className={`flex-1 rounded px-1.5 py-1 border ${
          isNodeControlled(propertyPath) 
            ? 'border-blue-500 bg-blue-900/20' 
            : 'border-gray-600 bg-gray-800'
        }`}>
          <div className={`text-xs ${isNodeControlled(propertyPath) ? 'text-blue-200' : 'text-gray-300'}`}>
            {(value || '#ffffff').toUpperCase()}
          </div>
        </div>
      </div>
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      </Show>
    </div>
  );

  const renderSliderInput = (label, value, propertyPath, min = 0, max = 1, step = 0.01) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">
        {label} <span className="text-gray-500">({(value || 0).toFixed(2)})</span>
      </label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value || 0}
        onChange={(e) => updateObjectProperty(props.objectId, propertyPath, parseFloat(e.target.value))}
        className={`w-full h-1.5 bg-gray-600 rounded-lg appearance-none cursor-pointer ${
          isNodeControlled(propertyPath) ? 'opacity-50' : ''
        }`}
        disabled={isNodeControlled(propertyPath)}
      />
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      </Show>
    </div>
  );

  const renderTextureSlot = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div
        className="border-2 border-dashed border-gray-600 rounded-lg p-3 text-center hover:border-gray-500 transition-colors"
        onDrop={(e) => handleDrop(e, propertyPath)}
        onDragOver={handleDragOver}
      >
        <Show 
          when={value}
          fallback={
            <div className="text-gray-500 text-xs">
              Drop texture here or click to browse
            </div>
          }
        >
          <div className="flex items-center justify-between">
            <span className="text-xs text-gray-300 truncate">{value.split('/').pop()}</span>
            <button
              onClick={() => updateObjectProperty(props.objectId, propertyPath, null)}
              className="text-red-400 hover:text-red-300 ml-2 text-sm"
            >
              ×
            </button>
          </div>
        </Show>
      </div>
    </div>
  );

  return (
    <Show 
      when={objectProps()}
      fallback={
        <div className="p-4 text-gray-500 text-sm">
          No properties available. Open node editor to add components.
        </div>
      }
    >
      <div className="p-4 space-y-4">
        <Show when={objectProps().transform && props.objectId !== 'scene-root'}>
          <div>
            <h3 className="text-sm font-semibold text-white mb-3 border-b border-gray-600 pb-2">
              Transform
            </h3>
            {renderVector3Input('Position', objectProps().transform.position || [0, 0, 0], 'transform.position')}
            {renderVector3Input('Rotation', objectProps().transform.rotation || [0, 0, 0], 'transform.rotation')}
            {renderVector3Input('Scale', objectProps().transform.scale || [1, 1, 1], 'transform.scale')}
          </div>
        </Show>

        <Show when={objectProps().material}>
          <div>
            <h3 className="text-sm font-semibold text-white mb-3 border-b border-gray-600 pb-2">
              Material
            </h3>
            {renderColorInput('Base Color', objectProps().material.baseColor, 'material.baseColor')}
            {renderSliderInput('Roughness', objectProps().material.roughness, 'material.roughness')}
            {renderSliderInput('Metallic', objectProps().material.metallic, 'material.metallic')}
            {renderSliderInput('Alpha', objectProps().material.alpha, 'material.alpha')}
            
            <Show when={objectProps().material.textures}>
              <div className="mt-3">
                <h4 className="text-xs font-medium text-gray-300 mb-2">Textures</h4>
                {renderTextureSlot('Diffuse', objectProps().material.textures.diffuse, 'material.textures.diffuse')}
                {renderTextureSlot('Normal', objectProps().material.textures.normal, 'material.textures.normal')}
                {renderTextureSlot('Roughness', objectProps().material.textures.roughness, 'material.textures.roughness')}
                {renderTextureSlot('Metallic', objectProps().material.textures.metallic, 'material.textures.metallic')}
              </div>
            </Show>
          </div>
        </Show>

        <Show when={objectProps().rendering}>
          <div>
            <h3 className="text-sm font-semibold text-white mb-3 border-b border-gray-600 pb-2">
              Rendering
            </h3>
            <div className="space-y-2">
              <label className="flex items-center space-x-2">
                <input
                  type="checkbox"
                  checked={objectProps().rendering.castShadows}
                  onChange={(e) => updateObjectProperty(props.objectId, 'rendering.castShadows', e.target.checked)}
                  className="rounded"
                />
                <span className="text-xs text-gray-300">Cast Shadows</span>
              </label>
              <label className="flex items-center space-x-2">
                <input
                  type="checkbox"
                  checked={objectProps().rendering.receiveShadows}
                  onChange={(e) => updateObjectProperty(props.objectId, 'rendering.receiveShadows', e.target.checked)}
                  className="rounded"
                />
                <span className="text-xs text-gray-300">Receive Shadows</span>
              </label>
              {renderSliderInput('Render Order', objectProps().rendering.renderOrder, 'rendering.renderOrder', -100, 100, 1)}
            </div>
          </div>
        </Show>

        <Show when={objectProps().components}>
          <div>
            <h3 className="text-sm font-semibold text-white mb-3 border-b border-gray-600 pb-2">
              Components
            </h3>
            
            <Show when={objectProps().components.scripting}>
              <div className="mb-4">
                <h4 className="text-xs font-medium text-gray-300 mb-2">Scripting</h4>
                <label className="flex items-center space-x-2 mb-2">
                  <input
                    type="checkbox"
                    checked={objectProps().components.scripting.enabled}
                    onChange={(e) => updateObjectProperty(props.objectId, 'components.scripting.enabled', e.target.checked)}
                    className="rounded"
                  />
                  <span className="text-xs text-gray-300">Enable Scripting</span>
                </label>
                
                <div
                  className="border-2 border-dashed border-gray-600 rounded-lg p-3 text-center hover:border-gray-500 transition-colors"
                  onDrop={(e) => handleDrop(e, 'components.scripting.scriptFile')}
                  onDragOver={handleDragOver}
                >
                  <Show 
                    when={objectProps().components.scripting.scriptFile}
                    fallback={
                      <div className="text-gray-500 text-xs">
                        Drop script file here (.js, .ts, .jsx, .tsx, .ren)
                      </div>
                    }
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-gray-300 truncate">
                        {objectProps().components.scripting.scriptFile.split('/').pop()}
                      </span>
                      <button
                        onClick={() => updateObjectProperty(props.objectId, 'components.scripting.scriptFile', null)}
                        className="text-red-400 hover:text-red-300 ml-2 text-sm"
                      >
                        ×
                      </button>
                    </div>
                  </Show>
                </div>
              </div>
            </Show>

            <Show when={objectProps().components.physics}>
              <div className="mb-4">
                <h4 className="text-xs font-medium text-gray-300 mb-2">Physics</h4>
                <label className="flex items-center space-x-2 mb-2">
                  <input
                    type="checkbox"
                    checked={objectProps().components.physics.enabled}
                    onChange={(e) => updateObjectProperty(props.objectId, 'components.physics.enabled', e.target.checked)}
                    className="rounded"
                  />
                  <span className="text-xs text-gray-300">Enable Physics</span>
                </label>
                
                <select
                  value={objectProps().components.physics.type}
                  onChange={(e) => updateObjectProperty(props.objectId, 'components.physics.type', e.target.value)}
                  className="w-full px-2 py-1.5 text-xs rounded border border-gray-600 bg-gray-800 text-white focus:outline-none focus:ring-1 focus:ring-blue-500"
                  disabled={!objectProps().components.physics.enabled}
                >
                  <option value="static">Static</option>
                  <option value="dynamic">Dynamic</option>
                  <option value="kinematic">Kinematic</option>
                </select>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </Show>
  );
};

export default ObjectProperties;
