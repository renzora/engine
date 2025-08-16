import { createSignal, onMount, onCleanup } from 'solid-js';

export default function NeonGridBackground() {
  let containerRef;
  const [shapes, setShapes] = createSignal([]);

  const createMovingShapes = () => {
    const newShapes = [];
    const shapeCount = 12;
    
    for (let i = 0; i < shapeCount; i++) {
      const shape = {
        id: i,
        type: ['circle', 'square', 'triangle', 'diamond'][i % 4],
        x: Math.random() * 100,
        y: Math.random() * 100,
        size: 15 + Math.random() * 35,
        speed: 0.3 + Math.random() * 0.7,
        direction: Math.random() * 360,
        color: [
          '#00ffff', // electric cyan
          '#ff00ff', // electric magenta
          '#ffff00', // electric yellow
          '#ff3300', // electric red
          '#00ff66', // electric green
          '#6600ff', // electric purple
          '#ff0099', // electric pink
          '#99ff00', // electric lime
          '#ff6600', // electric orange
          '#0099ff'  // electric blue
        ][i % 10],
        opacity: 0.6 + Math.random() * 0.4,
        rotation: 0,
        rotationSpeed: (Math.random() - 0.5) * 2
      };
      newShapes.push(shape);
    }
    setShapes(newShapes);
  };

  const animateShapes = () => {
    setShapes(prev => prev.map(shape => {
      let newX = shape.x + Math.cos(shape.direction * Math.PI / 180) * shape.speed;
      let newY = shape.y + Math.sin(shape.direction * Math.PI / 180) * shape.speed;
      
      // Bounce off edges
      if (newX < -5 || newX > 105) {
        shape.direction = 180 - shape.direction;
        newX = Math.max(-5, Math.min(105, newX));
      }
      if (newY < -5 || newY > 105) {
        shape.direction = -shape.direction;
        newY = Math.max(-5, Math.min(105, newY));
      }
      
      return {
        ...shape,
        x: newX,
        y: newY,
        rotation: shape.rotation + shape.rotationSpeed
      };
    }));
  };

  onMount(() => {
    createMovingShapes();
    
    const animationInterval = setInterval(animateShapes, 16); // ~60fps
    
    onCleanup(() => {
      clearInterval(animationInterval);
    });
  });

  const renderShape = (shape) => {
    const baseStyle = {
      position: 'absolute',
      left: `${shape.x}%`,
      top: `${shape.y}%`,
      width: `${shape.size}px`,
      height: `${shape.size}px`,
      transform: `translate(-50%, -50%) rotate(${shape.rotation}deg)`,
      'box-shadow': `0 0 20px ${shape.color}, 0 0 40px ${shape.color}, 0 0 60px ${shape.color}`,
      opacity: shape.opacity,
      'z-index': 2
    };

    switch (shape.type) {
      case 'circle':
        return (
          <div
            key={shape.id}
            style={{
              ...baseStyle,
              background: `radial-gradient(circle, ${shape.color}40, ${shape.color}80)`,
              'border-radius': '50%',
              border: `2px solid ${shape.color}`
            }}
          />
        );
      case 'square':
        return (
          <div
            key={shape.id}
            style={{
              ...baseStyle,
              background: `linear-gradient(45deg, ${shape.color}40, ${shape.color}80)`,
              border: `2px solid ${shape.color}`
            }}
          />
        );
      case 'triangle':
        return (
          <div
            key={shape.id}
            style={{
              ...baseStyle,
              width: 0,
              height: 0,
              'border-left': `${shape.size/2}px solid transparent`,
              'border-right': `${shape.size/2}px solid transparent`,
              'border-bottom': `${shape.size}px solid ${shape.color}`,
              'box-shadow': `0 0 20px ${shape.color}, 0 0 40px ${shape.color}`,
              background: 'transparent'
            }}
          />
        );
      case 'diamond':
        return (
          <div
            key={shape.id}
            style={{
              ...baseStyle,
              background: `linear-gradient(45deg, ${shape.color}40, ${shape.color}80)`,
              border: `2px solid ${shape.color}`,
              transform: `translate(-50%, -50%) rotate(${shape.rotation + 45}deg)`
            }}
          />
        );
      default:
        return null;
    }
  };

  return (
    <div 
      ref={containerRef}
      class="absolute inset-0 w-full h-full overflow-hidden"
      style={{ 'z-index': 1 }}
    >
      {/* Animated Grid Background */}
      <div 
        class="absolute inset-0 w-full h-full"
        style={{
          background: `
            linear-gradient(rgba(0, 255, 255, 0.15) 1px, transparent 1px),
            linear-gradient(90deg, rgba(0, 255, 255, 0.15) 1px, transparent 1px),
            linear-gradient(rgba(255, 0, 255, 0.1) 1px, transparent 1px),
            linear-gradient(90deg, rgba(255, 0, 255, 0.1) 1px, transparent 1px),
            linear-gradient(rgba(255, 255, 0, 0.05) 1px, transparent 1px),
            linear-gradient(90deg, rgba(255, 255, 0, 0.05) 1px, transparent 1px),
            radial-gradient(circle at 25% 25%, rgba(0, 255, 255, 0.2) 0%, transparent 40%),
            radial-gradient(circle at 75% 75%, rgba(255, 0, 255, 0.2) 0%, transparent 40%),
            radial-gradient(circle at 50% 50%, rgba(255, 255, 0, 0.1) 0%, transparent 60%)
          `,
          'background-size': '40px 40px, 40px 40px, 80px 80px, 80px 80px, 120px 120px, 120px 120px, 200px 200px, 200px 200px, 400px 400px',
          animation: 'gridMove 15s linear infinite, gridPulse 3s ease-in-out infinite alternate'
        }}
      />
      
      {/* Grid Lines with Glow */}
      <div 
        class="absolute inset-0 w-full h-full"
        style={{
          background: `
            linear-gradient(rgba(0, 255, 255, 0.4) 2px, transparent 2px),
            linear-gradient(90deg, rgba(0, 255, 255, 0.4) 2px, transparent 2px),
            linear-gradient(rgba(255, 0, 255, 0.2) 1px, transparent 1px),
            linear-gradient(90deg, rgba(255, 0, 255, 0.2) 1px, transparent 1px)
          `,
          'background-size': '100px 100px, 100px 100px, 50px 50px, 50px 50px',
          animation: 'gridGlow 2s ease-in-out infinite alternate, gridShift 8s linear infinite'
        }}
      />

      {/* Moving Shapes */}
      <div class="absolute inset-0 w-full h-full">
        {shapes().map(renderShape)}
      </div>

      {/* Simplified overlay gradient */}
      <div 
        class="absolute inset-0 w-full h-full"
        style={{
          background: `
            radial-gradient(circle at 30% 30%, rgba(0, 255, 255, 0.08) 0%, transparent 50%),
            radial-gradient(circle at 70% 70%, rgba(255, 0, 255, 0.08) 0%, transparent 50%)
          `,
          animation: 'overlayMove 20s linear infinite'
        }}
      />
      
      {/* No vignette for maximum clarity */}

      <style>{`
        @keyframes gridMove {
          0% { transform: translate(0, 0); }
          100% { transform: translate(40px, 40px); }
        }
        
        @keyframes gridPulse {
          0% { opacity: 0.6; }
          100% { opacity: 1; }
        }
        
        @keyframes gridGlow {
          0% { 
            filter: brightness(1) drop-shadow(0 0 10px rgba(0, 255, 255, 0.6));
          }
          100% { 
            filter: brightness(1.3) drop-shadow(0 0 15px rgba(0, 255, 255, 0.8));
          }
        }
        
        @keyframes gridShift {
          0% { transform: translate(0, 0) rotate(0deg); }
          20% { transform: translate(8px, -12px) rotate(0.8deg); }
          40% { transform: translate(-6px, 18px) rotate(-1.2deg); }
          60% { transform: translate(12px, 8px) rotate(0.6deg); }
          80% { transform: translate(-4px, -8px) rotate(-0.4deg); }
          100% { transform: translate(0, 0) rotate(0deg); }
        }
        
        @keyframes overlayMove {
          0% { transform: rotate(0deg) scale(1) translateX(0px); }
          25% { transform: rotate(90deg) scale(1.05) translateX(10px); }
          50% { transform: rotate(180deg) scale(0.95) translateX(-5px); }
          75% { transform: rotate(270deg) scale(1.1) translateX(15px); }
          100% { transform: rotate(360deg) scale(1) translateX(0px); }
        }
        
        @keyframes overlayPulse {
          0% { opacity: 0.6; }
          100% { opacity: 1; }
        }
        
        @keyframes scanlines {
          0% { transform: translateY(0px); }
          100% { transform: translateY(8px); }
        }
      `}</style>
    </div>
  );
}