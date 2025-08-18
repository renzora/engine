import { createSignal, createEffect, onCleanup } from 'solid-js'
import { Search, Refresh, X } from '@/ui/icons'

function SearchInput({ 
  value = '',
  onInput,
  onSearch,
  placeholder = "Search...",
  debounceMs = 300,
  loading = false,
  class: className = '',
  showShortcuts = true
}) {
  const [localValue, setLocalValue] = createSignal(value)
  let searchTimeout
  
  createEffect(() => {
    if (value !== localValue()) {
      setLocalValue(value)
    }
  })
  
  createEffect(() => {
    const query = localValue()
    
    clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => {
      if (onSearch) onSearch(query)
    }, debounceMs)
  })
  
  onCleanup(() => {
    clearTimeout(searchTimeout)
  })
  
  const handleInput = (e) => {
    const newValue = e.target.value
    setLocalValue(newValue)
    if (onInput) onInput(newValue)
  }
  
  const handleClear = () => {
    setLocalValue('')
    if (onSearch) onSearch('')
    if (onInput) onInput('')
  }
  
  const handleKeyDown = (e) => {
    if (e.key === 'Escape') {
      handleClear()
    } else if (e.key === 'Enter') {
      clearTimeout(searchTimeout)
      if (onSearch) onSearch(localValue())
    }
  }
  
  return (
    <div class={`relative ${className}`}>
      <div class="relative">
        <Search class="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400" />
        
        <input
          type="text"
          value={localValue()}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          class="
            w-full pl-9 pr-10 py-2 
            bg-slate-700/50 border border-gray-600 rounded-lg
            text-gray-200 placeholder-gray-400
            focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500
            transition-colors
          "
        />
        
        <div class="absolute right-3 top-1/2 transform -translate-y-1/2 flex items-center gap-1">
          {loading && (
            <Refresh class="w-4 h-4 text-blue-500 animate-spin" />
          )}
          
          {localValue() && (
            <button
              onClick={handleClear}
              class="p-0.5 hover:bg-slate-600 rounded transition-colors"
              title="Clear search"
            >
              <X class="w-3 h-3 text-gray-400 hover:text-gray-200" />
            </button>
          )}
        </div>
      </div>
      
      {showShortcuts && !localValue() && (
        <div class="absolute right-3 top-1/2 transform -translate-y-1/2 text-xs text-gray-500">
          <kbd class="px-1.5 py-0.5 bg-slate-600 rounded text-xs">⏎</kbd>
        </div>
      )}
    </div>
  )
}

export default SearchInput
