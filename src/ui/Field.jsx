import { Show } from 'solid-js';
import Input from './Input';
import Textarea from './Textarea';
import Select from './Select';

const Field = (props) => {
  const renderInput = () => {
    const commonProps = {
      error: !!props.error,
      success: !!props.success,
      ...props
    };

    const type = props.type || 'input';
    switch (type) {
      case 'textarea':
        return <Textarea {...commonProps} />;
      case 'select':
        return <Select {...commonProps} />;
      default:
        return <Input {...commonProps} />;
    }
  };

  const required = props.required || false;
  const orientation = props.orientation || 'vertical';
  const className = props.className || '';
  const labelClassName = props.labelClassName || '';
  const isHorizontal = orientation === 'horizontal';

  return (
    <div class={`${isHorizontal ? 'flex items-center gap-3' : 'space-y-2'} ${className}`}>
      <Show when={props.label}>
        <div class={isHorizontal ? 'flex-shrink-0' : ''}>
          <label class={`
            text-xs font-medium text-gray-300 uppercase tracking-wide
            ${required ? "after:content-['*'] after:text-red-400 after:ml-1" : ''}
            ${labelClassName}
          `}>
            {props.label}
          </label>
          <Show when={props.description && !isHorizontal}>
            <div class="text-xs text-gray-500 mt-0.5">{props.description}</div>
          </Show>
        </div>
      </Show>
      
      <div class={isHorizontal ? 'flex-1' : ''}>
        {props.children || renderInput()}
        
        <Show when={props.description && isHorizontal}>
          <div class="text-xs text-gray-500 mt-1">{props.description}</div>
        </Show>
        
        <Show when={props.error}>
          <div class="text-xs text-red-400 mt-1">{props.error}</div>
        </Show>
        
        <Show when={props.success}>
          <div class="text-xs text-green-400 mt-1">{props.success}</div>
        </Show>
      </div>
    </div>
  );
};

export default Field;
