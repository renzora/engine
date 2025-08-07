import { useState } from 'react';
import { 
  Section, 
  Field, 
  Toggle, 
  ColorPicker, 
  Button, 
  Slider,
  Forum,
  ThemeProvider 
} from '../index';

const UIShowcase = () => {
  const [toggleValue, setToggleValue] = useState(false);
  const [colorValue, setColorValue] = useState('#3b82f6');
  const [sliderValue, setSliderValue] = useState(50);
  const [threads, setThreads] = useState([
    {
      id: 1,
      title: 'Welcome to the Forum!',
      content: 'This is a sample thread to showcase the forum component.',
      author: 'Admin',
      timestamp: new Date().toISOString(),
      replies: [
        {
          id: 1,
          content: 'Thanks for the welcome!',
          author: 'User1',
          timestamp: new Date().toISOString()
        }
      ]
    }
  ]);

  const handleNewThread = (thread) => {
    setThreads(prev => [thread, ...prev]);
  };

  const handleReply = (threadId, reply) => {
    setThreads(prev => prev.map(thread => 
      thread.id === threadId 
        ? { ...thread, replies: [...(thread.replies || []), reply] }
        : thread
    ));
  };

  return (
    <ThemeProvider>
      <div className="min-h-screen bg-slate-900 p-8">
        <div className="max-w-6xl mx-auto space-y-8">
          <h1 className="text-3xl font-bold text-white mb-8">UI Component Showcase</h1>
          
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
            
            {/* Form Components */}
            <Section title="Form Components" defaultOpen={true} variant="accent">
              <div className="space-y-4">
                <Field 
                  label="Text Input"
                  placeholder="Enter some text..."
                />
                
                <Field 
                  label="Email Input"
                  type="email"
                  placeholder="user@example.com"
                />
                
                <Field 
                  label="Description"
                  type="textarea"
                  rows={3}
                  placeholder="Enter a description..."
                />
                
                <Field 
                  label="Category"
                  type="select"
                  options={['Option 1', 'Option 2', 'Option 3']}
                />
                
                <Field label="Favorite Color">
                  <ColorPicker 
                    value={colorValue}
                    onChange={setColorValue}
                    showValue={true}
                  />
                </Field>
              </div>
            </Section>

            {/* Interactive Components */}
            <Section title="Interactive Components" defaultOpen={true} variant="subtle">
              <div className="space-y-4">
                <Toggle
                  label="Enable Features"
                  description="This toggle controls various features"
                  checked={toggleValue}
                  onChange={setToggleValue}
                />
                
                <div className="space-y-2">
                  <label className="text-xs font-medium text-gray-300 uppercase tracking-wide">
                    Slider Value: {sliderValue}
                  </label>
                  <Slider
                    value={sliderValue}
                    onChange={setSliderValue}
                    min={0}
                    max={100}
                    step={1}
                  />
                </div>
                
                <div className="flex gap-2 flex-wrap">
                  <Button variant="primary">Primary</Button>
                  <Button variant="secondary">Secondary</Button>
                  <Button variant="outline">Outline</Button>
                  <Button variant="ghost">Ghost</Button>
                </div>
                
                <div className="flex gap-2 flex-wrap">
                  <Button variant="success">Success</Button>
                  <Button variant="warning">Warning</Button>
                  <Button variant="error">Error</Button>
                </div>
              </div>
            </Section>

            {/* Sections Demo */}
            <Section title="Section Variants" defaultOpen={true}>
              <div className="space-y-4">
                <Section title="Default Section" index={0} defaultOpen={false}>
                  <p className="text-sm text-gray-300">Content in default section</p>
                </Section>
                
                <Section title="Accent Section" variant="accent" defaultOpen={false}>
                  <p className="text-sm text-gray-300">Content in accent section</p>
                </Section>
                
                <Section title="Subtle Section" variant="subtle" defaultOpen={false}>
                  <p className="text-sm text-gray-300">Content in subtle section</p>
                </Section>
                
                <Section title="Non-collapsible Section" collapsible={false}>
                  <p className="text-sm text-gray-300">This section cannot be collapsed</p>
                </Section>
              </div>
            </Section>

            {/* Forum Component */}
            <Section title="Forum Component" defaultOpen={true}>
              <div className="h-96">
                <Forum
                  threads={threads}
                  onNewThread={handleNewThread}
                  onReply={handleReply}
                />
              </div>
            </Section>
          </div>
        </div>
      </div>
    </ThemeProvider>
  );
};

export default UIShowcase;