game = {
    needsFilterUpdate: true,
    canvas: undefined,
    ctx: undefined,
    isEditMode: false,
    x: null,
    y: null,
    timestamp: 0,
    worldWidth: 1280,
    worldHeight: 944,
    zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 5,
    targetX: 0,
    targetY: 0,
    roomData: undefined,
    sprites: {},
    objectData: null,
    spriteData: null,
    playerid: null,
    sceneid: null,
    desiredFPS: 60,
    fixedDeltaTime: 1000 / 60,
    deltaTime: null,
    accumulatedTime: 0,
    lastTime: null,
    maxAccumulatedTime: 1000,
    allowControls: true,
    selectedObjects: [],
    selectedCache: [],
    pathfinding: true,
    selectedTiles: [],
    overlappingTiles: [],
    isPaused: false,
    inputMethod: 'keyboard',
    fpsHistory: [],
    maxFpsHistory: 60,
    bgPattern: null,
    renderCalls: 0,
    tileCount: 0,
    spriteCount: 0,
    animationCount: 0,
    backgroundTileCount: 0,
    renderQueue: [],
    sceneBg: null,
    viewportXStart: 0,
    viewportXEnd: 0,
    viewportYStart: 0,
    viewportYEnd: 0,
    currentTileData: null,
    currentRoomItem: null,
    objectCache: {},
  
    loadGameAssets(onLoaded) {
      fetch('/api/tileset_manager/list_sheets')
        .then(r => r.json())
        .then(d => {
          if (!d.success || !Array.isArray(d.sheets)) throw new Error('Failed to load sheets list')
          const arr = d.sheets.map(n => ({ name: n, path: `assets/img/sheets/${n}.png` }))
          assets.preload(arr, () => {
            if (typeof onLoaded === 'function') onLoaded()
          })
        })
        .catch(err => {
          if (typeof onLoaded === 'function') onLoaded(err)
        })
    },
  
    create(config = {}) {
      this.loadGameAssets(() => {
        this.canvas = document.createElement('canvas')
        this.ctx = this.canvas.getContext('2d')
        this.ctx.imageSmoothingEnabled = false
        document.body.appendChild(this.canvas)
        input.assign('resize', e => this.resizeCanvas(e))
        this.resizeCanvas()
        this.loop()
        gamepad.init(config)
        plugin.hook('onGameCreate')
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this))
        this.canvas.addEventListener('contextmenu', e => e.preventDefault())
        document.addEventListener('visibilitychange', () => {
          if (document.hidden) this.pause(); else this.resume()
        })
        if (config.objectData) this.objectData = config.objectData
        if (config.spriteData) this.spriteData = config.spriteData
        if (config.player) {
          this.mainSprite = config.player
          this.playerid = this.mainSprite.id
          this.sprites[this.playerid] = this.mainSprite
        }
        if (typeof config.after === 'function') config.after()
      })
    },
  
    pause() {
      cancelAnimationFrame(this.animationFrameId)
      plugin.audio.pauseAll()
      this.isPaused = true
    },
  
    resume() {
      plugin.network.send({ command: 'requestGameState', playerId: this.playerid })
      plugin.audio.resumeAll()
    },
  
    resizeCanvas() {
      this.setZoomLevel()
      const c = document.getElementById('console_window')
      const t = document.getElementById('tabs')
      let cw = 0, tw = 0
      if (!this.isEditMode && c && console_window.isOpen) cw = c.offsetWidth
      if (t && t.style.display !== 'none') tw = t.offsetWidth
      const offW = cw + tw
      const aw = window.innerWidth - offW
      const ah = window.innerHeight
      const cw2 = Math.min(this.worldWidth * this.zoomLevel, aw)
      const ch2 = Math.min(this.worldHeight * this.zoomLevel, ah)
      this.canvas.width = cw2
      this.canvas.height = ch2
      this.canvas.style.width = `${cw2}px`
      this.canvas.style.height = `${ch2}px`
      const hOff = (aw - cw2) / 2 + offW
      const vOff = (ah - ch2) / 2
      this.canvas.style.position = 'absolute'
      this.canvas.style.left = `${hOff}px`
      this.canvas.style.top = `${vOff}px`
    },
  
    scene(sceneId) {
        plugin.pathfinding.cancelPathfinding(this.sprites[this.playerid])
        fetch(`/api/scenes/${encodeURIComponent(sceneId)}`, {
          method: 'GET', headers: { 'Content-Type': 'application/json' }
        })
        .then(r => { if (!r.ok) throw new Error(r.statusText); return r.json(); })
        .then(d => {
          if (d.message === 'success') {
            plugin.lighting.clearLightsAndEffects()
            this.roomData = d.roomData
            this.sceneid = d._id
            this.serverid = d.server_id
            localStorage.setItem('sceneid', d._id)
            this.worldWidth = d.width || 1280
            this.worldHeight = d.height || 944
            this.x = d.startingX || 0
            this.y = d.startingY || 0
            const p = this.sprites[this.playerid]
            if (p) { p.x = this.x; p.y = this.y; }
            this.sceneBg = d.bg || null
            this.resizeCanvas()
            plugin.collision.walkableGridCache = null
            plugin.collision.createWalkableGrid()
            this.overlappingTiles = []
            camera.update()
            plugin.effects.start('fadeOut', 1000)
            plugin.effects.start('fadeIn', 1000)
            this.buildRepeatingBackground()
          } else {
            plugin.load('errors', { ext: 'html' })
          }
        })
        .catch(() => {
          plugin.load('errors', { ext: 'html' })
        })
      },      
  
    loop(timestamp) {
      if (!this.lastTime) {
        this.lastTime = timestamp
        this.lastFpsUpdateTime = timestamp
        requestAnimationFrame(this.loop.bind(this))
        return
      }
      const dt = timestamp - this.lastTime
      if (dt > 1000) {
        this.accumulatedTime = this.fixedDeltaTime
      } else {
        this.accumulatedTime += dt
      }
      this.deltaTime = this.fixedDeltaTime
      this.lastTime = timestamp
      while (this.accumulatedTime >= this.fixedDeltaTime) {
        gamepad.updateGamepadState()
        this.viewportXStart = Math.floor(camera.cameraX / 16)
        this.viewportXEnd   = Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16)
        this.viewportYStart = Math.floor(camera.cameraY / 16)
        this.viewportYEnd   = Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16)
        this.viewportXStart = Math.max(0, this.viewportXStart)
        this.viewportXEnd   = Math.min(this.worldWidth / 16, this.viewportXEnd)
        this.viewportYStart = Math.max(0, this.viewportYStart)
        this.viewportYEnd   = Math.min(this.worldHeight / 16, this.viewportYEnd)
        for (let id in this.sprites) {
          const sp = this.sprites[id]
          const r = sp.x + sp.width
          const b = sp.y + sp.height
          if (r >= this.viewportXStart*16 && sp.x < this.viewportXEnd*16 &&
              b >= this.viewportYStart*16 && sp.y < this.viewportYEnd*16) {
            if (sp.update) sp.update()
          }
        }
        camera.update()
        this.updateAnimatedTiles()
        this.accumulatedTime -= this.fixedDeltaTime
      }
      this.ctx.imageSmoothingEnabled = false
      this.ctx.setTransform(1, 0, 0, 1, 0, 0)
      this.ctx.scale(this.zoomLevel, this.zoomLevel)
      this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY))
      this.renderCalls = 0
      this.tileCount = 0
      this.renderBackground()
      this.render()
      plugin.hook('onRender')
      this.renderCarriedObjects()
      this.handleDebugUtilities()
      if (plugin.ui_console_editor_inventory.selectedInventoryItem) plugin.ui_console_editor_inventory.render()
      plugin.ui_console_tab_window.renderCollisionBoundaries()
      plugin.ui_console_tab_window.renderNearestWalkableTile()
      plugin.ui_console_tab_window.renderObjectCollision()
      if (this.mainSprite && this.mainSprite.isVehicle) {
        plugin.ui_overlay_window.update(this.mainSprite.currentSpeed, this.mainSprite.maxSpeed)
      }
      plugin.debug.tracker('game.loop')
      requestAnimationFrame(this.loop.bind(this))
    },
  
    buildRepeatingBackground() {
      if (!this.sceneBg || !this.objectData[this.sceneBg]) {
        this.bgPattern = null
        return
      }
      const bgDataArray = this.objectData[this.sceneBg]
      if (!bgDataArray.length) {
        this.bgPattern = null
        return
      }
      const tileData = bgDataArray[0]
      if (!tileData.i || !tileData.t) {
        this.bgPattern = null
        return
      }
      let firstFrame = null
      if (Array.isArray(tileData.i)) {
        const first = tileData.i[0]
        if (typeof first === 'string' && first.includes('-')) {
          const all = this.parseRange(first)
          firstFrame = all[0]
        } else if (typeof first === 'string') {
          const pn = parseInt(first, 10)
          if (!isNaN(pn)) firstFrame = pn
        } else if (typeof first === 'number') {
          firstFrame = first
        }
      } else if (typeof tileData.i === 'string') {
        if (tileData.i.includes('-')) {
          const all = this.parseRange(tileData.i)
          firstFrame = all[0]
        } else {
          const pn = parseInt(tileData.i, 10)
          if (!isNaN(pn)) firstFrame = pn
        }
      } else if (typeof tileData.i === 'number') {
        firstFrame = tileData.i
      }
      if (firstFrame === null) {
        this.bgPattern = null
        return
      }
      const image = assets.use(tileData.t)
      if (!image) {
        this.bgPattern = null
        return
      }
      const tileSize = 16
      const sheetWidthInTiles = 150
      const sx = (firstFrame % sheetWidthInTiles) * tileSize
      const sy = Math.floor(firstFrame / sheetWidthInTiles) * tileSize
      const tc = document.createElement('canvas')
      tc.width = tileSize
      tc.height = tileSize
      const tcx = tc.getContext('2d')
      tcx.drawImage(image, sx, sy, tileSize, tileSize, 0, 0, tileSize, tileSize)
      this.bgPattern = this.ctx.createPattern(tc, 'repeat')
    },
  
    renderBackground() {
      const startX = this.viewportXStart * 16
      const startY = this.viewportYStart * 16
      const width  = (this.viewportXEnd - this.viewportXStart) * 16
      const height = (this.viewportYEnd - this.viewportYStart) * 16
      if (width <= 0 || height <= 0) return
      this.ctx.save()
      this.ctx.translate(startX, startY)
      if (!this.bgPattern) {
        this.ctx.fillStyle = 'black'
      } else {
        this.ctx.fillStyle = this.bgPattern
      }
      this.ctx.fillRect(0, 0, width, height)
      this.ctx.restore()
      plugin.hook('onRenderBackground')
      this.renderCalls++
    },
  
    buildObjectCanvas(cacheKey, tileData, xCoords, yCoords) {
      const minX = Math.min(...xCoords)
      const maxX = Math.max(...xCoords)
      const minY = Math.min(...yCoords)
      const maxY = Math.max(...yCoords)
      const wTiles = maxX - minX + 1
      const hTiles = maxY - minY + 1
      const offscreen = document.createElement('canvas')
      offscreen.width = wTiles * 16
      offscreen.height = hTiles * 16
      const offCtx = offscreen.getContext('2d', { willReadFrequently: true })
      let index = 0
      for (let yy=0; yy<yCoords.length; yy++) {
        for (let xx=0; xx<xCoords.length; xx++) {
          const tX = xCoords[xx]
          const tY = yCoords[yy]
          let srcIndex
          if (Array.isArray(tileData.i[0])) {
            const cf = tileData.currentFrame || 0
            const arr = tileData.i[cf]
            srcIndex = arr ? arr[index] : undefined
          } else {
            srcIndex = tileData.i[index]
          }
          if (srcIndex !== undefined) {
            const sx = (srcIndex % 150)*16
            const sy = Math.floor(srcIndex/150)*16
            const dx = (tX - minX)*16
            const dy = (tY - minY)*16
            offCtx.drawImage(assets.use(tileData.t), sx, sy, 16,16, dx, dy, 16,16)
          }
          index++
        }
      }
      this.objectCache[cacheKey] = offscreen
      return offscreen
    },
  
    render() {
      this.renderQueue = []
      plugin.hook('onRenderAll')
      const expanded = Object.keys(this.objectData).reduce((acc, key) => {
        acc[key] = this.objectData[key].map(this.expandTileData.bind(this))
        return acc
      }, {})
  
      if (this.roomData && this.roomData.items) {
        this.roomData.items.forEach(rItem => {
          const iData = expanded[rItem.id]
          if (!iData || iData.length === 0) return
          const tData = iData[0]
  
          if (rItem.visible === false) {
            this.currentTileData = tData
            this.currentRoomItem = rItem
            this.handleLights()
            return
          }
  
          const xC = rItem.x || []
          const yC = rItem.y || []
          const minX = Math.min(...xC)
          const maxX = Math.max(...xC)
          const minY = Math.min(...yC)
          const maxY = Math.max(...yC)
          const objL = minX * 16
          const objR = (maxX + 1) * 16
          const objT = minY * 16
          const objB = (maxY + 1) * 16
          const cL = this.viewportXStart * 16
          const cR = this.viewportXEnd * 16
          const cT = this.viewportYStart * 16
          const cB = this.viewportYEnd * 16
          if (objR < cL || objL > cR || objB < cT || objT > cB) return
  
          let rot = tData.rotation || 0
          if (rItem.isRotating) {
            actions.handleRotation(rItem)
            rot = rItem.rotation
          }
          if (tData.sway === true) {
            rot += this.handleSway(rItem)
          }
          if (rItem.rotation != null) {
            rot = rItem.rotation
          }
          const cacheKey = rItem.id + '_' + (tData.currentFrame || 0) + '_' + rot
          let offscreen = this.objectCache[cacheKey]
          if (!offscreen) {
            offscreen = this.buildObjectCanvas(cacheKey, tData, xC, yC)
          }
  
          let zIndex
          if (tData.z === 0) {
            zIndex = -9999
          } else {
            zIndex = maxY * 16
          }
  
          this.renderQueue.push({
            zIndex: zIndex,
            type: 'object',
            id: rItem.id,
            visible: true,
            layer_id: "item_" + rItem.layer_id,
            data: {
              offscreen,
              rotation: rot
            },
            draw: () => {
              this.ctx.save()
              const pivotX = (minX + maxX + 1) / 2 * 16
              const pivotY = (maxY + 1) * 16
              this.ctx.translate(pivotX, pivotY)
              const scale = rItem.scale || 1
              this.ctx.scale(scale, scale)
            
              if (rItem.flipHorizontal) {
                this.ctx.scale(-1, 1)
              }
              
              if (rItem.rotation != null) {
                this.ctx.rotate(rItem.rotation)
              } else {
                this.ctx.rotate(rot)
              }
            
              const dx = -offscreen.width / 2
              const dy = -offscreen.height
              this.ctx.drawImage(offscreen, dx, dy)
              this.renderCalls++
              this.tileCount++
              this.ctx.restore()
            }
          })
  
          this.currentTileData = tData
          this.currentRoomItem = rItem
          this.handleLights()
          this.handleEffects()
        })
      }
  
      for (let id in this.sprites) {
        const sp = this.sprites[id]
        const r = sp.x + sp.width
        const b = sp.y + sp.height
        if (
          r >= this.viewportXStart * 16 &&
          sp.x < this.viewportXEnd * 16 &&
          b >= this.viewportYStart * 16 &&
          sp.y < this.viewportYEnd * 16
        ) {
          const z = sp.y + sp.height
          this.renderQueue.push({
            zIndex: z,
            type: 'sprite',
            id: id,
            data: { sprite: sp },
            draw: () => {
              this.renderPathfinderLine()
              sp.drawShadow()
              sp.draw()
            }
          })
          this.spriteCount++
        }
      }
  
      this.renderQueue.sort((a, b) => a.zIndex - b.zIndex)
      this.renderQueue.forEach(item => {
        item.draw()
      })
      plugin.debug.tracker("render.renderAll")
    },
  
    handleMouseUp(e) {
      if (this.isEditMode || (this.mainSprite && this.mainSprite.targetAim)) return
      const r = this.canvas.getBoundingClientRect()
      const mx = (e.clientX - r.left)/this.zoomLevel + camera.cameraX
      const my = (e.clientY - r.top)/this.zoomLevel + camera.cameraY
      this.x = Math.floor(mx/16)
      this.y = Math.floor(my/16)
      if (plugin.exists('collision')) {
        if (!plugin.collision.isTileWalkable(this.x,this.y)) return
      }
      if (plugin.exists('pathfinding')) {
        plugin.pathfinding.walkToClickedTile(this.mainSprite, e, this.x, this.y)
      }
    },
  
    handleSway(rm) {
      if (!rm.swayInitialized) {
        rm.swayAngle = Math.PI/(160+Math.random()*40)
        rm.swaySpeed = 5000+Math.random()*2000
        rm.swayInitialized=true
      }
      if (rm.isInViewport) {
        const et = rm.swayElapsed||0
        rm.swayElapsed=et+this.deltaTime
        const s = Math.sin((rm.swayElapsed / rm.swaySpeed)*Math.PI*2)*rm.swayAngle
        return s
      }
      return 0
    },
  
    initializeSway(rm) {
      rm.swayAngle=Math.PI/(160+Math.random()*40)
      rm.swaySpeed=5000+Math.random()*2000
      rm.swayElapsed=0
      rm.swayInitialized=true
    },
  
    renderPathfinderLine() {
      if (this.mainSprite && this.mainSprite.path && this.mainSprite.path.length>0) {
        const ctx = this.ctx
        const last = this.mainSprite.path[this.mainSprite.path.length-1]
        const el = Date.now()%1000
        const p1 = (el%1000)/1000
        const p2 = ((el+500)%1000)/1000
        const r1 = 3+p1*10
        const r2 = 3+p2*12
        const o1 = 0.4-p1*0.4
        const o2 = 0.4-p2*0.4
        const ps=2
        const r1p=Math.floor(r1/ps)*ps
        for (let y=-r1p; y<=r1p; y+=ps) {
          for (let x=-r1p; x<=r1p; x+=ps) {
            const d=Math.sqrt(x*x+y*y)
            if (d>=r1p-ps && d<=r1p) {
              ctx.fillStyle=`rgba(0,102,255,${o1})`
              ctx.fillRect(last.x*16+8+x-ps/2, last.y*16+8+y-ps/2, ps, ps)
            }
          }
        }
        const r2p=Math.floor(r2/ps)*ps
        for (let y=-r2p; y<=r2p; y+=ps) {
          for (let x=-r2p; x<=r2p; x+=ps) {
            const d=Math.sqrt(x*x+y*y)
            if (d>=r2p-ps && d<=r2p) {
              ctx.fillStyle=`rgba(0,102,255,${o2})`
              ctx.fillRect(last.x*16+8+x-ps/2, last.y*16+8+y-ps/2, ps, ps)
            }
          }
        }
      }
    },
  
    renderCarriedObjects() {
      if (this.mainSprite && this.mainSprite.isCarrying) {
        const id=this.mainSprite.carriedItem
        const ix=this.mainSprite.x-8
        const iy=this.mainSprite.y-32-(this.objectData[id][0].b.length)
        this.drawCarriedObject(this.ctx,id,ix,iy)
      }
    },
  
    handleDebugUtilities() {
      if (typeof debug_window!=='undefined') {
        if (this.showGrid && debug_window.grid) debug_window.grid()
        if (this.showCollision && debug_window.tiles) debug_window.tiles()
        if (this.showTiles && debug_window.tiles) debug_window.tiles()
      }
    },
  
    handleLights: function() {
      if (!plugin.exists('time','lighting')) return
      const td = this.currentTileData
      const ri = this.currentRoomItem
      if (!td || !ri) return
      if (ri.visible === false && td.l && Array.isArray(td.l)) {
        td.l.forEach((lightConfig, lightIndex) => {
          const lId = `${ri.layer_id}_light_${lightIndex}`
          lighting.lights = lighting.lights.filter(l => l.id !== lId)
        })
        return
      }
      if (td.l && Array.isArray(td.l)) {
        td.l.forEach((lightConfig, lightIndex) => {
          const offsetX = lightConfig.x || 0
          const offsetY = lightConfig.y || 0
          const baseX   = Math.min(...ri.x) * 16
          const baseY   = Math.min(...ri.y) * 16
          const px      = baseX + offsetX
          const py      = baseY + offsetY
          const lId     = `${ri.layer_id}_light_${lightIndex}`
          const dh      = time.hours + time.minutes / 60
          const isNight = (dh >= 22 || dh < 7)
          const inViewport = (
            px + 200 >= this.viewportXStart * 16 &&
            px - 200 <  this.viewportXEnd   * 16 &&
            py + 200 >= this.viewportYStart * 16 &&
            py - 200 <  this.viewportYEnd   * 16
          )
          if (inViewport && isNight) {
            let existingLight = lighting.lights.find(l => l.id === lId)
            if (!existingLight) {
              const col = td.lc  || { r:255, g:255, b:255 }
              const intens = td.li  || 1
              const rad = td.lr  || 200
              const flickerSpeed = td.lfs || 0.03
              const flickerAmount = td.lfa || 0.04
              const lt = td.lt  || 'lamp'
              const shp = lightConfig.shape || null
              lighting.addLight(lId, px, py, rad, col, intens, lt, true,
                                flickerSpeed, flickerAmount, shp)
            } else {
              existingLight.x = px
              existingLight.y = py
            }
          } else {
            lighting.lights = lighting.lights.filter(l => l.id !== lId)
          }
        })
      }
    },
  
    handleEffects() {
      const td=this.currentTileData
      const ri=this.currentRoomItem
      if(!td||!ri||!this.fxData||!td.fx)return
      const fxInfo=this.fxData[td.fx]
      if(!fxInfo||!td.fxp)return
      const sx=this.viewportXStart
      const ex=this.viewportXEnd
      const sy=this.viewportYStart
      const ey=this.viewportYEnd
      td.fxp.forEach(pos=>{
        const ix=pos[0], iy=pos[1]
        if(ix>=0&&ix<ri.x.length && iy>=0&&iy<ri.y.length) {
          const tX=ri.x[ix], tY=ri.y[iy]
          const px=tX*16+8, py=tY*16+8
          const inV=(px>=sx*16&&px<ex*16&&py>=sy*16&&py<ey*16)
          const fID=`${ri.id}_${tX}_${tY}`
          if(inV) {
            if(!particles.activeEffects[fID]) {
              const opts={
                count:fxInfo.count,
                speed:fxInfo.speed,
                angle:fxInfo.baseAngle,
                spread:fxInfo.spread,
                colors:fxInfo.color.map(c=>`rgba(${c.join(',')},${fxInfo.Opacity})`),
                life:fxInfo.frames,
                size:fxInfo.size,
                type:'default',
                repeat:fxInfo.repeat,
                glow:fxInfo.Glow,
                opacity:fxInfo.Opacity,
                blur:fxInfo.Blur,
                shape:fxInfo.Shape.toLowerCase()
              }
              particles.createParticles(px,py,opts,fID)
            }
          } else {
            if(particles.activeEffects[fID]) delete particles.activeEffects[fID]
          }
        }
      })
    },
  
    renderBubbles(sp,colorHex) {
      if(!sp.bubbleEffect){
        sp.bubbleEffect={bubbles:[],duration:2000,startTime:Date.now()}
      }
      const ctx=this.ctx
      const ct=Date.now()
      const el=ct-sp.bubbleEffect.startTime
      if(el>sp.bubbleEffect.duration){delete sp.bubbleEffect;return}
      if(sp.bubbleEffect.bubbles.length<10){
        sp.bubbleEffect.bubbles.push({
          x:Math.random()*sp.width-sp.width/2,
          y:Math.random()*-10,
          radius:Math.random()*3+2,
          opacity:1,
          riseSpeed:Math.random()*0.5+0.2
        })
      }
      sp.bubbleEffect.bubbles.forEach((bb,i)=>{
        const bX=sp.x+sp.width/2+bb.x
        const bY=sp.y-bb.y
        const ah=Math.floor(bb.opacity*255).toString(16).padStart(2,'0')
        const cwo=`${colorHex}${ah}`
        ctx.fillStyle=cwo
        ctx.beginPath()
        ctx.arc(bX,bY,bb.radius,0,Math.PI*2)
        ctx.fill()
        bb.y += bb.riseSpeed*this.deltaTime/16
        bb.opacity-=0.01
        if(bb.opacity<=0||bY<sp.y-40) sp.bubbleEffect.bubbles.splice(i,1)
      })
    },
  
    updateAnimatedTiles() {
      if(!this.roomData||!this.roomData.items)return
      this.roomData.items.forEach(rItem=>{
        const it=assets.use('objectData')[rItem.id]
        if(it && it.length>0){
          if(!rItem.animationState){
            rItem.animationState=it.map(td=>({currentFrame:0,elapsedTime:0}))
          }
          it.forEach((td,idx)=>{
            if(td.i&&Array.isArray(td.i[0])&&td.d){
              const anim=td.i
              const st=rItem.animationState[idx]
              st.elapsedTime+=this.deltaTime
              if(st.elapsedTime>=td.d){
                st.elapsedTime-=td.d
                st.currentFrame=(st.currentFrame+1)%anim.length
              }
              td.currentFrame=st.currentFrame
            }
          })
        }
      })
    },
  
    getTileIdAt(x,y) {
      if(!this.roomData||!this.roomData.items)return null
      for(const itm of this.roomData.items){
        const xA=itm.x||[]
        const yA=itm.y||[]
        if(xA.includes(x)&&yA.includes(y)) return itm.id
      }
      return null
    },
  
    findObjectAt(x,y) {
      if(!this.roomData||!this.roomData.items)return null
      const arr=[]
      this.roomData.items.forEach(ri=>{
        const idata=assets.use('objectData')[ri.id]
        if(idata&&idata.length>0){
          const td=idata[0]
          const xA=ri.x||[]
          const yA=ri.y||[]
          let i=0
          for(let ty=Math.min(...yA);ty<=Math.max(...yA);ty++){
            for(let tx=Math.min(...xA);tx<=Math.max(...xA);tx++){
              const px=tx*16
              const py=ty*16
              let fr
              if(td.d){
                const cf=td.currentFrame||0
                if(Array.isArray(td.i)) fr=td.i[(cf+i)%td.i.length]
              } else {
                fr=td.i[i]
              }
              arr.push({
                tileIndex: fr,
                posX: px,
                posY: py,
                z: Array.isArray(td.z)?td.z[i%td.z.length]:td.z,
                id:ri.id,
                item:ri
              })
              i++
            }
          }
        }
      })
      arr.sort((a,b)=>a.z-b.z)
      let highest=null
      for(const it of arr){
        const r={x:it.posX,y:it.posY,width:16,height:16}
        if(x>=r.x&&x<=r.x+16&&y>=r.y&&y<=r.y+16){
          highest=it.item
        }
      }
      return highest
    },
  
    setZoomLevel(n) {
      localStorage.setItem('zoomLevel',this.zoomLevel)
    },
  
    parseRange(rs) {
      const [st,en]=rs.split('-').map(Number)
      const arr=[]
      if(st>en){for(let i=st;i>=en;i--)arr.push(i)} else {
        for(let i=st;i<=en;i++)arr.push(i)
      }
      return arr
    },
  
    expandTileData(td) {
      const e={...td}
      if(Array.isArray(td.i)){
        e.i=td.i.map(fr=>{
          if(Array.isArray(fr)){
            return fr.map(v=>{
              if(typeof v==='string'&&v.includes('-')) return this.parseRange(v)
              return v
            }).flat()
          } else if(typeof fr==='string'&&fr.includes('-')){
            return this.parseRange(fr)
          }
          return fr
        })
      }
      return e
    }
  };
  