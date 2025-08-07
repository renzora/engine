import React, { useState, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { projectCreationService } from '@/services/ProjectCreationService';

const ProjectSplashScreen = ({ onCreateProject, onSelectProject, onClose }) => {
  const [projectName, setProjectName] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');
  const [existingProjects, setExistingProjects] = useState([]);
  const [showCreateForm, setShowCreateForm] = useState(false);

  useEffect(() => {
    loadExistingProjects();
  }, []);

  const loadExistingProjects = async () => {
    setIsLoading(true);
    try {
      const projects = await projectCreationService.listProjects();
      setExistingProjects(projects);
      console.log('Loaded existing projects:', projects);
    } catch (error) {
      console.warn('Failed to load existing projects:', error);
      setExistingProjects([]);
    }
    setIsLoading(false);
  };

  const handleSelectProject = async (project) => {
    if (isLoading || isCreating) return;
    
    setIsLoading(true);
    setError('');
    
    try {
      await onSelectProject(project);
    } catch (err) {
      setError(err.message || 'Failed to load project');
      setIsLoading(false);
    }
  };

  const handleCreateProject = async () => {
    if (!projectName.trim()) {
      setError('Project name is required');
      return;
    }

    // Validate project name
    const validNamePattern = /^[a-zA-Z0-9_-]+$/;
    if (!validNamePattern.test(projectName.trim())) {
      setError('Project name can only contain letters, numbers, underscores, and hyphens');
      return;
    }

    setIsCreating(true);
    setError('');

    try {
      await onCreateProject(projectName.trim());
    } catch (err) {
      setError(err.message || 'Failed to create project');
      setIsCreating(false);
    }
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter' && !isCreating) {
      handleCreateProject();
    }
  };

  if (isLoading && existingProjects.length === 0) {
    return (
      <div className="fixed inset-0 bg-gray-900 flex items-center justify-center z-50">
        <div className="text-white text-lg flex items-center gap-3">
          <div className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
          Loading projects...
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-gray-900 flex items-center justify-center z-50">
      <div className="bg-slate-800 rounded-lg shadow-2xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-slate-700">
          <div className="flex items-center gap-3">
            <Icons.Folder className="w-6 h-6 text-blue-400" />
            <div>
              <h2 className="text-xl font-semibold text-white">
                {showCreateForm ? 'Create New Project' : 'Select Project'}
              </h2>
              <p className="text-sm text-gray-400">
                {showCreateForm ? 'Start building something amazing' : 'Choose a project or create a new one'}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-slate-700 rounded transition-colors"
            disabled={isLoading || isCreating}
          >
            <Icons.XMark className="w-4 h-4 text-gray-400" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4 max-h-[60vh] overflow-y-auto">
          {showCreateForm ? (
            <>
              {/* Create Project Form */}
              <div>
                <label className="block text-sm font-medium text-gray-300 mb-2">
                  Project Name
                </label>
                <input
                  type="text"
                  value={projectName}
                  onChange={(e) => {
                    setProjectName(e.target.value);
                    setError(''); // Clear error when typing
                  }}
                  onKeyPress={handleKeyPress}
                  placeholder="Enter project name"
                  className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  autoFocus
                  disabled={isCreating}
                />
                {error && (
                  <p className="mt-2 text-sm text-red-400 flex items-center gap-1">
                    <Icons.ExclamationTriangle className="w-4 h-4" />
                    {error}
                  </p>
                )}
              </div>

              <div className="bg-slate-900/50 rounded-lg p-3">
                <div className="text-sm text-gray-300 mb-2">Project will be created with:</div>
                <div className="text-xs text-gray-400 space-y-1">
                  <div className="flex items-center gap-2">
                    <Icons.Folder className="w-3 h-3" />
                    A dedicated project folder
                  </div>
                  <div className="flex items-center gap-2">
                    <Icons.Photo className="w-3 h-3" />
                    Assets directory for your files
                  </div>
                  <div className="flex items-center gap-2">
                    <Icons.Cube className="w-3 h-3" />
                    Default scene setup
                  </div>
                </div>
              </div>
            </>
          ) : (
            <>
              {/* Project List */}
              {existingProjects.length > 0 ? (
                <div>
                  <label className="block text-sm font-medium text-gray-300 mb-3">
                    Existing Projects
                  </label>
                  <div className="space-y-2 max-h-60 overflow-y-auto">
                    {existingProjects.map((project, index) => (
                      <button
                        key={project.path || project.name || index}
                        onClick={() => handleSelectProject(project)}
                        disabled={isLoading}
                        className="w-full p-4 bg-slate-700 hover:bg-slate-600 rounded-lg text-left transition-colors border border-slate-600 hover:border-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        <div className="flex items-start justify-between">
                          <div>
                            <div className="font-medium text-white">
                              {project.displayName || project.name}
                            </div>
                            <div className="text-sm text-gray-400">
                              {project.path || project.name}
                            </div>
                            {project.lastModified && (
                              <div className="text-xs text-gray-500 mt-1">
                                Last modified: {new Date(project.lastModified).toLocaleDateString()}
                              </div>
                            )}
                          </div>
                          <Icons.ChevronRight className="w-4 h-4 text-gray-400 mt-1" />
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="text-center py-8">
                  <Icons.Folder className="w-12 h-12 text-gray-400 mx-auto mb-3" />
                  <div className="text-gray-300 mb-2">No Projects Found</div>
                  <div className="text-sm text-gray-400">Create your first project to get started</div>
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-6 border-t border-slate-700">
          <div>
            {!showCreateForm && (
              <button
                onClick={() => setShowCreateForm(true)}
                disabled={isLoading}
                className="px-4 py-2 text-sm bg-green-600 hover:bg-green-700 text-white rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                New Project
              </button>
            )}
          </div>
          
          <div className="flex items-center gap-3">
            {showCreateForm ? (
              <>
                <button
                  onClick={() => {
                    setShowCreateForm(false);
                    setProjectName('');
                    setError('');
                  }}
                  className="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
                  disabled={isCreating}
                >
                  Back
                </button>
                <button
                  onClick={handleCreateProject}
                  disabled={!projectName.trim() || isCreating}
                  className={`px-6 py-2 text-sm rounded-lg transition-colors flex items-center gap-2 ${
                    !projectName.trim() || isCreating
                      ? 'bg-gray-600 text-gray-400 cursor-not-allowed'
                      : 'bg-blue-600 hover:bg-blue-700 text-white'
                  }`}
                >
                  {isCreating && <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />}
                  {isCreating ? 'Creating...' : 'Create Project'}
                </button>
              </>
            ) : (
              <button
                onClick={onClose}
                className="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
                disabled={isLoading}
              >
                Cancel
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default ProjectSplashScreen;