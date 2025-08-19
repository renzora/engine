import { createSignal } from 'solid-js'
import { Select, Field, Stack, Title, Caption, Grid } from '@/ui'

export default function SelectDemo() {
  const [basicValue, setBasicValue] = createSignal('')
  const [searchableValue, setSearchableValue] = createSignal('')
  const [clearableValue, setClearableValue] = createSignal('option2')
  const [sizeValue, setSizeValue] = createSignal('')
  const [countryValue, setCountryValue] = createSignal('')
  
  // Basic options
  const basicOptions = [
    'Option 1',
    'Option 2', 
    'Option 3',
    'Option 4',
    'Option 5'
  ]
  
  // Complex options with objects
  const countryOptions = [
    { value: 'us', label: '🇺🇸 United States' },
    { value: 'uk', label: '🇬🇧 United Kingdom' },
    { value: 'ca', label: '🇨🇦 Canada' },
    { value: 'au', label: '🇦🇺 Australia' },
    { value: 'de', label: '🇩🇪 Germany' },
    { value: 'fr', label: '🇫🇷 France' },
    { value: 'jp', label: '🇯🇵 Japan' },
    { value: 'kr', label: '🇰🇷 South Korea' },
    { value: 'br', label: '🇧🇷 Brazil' },
    { value: 'mx', label: '🇲🇽 Mexico' },
    { value: 'in', label: '🇮🇳 India' },
    { value: 'cn', label: '🇨🇳 China' }
  ]
  
  // Long list for searchable
  const programmingLanguages = [
    'JavaScript',
    'TypeScript',
    'Python',
    'Java',
    'C++',
    'C#',
    'Ruby',
    'Go',
    'Rust',
    'Swift',
    'Kotlin',
    'PHP',
    'Scala',
    'Haskell',
    'Elixir',
    'Clojure',
    'F#',
    'Dart',
    'Lua',
    'R'
  ]
  
  return (
    <div class="p-8 max-w-4xl mx-auto">
      <Title class="text-2xl font-bold mb-6">Select Component Demo</Title>
      
      <Stack gap="lg">
        {/* Basic Select */}
        <Field 
          label="Basic Select" 
          help="Simple dropdown with string options"
        >
          <Select
            value={basicValue()}
            onChange={setBasicValue}
            options={basicOptions}
            placeholder="Choose an option..."
          />
          <Caption class="mt-2">Selected: {basicValue() || 'None'}</Caption>
        </Field>
        
        {/* Searchable Select */}
        <Field 
          label="Searchable Select" 
          help="Type to filter options"
        >
          <Select
            value={searchableValue()}
            onChange={setSearchableValue}
            options={programmingLanguages}
            placeholder="Select a programming language..."
            searchable={true}
          />
          <Caption class="mt-2">Selected: {searchableValue() || 'None'}</Caption>
        </Field>
        
        {/* Clearable Select */}
        <Field 
          label="Clearable Select" 
          help="Click the X to clear selection"
        >
          <Select
            value={clearableValue()}
            onChange={setClearableValue}
            options={basicOptions}
            placeholder="Choose an option..."
            clearable={true}
          />
          <Caption class="mt-2">Selected: {clearableValue() || 'None'}</Caption>
        </Field>
        
        {/* Country Select with Complex Options */}
        <Field 
          label="Country Select" 
          help="Options with custom labels and emojis"
        >
          <Select
            value={countryValue()}
            onChange={setCountryValue}
            options={countryOptions}
            placeholder="Select your country..."
            searchable={true}
            clearable={true}
          />
          <Caption class="mt-2">Selected: {countryValue() || 'None'}</Caption>
        </Field>
        
        {/* Different Sizes */}
        <Field label="Size Variants" help="Small, medium, and large sizes">
          <Grid cols={3} gap="md">
            <Select
              value={sizeValue()}
              onChange={setSizeValue}
              options={['Small', 'Medium', 'Large']}
              placeholder="Small size"
              size="sm"
            />
            <Select
              value={sizeValue()}
              onChange={setSizeValue}
              options={['Small', 'Medium', 'Large']}
              placeholder="Medium size"
              size="md"
            />
            <Select
              value={sizeValue()}
              onChange={setSizeValue}
              options={['Small', 'Medium', 'Large']}
              placeholder="Large size"
              size="lg"
            />
          </Grid>
        </Field>
        
        {/* Disabled State */}
        <Field label="Disabled Select" help="Cannot be interacted with">
          <Select
            value="disabled"
            options={['Disabled Option']}
            disabled={true}
          />
        </Field>
        
        {/* Position Demo */}
        <Field label="Position Control" help="Force dropdown position">
          <Grid cols={2} gap="md">
            <Select
              options={basicOptions}
              placeholder="Bottom position"
              position="bottom"
            />
            <Select
              options={basicOptions}
              placeholder="Auto position"
              position="auto"
            />
          </Grid>
        </Field>
      </Stack>
    </div>
  )
}