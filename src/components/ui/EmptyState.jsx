import { Show } from 'solid-js'

function EmptyState({ 
  icon,
  title,
  description,
  action,
  class: className = ''
}) {
  return (
    <div class={`flex items-center justify-center my-auto py-8 ${className}`}>
      <div class="text-center">
        <Show when={icon}>
          <div class="flex justify-center mb-4">
            <div class="text-gray-500">
              {icon}
            </div>
          </div>
        </Show>
        
        <Show when={title}>
          <h3 class="text-lg font-medium text-gray-300 mb-2">
            {title}
          </h3>
        </Show>
        
        <Show when={description}>
          <p class="text-gray-500 mb-6 max-w-sm">
            {description}
          </p>
        </Show>
        
        <Show when={action}>
          {action}
        </Show>
      </div>
    </div>
  )
}

export default EmptyState