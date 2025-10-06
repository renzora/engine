import { IconSphere, IconCube, IconSquare, IconCircle, IconHexagon } from '@tabler/icons-solidjs';

export default function ShapeSelector(props) {
  const { previewShape, setPreviewShape } = props;

  return (
    <div class="absolute bottom-2 left-2 flex gap-2">
      <button
        class={`btn btn-sm ${previewShape() === 'sphere' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
        onClick={() => setPreviewShape('sphere')}
        title="Sphere"
      >
        <IconSphere class="w-4 h-4" />
      </button>
      <button
        class={`btn btn-sm ${previewShape() === 'cube' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
        onClick={() => setPreviewShape('cube')}
        title="Cube"
      >
        <IconCube class="w-4 h-4" />
      </button>
      <button
        class={`btn btn-sm ${previewShape() === 'plane' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
        onClick={() => setPreviewShape('plane')}
        title="Plane"
      >
        <IconSquare class="w-4 h-4" />
      </button>
      <button
        class={`btn btn-sm ${previewShape() === 'cylinder' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
        onClick={() => setPreviewShape('cylinder')}
        title="Cylinder"
      >
        <IconCircle class="w-4 h-4" />
      </button>
      <button
        class={`btn btn-sm ${previewShape() === 'torus' ? 'btn-primary' : 'btn-ghost'} bg-opacity-80 backdrop-blur-sm`}
        onClick={() => setPreviewShape('torus')}
        title="Torus"
      >
        <IconHexagon class="w-4 h-4" />
      </button>
    </div>
  );
}