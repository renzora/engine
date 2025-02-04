game = {
  canvas: undefined,
  ctx: undefined,
  isEditMode: false,
  x: null,
  y: null,
  timestamp: 0,
  worldWidth: 1280,
  worldHeight: 944,
  zoomLevel: localStorage.getItem('zoomLevel') ? parseInt(localStorage.getItem('zoomLevel')) : 5,
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
  pathfinding: true,
  isPaused: false,
  inputMethod: 'keyboard',
  bgPattern: null,
  renderCalls: 0,
  tileCount: 0,
  spriteCount: 0,
  animationCount: 0,
  backgroundTileCount: 0,
  sceneBg: null,
  viewportXStart: 0,
  viewportXEnd: 0,
  viewportYStart: 0,
  viewportYEnd: 0,
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
      
      input.assign('mouseup', this.handleMouseUp.bind(this), {}, this.canvas)
      input.assign('contextmenu', e => e.preventDefault(), {}, this.canvas)
      input.assign('resize', e => this.resizeCanvas(e))
      input.assign('visibilitychange', () => {
        if (document.hidden) this.pause(); else this.resume()
      }, {}, document)
      
      this.resizeCanvas()
      this.loop()
      plugin.hook('onGameCreate')
      
      if (config.objectData) this.objectData = config.objectData
      if (config.spriteData) this.spriteData = config.spriteData
      if (config.player) {
        this.mainSprite = config.player
        this.playerid = this.mainSprite.id
        this.sprites[this.playerid] = this.mainSprite
      }
      this.local = config.local
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
    plugin.pathfinding.cancelPathfinding(this.sprites[this.playerid]);
        console.log("fetching from database");
        fetch(`/api/scenes/${encodeURIComponent(sceneId)}`, {
            method: 'GET',
            headers: { 'Content-Type': 'application/json' }
        })
        .then(r => { if (!r.ok) throw new Error(r.statusText); return r.json(); })
        .then(d => {
            if (d.message === 'success') {
                plugin.lighting.clearLightsAndEffects();
                this.roomData = d.roomData;
                this.sceneid = d._id;
                this.serverid = d.server_id;
                localStorage.setItem('sceneid', d._id);
                this.worldWidth = d.width || 1280;
                this.worldHeight = d.height || 944;
                this.x = d.startingX || 0;
                this.y = d.startingY || 0;
                this.roomData.items = this.roomData.items.filter(item => this.objectData[item.id]);
                const p = this.sprites[this.playerid];
                if (p) { p.x = this.x; p.y = this.y; }
                this.sceneBg = d.bg || null;
                this.resizeCanvas();
                this.overlappingTiles = [];
                camera.update();
                plugin.effects.start('fadeOut', 1000);
                plugin.effects.start('fadeIn', 1000);
                this.buildRepeatingBackground();
            }
        })
        .catch(() => {
            plugin.load('errors', { ext: 'html' });
        });
},

loop(timestamp) {
  if (!this.lastTime) {
    this.lastTime = timestamp;
    this.lastFpsUpdateTime = timestamp;
    requestAnimationFrame(this.loop.bind(this));
    return;
  }

  const frameTime = 1000 / 60; // Target 60 FPS
  const dt = timestamp - this.lastTime;
  
  if (dt < frameTime) {
    requestAnimationFrame(this.loop.bind(this));
    return;
  }

  if (dt > 1000) {
    this.accumulatedTime = this.fixedDeltaTime;
  } else {
    this.accumulatedTime += dt;
  }

  this.deltaTime = this.fixedDeltaTime;
  this.lastTime = timestamp;

  while (this.accumulatedTime >= this.fixedDeltaTime) {
    this.viewportXStart = Math.floor(camera.cameraX / 16);
    this.viewportXEnd   = Math.ceil((camera.cameraX + window.innerWidth / this.zoomLevel) / 16);
    this.viewportYStart = Math.floor(camera.cameraY / 16);
    this.viewportYEnd   = Math.ceil((camera.cameraY + window.innerHeight / this.zoomLevel) / 16);
    this.viewportXStart = Math.max(0, this.viewportXStart);
    this.viewportXEnd   = Math.min(this.worldWidth / 16, this.viewportXEnd);
    this.viewportYStart = Math.max(0, this.viewportYStart);
    this.viewportYEnd   = Math.min(this.worldHeight / 16, this.viewportYEnd);

    for (let id in this.sprites) {
      const sp = this.sprites[id];
      const r = sp.x + sp.width;
      const b = sp.y + sp.height;
      if (r >= this.viewportXStart*16 && sp.x < this.viewportXEnd*16 &&
          b >= this.viewportYStart*16 && sp.y < this.viewportYEnd*16) {
        if (sp.update) sp.update();
      }
    }

    camera.update();
    this.updateAnimatedTiles();
    this.accumulatedTime -= this.fixedDeltaTime;
  }

  this.ctx.imageSmoothingEnabled = false;
  this.ctx.setTransform(1, 0, 0, 1, 0, 0);
  this.ctx.scale(this.zoomLevel, this.zoomLevel);
  this.ctx.translate(-Math.round(camera.cameraX), -Math.round(camera.cameraY));
  this.renderCalls = 0;
  this.tileCount = 0;
  this.renderBackground();
  this.render();
  plugin.hook('onRender');
  if (plugin.ui_console_editor_inventory.selectedInventoryItem) plugin.ui_console_editor_inventory.render();

  plugin.debug.tracker('game.loop');
  requestAnimationFrame(this.loop.bind(this));
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
    this.ctx.save();
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);
    this.ctx.fillStyle = 'black';
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
    this.ctx.restore();
    this.ctx.setTransform(this.zoomLevel, 0, 0, this.zoomLevel,
      -Math.round(camera.cameraX * this.zoomLevel),
      -Math.round(camera.cameraY * this.zoomLevel));
    this.ctx.save();
    this.ctx.beginPath();
    this.ctx.rect(0, 0, this.worldWidth, this.worldHeight);
    this.ctx.clip();
    if (this.bgPattern) {
      this.ctx.fillStyle = this.bgPattern;
      this.ctx.fillRect(0, 0, this.worldWidth, this.worldHeight);
    }
    this.ctx.restore();
    plugin.hook('onRenderBackground');
    this.renderCalls++;
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
    this.renderQueue = [];
    plugin.hook('onRenderAll');
    const expanded = Object.keys(this.objectData).reduce((acc, key) => {
      acc[key] = this.objectData[key].map(this.expandTileData.bind(this));
      return acc;
    }, {});
    this.ctx.setTransform(this.zoomLevel, 0, 0, this.zoomLevel,
      -Math.round(camera.cameraX * this.zoomLevel),
      -Math.round(camera.cameraY * this.zoomLevel));
    if (this.roomData && this.roomData.items) {
      this.roomData.items.forEach(rItem => {
        const iData = expanded[rItem.id];
        if (!iData || iData.length === 0) return;
        const tData = iData[0];
        if (rItem.visible === false) {
          this.currentTileData = tData;
          this.currentRoomItem = rItem;
          return;
        }
        const xC = rItem.x || [];
        const yC = rItem.y || [];
        const minX = Math.min(...xC);
        const maxX = Math.max(...xC);
        const minY = Math.min(...yC);
        const maxY = Math.max(...yC);
        const objL = minX * 16;
        const objR = (maxX + 1) * 16;
        const objT = minY * 16;
        const objB = (maxY + 1) * 16;
        const cL = this.viewportXStart * 16;
        const cR = this.viewportXEnd * 16;
        const cT = this.viewportYStart * 16;
        const cB = this.viewportYEnd * 16;
        if (objR < cL || objL > cR || objB < cT || objT > cB) return;
        const cacheKey = rItem.id + '_' + (tData.currentFrame || 0);
        let offscreen = this.objectCache[cacheKey];
        if (!offscreen) {
          offscreen = this.buildObjectCanvas(cacheKey, tData, xC, yC);
        }
        let zIndex;
        if (tData.z === 0) {
          zIndex = -9999;
        } else {
          zIndex = maxY * 16;
          if (!tData.s) {
            const overlappingItems = this.roomData.items.filter(other => {
              if (other === rItem) return false;
              const otherData = expanded[other.id]?.[0];
              if (!otherData || !otherData.s) return false;
              const otherX = other.x || [];
              const otherY = other.y || [];
              const ox1 = Math.min(...otherX) * 16;
              const ox2 = (Math.max(...otherX) + 1) * 16;
              const oy1 = Math.min(...otherY) * 16;
              const oy2 = (Math.max(...otherY) + 1) * 16;
              const thisX1 = minX * 16;
              const thisX2 = (maxX + 1) * 16;
              const thisY1 = minY * 16;
              const thisY2 = (maxY + 1) * 16;
              return (
                thisX1 < ox2 && thisX2 > ox1 &&
                thisY1 < oy2 && thisY2 > oy1
              );
            });
            if (overlappingItems.length > 0) {
              let highestSurfaceZ = 0;
              overlappingItems.forEach(surface => {
                const surfaceY = Math.max(...(surface.y || []));
                const surfaceZ = surfaceY * 16;
                highestSurfaceZ = Math.max(highestSurfaceZ, surfaceZ);
              });
              zIndex = highestSurfaceZ + 1;
            }
          }
        }
        this.renderQueue.push({
          zIndex: zIndex,
          type: 'object',
          id: rItem.id,
          visible: true,
          layer_id: "item_" + rItem.layer_id,
          data: {
            offscreen
          },
          draw: () => {
            this.ctx.save();
            const pivotX = (minX + maxX + 1) / 2 * 16;
            const pivotY = (maxY + 1) * 16;
            this.ctx.translate(pivotX, pivotY);
            const scale = rItem.scale || 1;
            this.ctx.scale(scale, scale);
            if (rItem.flipHorizontal) {
              this.ctx.scale(-1, 1);
            }
            const dx = -offscreen.width / 2;
            const dy = -offscreen.height;
            this.ctx.drawImage(offscreen, dx, dy);
            this.renderCalls++;
            this.tileCount++;
            this.ctx.restore();
          }
        });
        this.currentTileData = tData;
        this.currentRoomItem = rItem;
      });
    }
    for (let id in this.sprites) {
      const sp = this.sprites[id];
      const r = sp.x + sp.width;
      const b = sp.y + sp.height;
      if (
        r >= this.viewportXStart * 16 &&
        sp.x < this.viewportXEnd * 16 &&
        b >= this.viewportYStart * 16 &&
        sp.y < this.viewportYEnd * 16
      ) {
        const z = sp.y + sp.height;
        this.renderQueue.push({
          zIndex: z,
          type: 'sprite',
          id: id,
          data: { sprite: sp },
          draw: () => {
            sp.drawShadow();
            sp.draw();
          }
        });
        this.spriteCount++;
      }
    }
    this.renderQueue.sort((a, b) => a.zIndex - b.zIndex);
    this.renderQueue.forEach(item => {
      item.draw();
    });
    plugin.hook('onRender');
    plugin.debug.tracker("render.renderAll");
  },

  handleMouseUp(e) {
    if (this.isEditMode || (this.mainSprite && this.mainSprite.targetAim)) return
    const r = this.canvas.getBoundingClientRect()
    const mx = (e.clientX - r.left)/this.zoomLevel + camera.cameraX
    const my = (e.clientY - r.top)/this.zoomLevel + camera.cameraY
    this.x = Math.floor(mx/16)
    this.y = Math.floor(my/16)
    if (plugin.exists('pathfinding')) {
      plugin.pathfinding.walkToClickedTile(this.mainSprite, e, this.x, this.y);
    }
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