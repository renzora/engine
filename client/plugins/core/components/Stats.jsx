import { useEffect, useRef } from 'react'
import { useSnapshot } from 'valtio'
import Stats from 'stats.js'
import { globalStore } from '@/store.js'

export default function StatsMonitor() {
  const statsRef = useRef(null)
  const containerRef = useRef(null)
  const { settings } = useSnapshot(globalStore.editor)
  const { showStats } = settings.editor

  useEffect(() => {
    if (showStats && !statsRef.current) {
      const stats = new Stats()
      
      stats.showPanel(0)
      
      stats.dom.style.position = 'fixed'
      stats.dom.style.top = '10px'
      stats.dom.style.left = '10px'
      stats.dom.style.zIndex = '9999'
      stats.dom.style.opacity = '0.8'
      
      if (containerRef.current) {
        containerRef.current.appendChild(stats.dom)
      } else {
        document.body.appendChild(stats.dom)
      }
      
      statsRef.current = stats
      
      function animate() {
        if (statsRef.current) {
          statsRef.current.begin()
          statsRef.current.end()
          requestAnimationFrame(animate)
        }
      }
      animate()
      
      console.log('📊 Stats.js enabled')
    }
    
    if (!showStats && statsRef.current) {
      if (statsRef.current.dom.parentNode) {
        statsRef.current.dom.parentNode.removeChild(statsRef.current.dom)
      }
      statsRef.current = null
      console.log('📊 Stats.js disabled')
    }
  }, [showStats])

  useEffect(() => {
    return () => {
      if (statsRef.current && statsRef.current.dom.parentNode) {
        statsRef.current.dom.parentNode.removeChild(statsRef.current.dom)
      }
    }
  }, [])

  return <div ref={containerRef} style={{ position: 'fixed', top: 0, left: 0, pointerEvents: 'none' }} />
}