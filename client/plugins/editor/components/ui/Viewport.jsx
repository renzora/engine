import React, { useRef, useEffect } from 'react';

function Viewport({ selectedTool }) {
  const canvasRef = useRef(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas) {
      const ctx = canvas.getContext('2d');
      
      const drawScene = () => {
        ctx.fillStyle = '#10b981';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
        ctx.lineWidth = 1;
        const gridSize = 50;
        
        for (let x = 0; x <= canvas.width; x += gridSize) {
          ctx.beginPath();
          ctx.moveTo(x, 0);
          ctx.lineTo(x, canvas.height);
          ctx.stroke();
        }
        
        for (let y = 0; y <= canvas.height; y += gridSize) {
          ctx.beginPath();
          ctx.moveTo(0, y);
          ctx.lineTo(canvas.width, y);
          ctx.stroke();
        }
        
        drawMockObjects(ctx);
      };
      
      const drawMockObjects = (ctx) => {
        ctx.fillStyle = '#93c5fd';
        ctx.beginPath();
        ctx.arc(400, 200, 60, 0, Math.PI * 2);
        ctx.fill();
        
        const trees = [
          { x: 150, y: 300, color: '#8b5cf6' },
          { x: 250, y: 350, color: '#f59e0b' },
          { x: 350, y: 320, color: '#ef4444' },
          { x: 450, y: 380, color: '#10b981' }
        ];
        
        trees.forEach(tree => {
          ctx.fillStyle = tree.color;
          ctx.fillRect(tree.x - 15, tree.y - 30, 30, 40);
        });
        
        ctx.fillStyle = '#dc2626';
        ctx.fillRect(500, 250, 80, 60);
        ctx.fillStyle = '#7c2d12';
        ctx.fillRect(500, 240, 80, 20);
      };
      
      canvas.width = canvas.offsetWidth;
      canvas.height = canvas.offsetHeight;
      drawScene();
      
      const handleResize = () => {
        canvas.width = canvas.offsetWidth;
        canvas.height = canvas.offsetHeight;
        drawScene();
      };
      
      window.addEventListener('resize', handleResize);
      return () => window.removeEventListener('resize', handleResize);
    }
  }, []);

  return (
    <div className="flex-1 relative bg-green-500">
      <canvas
        ref={canvasRef}
        className="w-full h-full cursor-crosshair"
        style={{ 
          background: 'linear-gradient(135deg, #10b981 0%, #059669 100%)'
        }}
      />
      
      <div className="absolute top-4 right-4 bg-gray-800 rounded-lg p-2 space-y-2">
        <div className="text-xs text-gray-400">Camera Presets</div>
        <div className="flex space-x-2">
          <button className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-xs">
            Home
          </button>
          <button className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-xs">
            Top
          </button>
          <button className="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-xs">
            Side
          </button>
        </div>
      </div>
    </div>
  );
}

export default Viewport;