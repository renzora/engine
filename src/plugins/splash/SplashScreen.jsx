import { createSignal, createEffect, onMount, Show, For } from 'solid-js';
import { Icons } from '@/ui';
const { Folder, Plus, FolderOpen, Settings, Code, Rocket, Cube } = Icons;
import { bridgeService } from '@/plugins/core/bridge';
import AnimatedBackground from './AnimatedBackground';
import { 
  Title, 
  Subtitle, 
  Caption, 
  Button, 
  Card, 
  Grid, 
  Stack, 
  Spinner, 
  IconContainer,
  Field,
  Input 
} from '@/ui';

export default function SplashScreen({ onProjectSelect }) {
  const [projects, setProjects] = createSignal([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [newProjectName, setNewProjectName] = createSignal('');
  const [creating, setCreating] = createSignal(false);

  const loadProjects = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const response = await fetch('http://localhost:3001/projects');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      
      const projectData = await response.json();
      setProjects(projectData || []);
    } catch (err) {
      console.error('Failed to load projects:', err);
      setError('Failed to connect to project server. Make sure the bridge server is running.');
    } finally {
      setLoading(false);
    }
  };

  const createProject = async () => {
    const name = newProjectName().trim();
    if (!name) return;

    try {
      setCreating(true);
      
      const response = await fetch('http://localhost:3001/projects', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          name,
          template: 'basic'
        })
      });

      if (!response.ok) {
        throw new Error(`Failed to create project: ${response.status}`);
      }

      await loadProjects();
      const newProject = projects().find(p => p.name === name);
      if (newProject) {
        onProjectSelect(newProject);
      }
      
      setShowCreateDialog(false);
      setNewProjectName('');
    } catch (err) {
      console.error('Failed to create project:', err);
      setError('Failed to create project. Please try again.');
    } finally {
      setCreating(false);
    }
  };

  onMount(() => {
    loadProjects();
  });

  return (
    <div class="w-full h-full relative flex overflow-hidden bg-black">
      <AnimatedBackground />
      
      <div class="flex-1 relative z-10 flex flex-col justify-center items-center p-12">
        <Stack align="center" gap="lg" class="text-center mb-8">
          <IconContainer size="xxl" variant="primary">
            <Rocket />
          </IconContainer>
          
          <Stack gap="sm" align="center">
            <Title size="xl" class="mb-3">
              Renzora <span class="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">Engine</span> <span class="text-orange-400">r2</span>
            </Title>
            <Subtitle class="max-w-md mx-auto mb-8">
              Open source render-agnostic game engine for building cross-platform games
            </Subtitle>
          </Stack>
          
          <Button 
            variant="gradient" 
            size="lg"
            onClick={() => setShowCreateDialog(true)}
            class="w-full p-5 group"
          >
            <Stack align="center" gap="sm">
              <IconContainer size="lg" variant="glass" class="group-hover:from-blue-500/20 group-hover:to-purple-500/20 group-hover:border-blue-400/30">
                <Plus />
              </IconContainer>
              <Stack gap="xs" align="center">
                <div class="font-semibold">Create New Project</div>
                <div class="text-xs text-gray-500 group-hover:text-gray-400">Start building something amazing</div>
              </Stack>
            </Stack>
          </Button>
        </Stack>
      </div>

      <div class="w-[32rem] relative z-10 flex flex-col">
        <div class="flex-1 p-12 flex flex-col min-h-0">
          <Show when={loading()}>
            <Stack align="center" justify="center" gap="md" class="text-center py-8 flex-1">
              <Spinner size="lg" />
              <Subtitle size="md">Loading projects...</Subtitle>
            </Stack>
          </Show>

          <Show when={error()}>
            <Stack align="center" justify="center" gap="md" class="text-center py-8 flex-1">
              <IconContainer size="lg" variant="danger">
                <Settings />
              </IconContainer>
              <Subtitle size="sm" class="text-red-400 mb-4">{error()}</Subtitle>
              <Button variant="primary" size="sm" onClick={loadProjects}>
                Retry
              </Button>
            </Stack>
          </Show>

          <Show when={!loading() && !error()}>
            <div class="flex flex-col h-full min-h-0">
              <Show when={projects().length > 0}>
                <div class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden scrollbar-thin">
                  <Caption size="sm" class="uppercase tracking-wider mb-6">Recent Projects</Caption>
                  <Grid cols={3} gap="md" class="mr-2">
                    <For each={projects()}>
                      {(project) => (
                        <Card 
                          variant="gradient"
                          hoverable={true}
                          onClick={() => onProjectSelect(project)}
                          class="cursor-pointer group"
                        >
                          <Stack align="center" gap="sm">
                            <IconContainer size="lg" variant="glass">
                              <Folder />
                            </IconContainer>
                            <Stack gap="xs" align="center" class="w-full">
                              <Title size="sm" class="truncate group-hover:text-blue-100">{project.name}</Title>
                              <Caption size="xs" class="truncate font-mono">{project.path}</Caption>
                              <Stack direction="horizontal" align="center" gap="xs">
                                <Cube class="w-3 h-3" />
                                <Caption size="xs">{project.files?.length || 0} assets</Caption>
                              </Stack>
                            </Stack>
                          </Stack>
                        </Card>
                      )}
                    </For>
                  </Grid>
                </div>
              </Show>

              <Show when={projects().length === 0}>
                <Stack align="center" justify="center" gap="lg" class="text-center py-12 flex-1">
                  <IconContainer size="xxl" variant="surface">
                    <FolderOpen />
                  </IconContainer>
                  <Stack gap="sm" align="center">
                    <Title size="lg">No projects yet</Title>
                    <Subtitle size="md" class="max-w-xs">Create your first project to start building amazing 3D experiences</Subtitle>
                  </Stack>
                  <Button 
                    variant="gradient" 
                    size="lg"
                    onClick={() => setShowCreateDialog(true)}
                  >
                    Get Started
                  </Button>
                </Stack>
              </Show>
            </div>
          </Show>
        </div>
      </div>

      <Show when={showCreateDialog()}>
        <div class="fixed inset-0 bg-black/70 backdrop-blur-md flex items-center justify-center p-4 z-[100] animate-in fade-in duration-300">
          <Card variant="glass" padding="xl" class="w-full max-w-lg animate-in zoom-in-95 duration-300 shadow-2xl">
            <Stack gap="lg">
              <Stack direction="horizontal" align="center" gap="md">
                <IconContainer size="lg" variant="primary">
                  <Plus />
                </IconContainer>
                <Title size="lg">Create New Project</Title>
              </Stack>
              
              <Field 
                label="Project Name"
                help="Choose a descriptive name for your project"
              >
                <Input
                  type="text"
                  value={newProjectName()}
                  onInput={(e) => setNewProjectName(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && createProject()}
                  placeholder="My Awesome Project"
                  size="lg"
                  autofocus
                />
              </Field>

              <Stack direction="horizontal" justify="end" gap="md">
                <Button
                  variant="ghost"
                  onClick={() => {
                    setShowCreateDialog(false);
                    setNewProjectName('');
                  }}
                  disabled={creating()}
                >
                  Cancel
                </Button>
                <Button
                  variant="gradient"
                  size="lg"
                  onClick={createProject}
                  disabled={!newProjectName().trim() || creating()}
                  class="min-w-40"
                >
                  <Show when={creating()}>
                    <Spinner size="sm" />
                  </Show>
                  <Show when={!creating()}>
                    <Rocket class="w-5 h-5" />
                  </Show>
                  {creating() ? 'Creating...' : 'Create Project'}
                </Button>
              </Stack>
            </Stack>
          </Card>
        </div>
      </Show>
    </div>
  );
}