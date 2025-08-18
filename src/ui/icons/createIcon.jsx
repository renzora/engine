export const createIcon = (path, viewBox = "0 0 24 24") => (props) => (
  <svg 
    {...props} 
    viewBox={viewBox} 
    fill="currentColor"
    class={`w-4 h-4 ${props.class || ''}`}
    style={props.style}
  >
    <path d={path} />
  </svg>
);
