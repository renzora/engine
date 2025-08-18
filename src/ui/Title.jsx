import { useThemedClasses } from './hooks/useThemedClasses.js';

const titleSizes = {
  xs: 'text-lg',
  sm: 'text-xl', 
  md: 'text-2xl',
  lg: 'text-3xl',
  xl: 'text-4xl',
  xxl: 'text-5xl'
};

const titleWeights = {
  normal: 'font-normal',
  medium: 'font-medium',
  semibold: 'font-semibold',
  bold: 'font-bold'
};

export default function Title({ 
  children, 
  size = 'lg',
  weight = 'bold',
  gradient = false,
  class: className = '',
  ...props 
}) {
  const { themed, combine } = useThemedClasses();
  
  const baseClasses = combine(
    titleSizes[size],
    titleWeights[weight],
    'tracking-tight',
    gradient ? 'text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400' : themed.text.primary,
    className
  );
  
  return (
    <h1 class={baseClasses} {...props}>
      {children}
    </h1>
  );
}