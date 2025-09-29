import { createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { renderStore } from '../store.jsx';
import { Vector3, Matrix, Color3 } from '@babylonjs/core/Maths/math';

export function AxisHelper() {
  let canvasRef;
  let ctx;
  const [isInitialized, setIsInitialized] = createSignal(false);
  
  const options = {
    size: 90,
    padding: 8,
    bubbleSizePrimary: 8,
    bubbleSizeSecondary: 6,
    showSecondary: true,
    lineWidth: 2,
    fontSize: "11px",
    fontFamily: "arial",
    fontWeight: "bold",
    fontColor: "#151515",
    fontYAdjust: 0,
    colors: {
      x: ["#f73c3c", "#942424"],
      y: ["#6ccb26", "#417a17"],
      z: ["#178cf0", "#0e5490"],
    }
  };

  const bubbles = [
    { axis: "x", direction: new Vector3(1, 0, 0), size: options.bubbleSizePrimary, color: options.colors.x, line: options.lineWidth, label: "X" },
    { axis: "y", direction: new Vector3(0, 1, 0), size: options.bubbleSizePrimary, color: options.colors.y, line: options.lineWidth, label: "Y" },
    { axis: "z", direction: new Vector3(0, 0, 1), size: options.bubbleSizePrimary, color: options.colors.z, line: options.lineWidth, label: "Z" },
    { axis: "-x", direction: new Vector3(-1, 0, 0), size: options.bubbleSizeSecondary, color: options.colors.x },
    { axis: "-y", direction: new Vector3(0, -1, 0), size: options.bubbleSizeSecondary, color: options.colors.y },
    { axis: "-z", direction: new Vector3(0, 0, -1), size: options.bubbleSizeSecondary, color: options.colors.z },
  ];

  const center = new Vector3(options.size / 2, options.size / 2, 0);
  let selectedAxis = null;
  let mouse = null;

  const clear = () => {
    if (ctx) {
      ctx.clearRect(0, 0, options.size, options.size);
    }
  };

  const drawCircle = (p, radius = 10, color = "#FF0000") => {
    if (!ctx) return;
    ctx.beginPath();
    ctx.arc(p.x, p.y, radius, 0, 2 * Math.PI, false);
    ctx.fillStyle = color;
    ctx.fill();
    ctx.closePath();
  };

  const drawLine = (p1, p2, width = 1, color = "#FF0000") => {
    if (!ctx) return;
    ctx.beginPath();
    ctx.moveTo(p1.x, p1.y);
    ctx.lineTo(p2.x, p2.y);
    ctx.lineWidth = width;
    ctx.strokeStyle = color;
    ctx.stroke();
    ctx.closePath();
  };

  const getBubblePosition = (position) => {
    return new Vector3(
      (position.x * (center.x - (options.bubbleSizePrimary / 2) - options.padding)) + center.x,
      center.y - (position.y * (center.y - (options.bubbleSizePrimary / 2) - options.padding)),
      position.z
    );
  };

  const onMouseMove = (evt) => {
    const rect = canvasRef.getBoundingClientRect();
    mouse = new Vector3(evt.clientX - rect.left, evt.clientY - rect.top, 0);
    update();
  };

  const onMouseOut = () => {
    mouse = null;
    selectedAxis = null;
    update();
  };

  const onMouseClick = () => {
    if (!selectedAxis || !renderStore.camera) return;
    
    const camera = renderStore.camera;
    const vec = selectedAxis.direction.clone();
    
    // Calculate distance to maintain current camera distance from target
    const currentTarget = camera.getTarget ? camera.getTarget() : Vector3.Zero();
    const currentDistance = Vector3.Distance(camera.position, currentTarget);
    
    // Position camera at the selected axis direction
    vec.scaleInPlace(currentDistance);
    const newPosition = currentTarget.add(vec);
    
    // Smooth animation to new position
    const startPosition = camera.position.clone();
    const animationDuration = 500;
    const startTime = Date.now();
    
    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / animationDuration, 1);
      
      // Smooth easing function (ease-out)
      const easedProgress = 1 - Math.pow(1 - progress, 3);
      
      // Interpolate position
      camera.position = Vector3.Lerp(startPosition, newPosition, easedProgress);
      
      // Set target
      if (camera.setTarget) {
        camera.setTarget(currentTarget);
      }
      
      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };
    
    requestAnimationFrame(animate);
  };

  const update = () => {
    const camera = renderStore.camera;
    if (!camera || !ctx) return;

    clear();

    // Calculate the rotation matrix from the camera
    let rotMat = new Matrix();
    camera.absoluteRotation.toRotationMatrix(rotMat);
    let invRotMat = rotMat.clone().invert();

    // Update bubble positions
    for (let bubble of bubbles) {
      const invRotVec = Vector3.TransformCoordinates(bubble.direction.clone(), invRotMat);
      bubble.position = getBubblePosition(invRotVec);
    }

    // Generate layers to draw
    const layers = [];
    for (let bubble of bubbles) {
      if (options.showSecondary === true || !bubble.axis.startsWith("-")) {
        layers.push(bubble);
      }
    }

    // Sort layers by Z position
    layers.sort((a, b) => (a.position.z > b.position.z) ? 1 : -1);

    // Find closest axis if mouse is present
    selectedAxis = null;
    if (mouse) {
      let closestDist = Infinity;
      for (let bubble of layers) {
        const distance = Vector3.Distance(mouse, bubble.position);
        if (distance < closestDist || distance < bubble.size) {
          closestDist = distance;
          selectedAxis = bubble;
        }
      }
    }

    // Draw layers
    for (let bubble of layers) {
      let color = bubble.color;

      // Determine color based on selection and position
      if (selectedAxis === bubble) {
        color = "#FFFFFF";
      } else if (bubble.position.z >= -0.01) {
        color = bubble.color[0];
      } else {
        color = bubble.color[1];
      }

      // Draw circle
      drawCircle(bubble.position, bubble.size, color);

      // Draw line to center
      if (bubble.line) {
        drawLine(center, bubble.position, bubble.line, color);
      }

      // Draw label
      if (bubble.label) {
        ctx.font = [options.fontWeight, options.fontSize, options.fontFamily].join(" ");
        ctx.fillStyle = options.fontColor;
        ctx.textBaseline = 'middle';
        ctx.textAlign = 'center';
        ctx.fillText(bubble.label, bubble.position.x, bubble.position.y + options.fontYAdjust);
      }
    }
  };

  onMount(() => {
    if (canvasRef) {
      ctx = canvasRef.getContext("2d");
      setIsInitialized(true);
      
      canvasRef.addEventListener('mousemove', onMouseMove);
      canvasRef.addEventListener('mouseout', onMouseOut);
      canvasRef.addEventListener('click', onMouseClick);
    }
  });

  onCleanup(() => {
    if (canvasRef) {
      canvasRef.removeEventListener('mousemove', onMouseMove);
      canvasRef.removeEventListener('mouseout', onMouseOut);
      canvasRef.removeEventListener('click', onMouseClick);
    }
  });

  // Update when camera changes
  createEffect(() => {
    if (isInitialized() && renderStore.camera) {
      update();
    }
  });

  // Update on render loop
  createEffect(() => {
    const scene = renderStore.scene;
    if (scene && isInitialized()) {
      const renderLoop = () => {
        update();
      };
      scene.registerBeforeRender(renderLoop);
      
      onCleanup(() => {
        if (scene && !scene.isDisposed) {
          scene.unregisterBeforeRender(renderLoop);
        }
      });
    }
  });

  return (
    <div className="absolute bottom-4 right-4 pointer-events-auto">
      <canvas
        ref={canvasRef}
        width={options.size}
        height={options.size}
        className="cursor-pointer"
        style={{
          width: `${options.size}px`,
          height: `${options.size}px`
        }}
      />
    </div>
  );
}