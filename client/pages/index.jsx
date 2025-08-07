import { useEffect, useCallback, Suspense, lazy } from 'react'

const EditorPlugin = lazy(() => import('@/plugins/editor/index.jsx'))
const ProjectsPlugin = lazy(() => import('@/plugins/projects/index.jsx'))
const LoadingProvider = lazy(() => import('@/plugins/projects/components/LoadingProvider.jsx'))
const EngineLoader = lazy(() => import('@/plugins/core/SimpleEngineLoader.jsx'))
const ProjectSplashManager = lazy(() => import('@/components/ProjectSplashManager.jsx'))
const DevelopmentDisclaimer = lazy(() => import('@/components/DevelopmentDisclaimer.jsx'))

export default function Index() {
  useEffect(() => {
    console.log('Engine starting...')
  }, []);

  const handleLoadComplete = useCallback(() => {
    console.log('🎮 Renzora Engine UI ready!')
  }, []);

  return (
    <>
      <Suspense fallback={
        <div className="fixed inset-0 bg-gray-900 flex items-center justify-center">
          <div className="text-white text-lg">Loading Renzora Engine...</div>
        </div>
      }>
        <Suspense fallback={
          <div className="fixed inset-0 bg-gray-900 flex items-center justify-center">
            <div className="text-white text-lg">Initializing project system...</div>
          </div>
        }>
          <ProjectSplashManager onProjectReady={handleLoadComplete}>
            <EngineLoader onLoadComplete={handleLoadComplete}>
              <Suspense fallback={<div className="text-white">Loading project system...</div>}>
                <LoadingProvider>
                  <Suspense fallback={<div className="text-white">Loading editor...</div>}>
                    <EditorPlugin />
                    <ProjectsPlugin />
                  </Suspense>
                </LoadingProvider>
              </Suspense>
            </EngineLoader>
          </ProjectSplashManager>
        </Suspense>
      </Suspense>
      
      <Suspense fallback={null}>
        <DevelopmentDisclaimer />
      </Suspense>
    </>
  )
}