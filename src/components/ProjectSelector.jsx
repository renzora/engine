import { createSignal, onMount, Show, For } from 'solid-js'
import { IconFolder, IconChevronRight, IconPlus } from '@tabler/icons-solidjs'
import { bridgeService } from '@/plugins/core/bridge'

export default function ProjectSelector(props) {
  const [projectName, setProjectName] = createSignal('')
  const [isCreating, setIsCreating] = createSignal(false)
  const [isLoading, setIsLoading] = createSignal(true)
  const [error, setError] = createSignal('')
  const [existingProjects, setExistingProjects] = createSignal([])
  const [showCreateForm, setShowCreateForm] = createSignal(false)

  onMount(() => {
    loadExistingProjects()
  })

  const loadExistingProjects = async () => {
    setIsLoading(true)
    try {
      const [projectList] = await Promise.all([
        bridgeService.getProjects(),
        new Promise(resolve => setTimeout(resolve, 300))
      ])
      setExistingProjects(projectList)
    } catch (error) {
      console.warn('Failed to load existing projects:', error)
      setExistingProjects([])
    }
    setIsLoading(false)
  }

  const handleSelectProject = async (project) => {
    if (isLoading() || isCreating()) return
    
    setIsLoading(true)
    setError('')
    
    try {
      await props.onSelectProject(project)
    } catch (err) {
      setError(err.message)
      setIsLoading(false)
    }
  }

  const handleCreateProject = async (e) => {
    e.preventDefault()
    if (!projectName().trim() || isCreating()) return

    setIsCreating(true)
    setError('')

    try {
      // Create basic project data structure
      const projectData = {
        name: projectName().trim(),
        path: `projects/${projectName().trim()}`,
        created: new Date()
      };
      
      // Set as current project in bridge service
      bridgeService.setCurrentProject(projectData);
      props.onProjectCreated?.(projectData)
    } catch (err) {
      setError(err.message)
    }
    setIsCreating(false)
  }

  const toggleCreateForm = () => {
    setShowCreateForm(!showCreateForm())
    setProjectName('')
    setError('')
  }

  return (
    <div class="fixed inset-0 bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900 flex items-center justify-center">
        <div class="bg-gray-800 rounded-xl shadow-2xl border border-gray-700 w-full max-w-md mx-4">
          <div class="p-6 border-b border-gray-700">
            <div class="flex items-center gap-3">
              <div class="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center">
                <IconFolder class="w-5 h-5 text-white" />
              </div>
              <div>
                <h1 class="text-xl font-bold text-white">Renzora Engine</h1>
                <p class="text-gray-400 text-sm">Select or create a project</p>
              </div>
            </div>
          </div>

          <div class="p-6 space-y-4">
            <Show when={error()}>
              <div class="bg-red-900/50 border border-red-700 rounded-lg p-3">
                <p class="text-red-300 text-sm">{error()}</p>
              </div>
            </Show>

            <Show when={isLoading()}>
              <div class="flex items-center justify-center py-8">
                <div class="w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full animate-spin" />
                <span class="ml-3 text-gray-300">Loading projects...</span>
              </div>
            </Show>

            <Show when={!isLoading()}>
              <Show when={existingProjects().length > 0}>
                <div class="space-y-2">
                  <h3 class="text-sm font-medium text-gray-300 mb-3">Recent Projects</h3>
                  <For each={existingProjects()}>
                    {(project) => (
                      <button
                        onClick={() => handleSelectProject(project)}
                        disabled={isCreating()}
                        class="w-full p-3 text-left bg-gray-700 hover:bg-gray-600 rounded-lg border border-gray-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed group"
                      >
                        <div class="flex items-center justify-between">
                          <div class="flex items-center gap-3">
                            <IconFolder class="w-5 h-5 text-blue-400" />
                            <div>
                              <p class="font-medium text-white group-hover:text-blue-300 transition-colors">
                                {project.display_name || project.name}
                              </p>
                              <p class="text-xs text-gray-400 truncate max-w-64">
                                {project.path}
                              </p>
                            </div>
                          </div>
                          <IconChevronRight class="w-4 h-4 text-gray-400 group-hover:text-white transition-colors" />
                        </div>
                      </button>
                    )}
                  </For>
                </div>
              </Show>

              <div class="pt-4 border-t border-gray-700">
                <Show when={!showCreateForm()}>
                  <button
                    onClick={toggleCreateForm}
                    disabled={isCreating()}
                    class="w-full flex items-center justify-center gap-2 px-4 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <IconPlus class="w-4 h-4" />
                    Create New Project
                  </button>
                </Show>

                <Show when={showCreateForm()}>
                  <form onSubmit={handleCreateProject} class="space-y-3">
                    <input
                      type="text"
                      placeholder="Project name"
                      value={projectName()}
                      onInput={(e) => setProjectName(e.target.value)}
                      disabled={isCreating()}
                      class="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
                      autofocus
                    />
                    <div class="flex gap-2">
                      <button
                        type="submit"
                        disabled={!projectName().trim() || isCreating()}
                        class="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-green-600 hover:bg-green-700 text-white rounded-lg font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        <Show when={isCreating()}>
                          <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                        </Show>
                        <Show when={!isCreating()}>
                          <IconPlus class="w-4 h-4" />
                        </Show>
                        Create
                      </button>
                      <button
                        type="button"
                        onClick={toggleCreateForm}
                        disabled={isCreating()}
                        class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white rounded-lg transition-colors disabled:opacity-50"
                      >
                        Cancel
                      </button>
                    </div>
                  </form>
                </Show>
              </div>
            </Show>
          </div>
        </div>
      </div>
  )
}