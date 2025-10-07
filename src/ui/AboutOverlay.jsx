import { Show } from 'solid-js';
import { IconX, IconRocket, IconHeart, IconCode, IconWorld, IconUsers } from '@tabler/icons-solidjs';

export default function AboutOverlay({ isOpen, onClose }) {
  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <Show when={isOpen()}>
      <div 
        class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[200] animate-in fade-in duration-300"
        onClick={handleOverlayClick}
      >
        <div class="bg-base-200 rounded-2xl border border-base-300 shadow-2xl max-w-2xl w-full mx-4 animate-in zoom-in-95 duration-300">
          {/* Header */}
          <div class="flex items-center justify-between p-6 border-b border-base-300">
            <div class="flex items-center gap-4">
              <div class="w-12 h-12 bg-gradient-to-br from-primary to-secondary rounded-xl flex items-center justify-center">
                <IconRocket class="w-6 h-6 text-primary-content" />
              </div>
              <div>
                <h2 class="text-2xl font-bold text-base-content">About Renzora Engine</h2>
                <p class="text-sm text-base-content/60">r3-broken-af</p>
              </div>
            </div>
            <button
              onClick={onClose}
              class="w-8 h-8 flex items-center justify-center text-base-content/60 hover:text-base-content hover:bg-base-300 rounded-lg transition-colors"
            >
              <IconX class="w-4 h-4" />
            </button>
          </div>

          {/* Content */}
          <div class="p-6">
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* Project Info */}
              <div>
                <div class="flex items-center gap-2 mb-3">
                  <IconWorld class="w-4 h-4 text-primary" />
                  <h3 class="font-semibold text-base-content">Project</h3>
                </div>
                <p class="text-sm text-base-content/70 leading-relaxed mb-4">
                  Renzora Engine is an open-source, royalty-free 3D game engine designed to create 
                  console-quality games that run in web browsers. Built with modern web technologies 
                  and optimized for performance.
                </p>
                
                <div class="space-y-2 text-sm">
                  <div class="flex justify-between">
                    <span class="text-base-content/60">Version:</span>
                    <span class="text-base-content font-mono">r3-broken-af</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-base-content/60">Engine:</span>
                    <span class="text-base-content">Babylon.js + SolidJS</span>
                  </div>
                  <div class="flex justify-between">
                    <span class="text-base-content/60">License:</span>
                    <span class="text-base-content">Open Source</span>
                  </div>
                </div>
              </div>

              {/* Team & Credits */}
              <div>
                <div class="flex items-center gap-2 mb-3">
                  <IconUsers class="w-4 h-4 text-secondary" />
                  <h3 class="font-semibold text-base-content">Team</h3>
                </div>
                
                <div class="space-y-3">
                  <div class="p-3 bg-base-300/50 rounded-lg">
                    <div class="flex items-center gap-2 mb-1">
                      <IconCode class="w-4 h-4 text-primary" />
                      <span class="font-medium text-base-content">Core Development</span>
                    </div>
                    <p class="text-sm text-base-content/60">Renzora Engine Team</p>
                  </div>
                  
                  <div class="p-3 bg-base-300/50 rounded-lg">
                    <div class="flex items-center gap-2 mb-1">
                      <IconHeart class="w-4 h-4 text-accent" />
                      <span class="font-medium text-base-content">Special Thanks</span>
                    </div>
                    <p class="text-sm text-base-content/60">
                      Babylon.js, SolidJS, Tauri, and the open-source community
                    </p>
                  </div>
                </div>
              </div>
            </div>

            {/* Technology Stack */}
            <div class="mt-6 pt-6 border-t border-base-300">
              <h3 class="font-semibold text-base-content mb-3">Technology Stack</h3>
              <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
                {[
                  { name: 'Babylon.js', desc: '3D Engine' },
                  { name: 'SolidJS', desc: 'UI Framework' },
                  { name: 'Tauri', desc: 'Desktop App' },
                  { name: 'Rust', desc: 'Backend Bridge' },
                  { name: 'TailwindCSS', desc: 'Styling' },
                  { name: 'DaisyUI', desc: 'Components' },
                  { name: 'Rspack', desc: 'Bundler' },
                  { name: 'Bun', desc: 'Runtime' }
                ].map(tech => (
                  <div class="p-2 bg-gradient-to-br from-primary/10 to-secondary/10 rounded-lg border border-base-content/10">
                    <div class="font-medium text-xs text-base-content">{tech.name}</div>
                    <div class="text-xs text-base-content/50">{tech.desc}</div>
                  </div>
                ))}
              </div>
            </div>

            {/* Footer */}
            <div class="mt-6 pt-6 border-t border-base-300 text-center">
              <p class="text-xs text-base-content/50 mb-2">
                Built with passion for game developers worldwide
              </p>
              <p class="text-xs text-base-content/40">
                Copyright © {new Date().getFullYear()} Renzora Engine Team. All rights reserved.
              </p>
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
}