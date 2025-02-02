gamepad = {
    gamepadIndex: null,
    buttons: [],
    axes: [],
    isConnected: false,
    name: '',
    buttonPressures: {},
    axesPressures: {},
    throttledEvents: {},
    buttonOverwrite: { leftButton: null, rightButton: null, aButton: null, bButton: null },
    playerLimit: 2,
    assignedControllers: {},
    spritesheetButtonMap: {
      ps5: ['up','left','down','right','leftstick','leftstickpressed','rightstick','rightstickpressed','x','square','circle','triangle','l1','l2','r1','r2'],
      xbox: ['up','left','down','right','leftstick','leftstickpressed','rightstick','rightstickpressed','a','x','b','y','lb','lt','rb','rt'],
      switch: ['up','left','down','right','leftstick','leftstickpressed','rightstick','rightstickpressed','b','y','a','x','l','zl','r','zr']
    },
    defaultPlatform: 'ps5',
    button_size: 16,
    display_size: 32,
  
    start() {
        assets.preload([
            { name: 'gamepad_buttons', path: 'plugins/gamepad/buttons.png' }
        ]);
      console.log(`Plugin started: ${this.id}`)
      window.addEventListener('gamepadconnected', (e) => this.connectGamepad(e))
      window.addEventListener('gamepaddisconnected', (e) => this.disconnectGamepad(e))
      window.addEventListener('gamepada', (e) => this.gamepadAButton(e))
      window.addEventListener('gamepadb', (e) => this.gamepadBButton(e))
      window.addEventListener('gamepadyPressed', this.throttle((e) => this.gamepadYButtonPressed(e), 1000))
      window.addEventListener('gamepadyReleased', this.throttle((e) => this.gamepadYButtonReleased(e), 1000))
      window.addEventListener('gamepadxPressed', (e) => this.gamepadXButton(e))
      window.addEventListener('gamepadxReleased', (e) => this.gamepadXButtonReleased(e))
      window.addEventListener('gamepady', (e) => this.gamepadYButton(e))
      window.addEventListener('gamepadl1Pressed', (e) => this.gamepadLeftBumper(e.detail))
      window.addEventListener('gamepadstartPressed', this.throttle((e) => this.gamepadStart(e), 1000))
      window.addEventListener('gamepadAxes', (e) => this.handleAxes(e.detail))
      window.addEventListener('gamepadl2Pressed', (e) => this.gamepadLeftTrigger(e))
      window.addEventListener('gamepadr2Pressed', this.throttle((e) => this.gamepadRightTrigger(e), 50))
      window.addEventListener('gamepadr2Released', this.throttle(() => this.gamepadRightTriggerReleased(), 50))
      window.addEventListener('gamepadl2Released', (e) => this.gamepadLeftTrigger())
      window.addEventListener('gamepadrightStickPressed', this.throttle(() => {
        this.toggleSubmenu()
        this.flashR3Button()
      }, 500))
      this.loop = setInterval(() => this.updateGamepadState(), 16)
    },
  
    unmount() {
      clearInterval(this.loop)
      window.removeEventListener('gamepadconnected', (e) => this.connectGamepad(e))
      window.removeEventListener('gamepaddisconnected', (e) => this.disconnectGamepad(e))
      this.isConnected = false
      this.gamepadIndex = null
      console.log(`Plugin unmounted: ${this.id}`)
    },
  
    onRender() {
      this.updateGamepadState()
    },
  
    throttle(func, delay) {
      return (...args) => {
        const now = Date.now()
        const lastCall = this.throttledEvents[func] || 0
        if (now - lastCall >= delay) {
          this.throttledEvents[func] = now
          func(...args)
        }
      }
    },
  
    connectGamepad(e) {
      this.gamepadIndex = e.gamepad.index
      this.isConnected = true
      this.name = this.getGamepadName(e.gamepad)
      input.updateInputMethod('gamepad', this.name)
      const event = new CustomEvent('gamepadConnected')
      window.dispatchEvent(event)
    },
  
    disconnectGamepad(e) {
      if (e.gamepad.index === this.gamepadIndex) {
        this.isConnected = false
        this.gamepadIndex = null
        input.updateInputMethod('keyboard')
        const event = new CustomEvent('gamepadDisconnected')
        window.dispatchEvent(event)
      }
    },
  
    assignController(player, gamepadIndex) {
      if (Object.keys(this.assignedControllers).length < this.playerLimit) {
        this.assignedControllers[player] = gamepadIndex
      }
    },
  
    unassignController(player) {
      if (this.assignedControllers[player]) {
        delete this.assignedControllers[player]
      }
    },
  
    updateGamepadState() {
      if (this.isConnected && this.gamepadIndex !== null) {
        const gp = navigator.getGamepads()[this.gamepadIndex]
        if (gp) {
          if (this.hasActiveInput(gp)) {
            input.updateInputMethod('gamepad', this.name)
          }
          this.handleButtons(gp.buttons)
          const axesEvent = new CustomEvent('gamepadAxes', { detail: gp.axes })
          window.dispatchEvent(axesEvent)
        }
      }
    },
  
    hasActiveInput(gp) {
      const t = 0.2
      const bp = gp.buttons.some(b => b.pressed)
      const am = gp.axes.some(a => Math.abs(a) > t)
      return bp || am
    },
  
    getGamepadName(gp) {
      const map = {
        '045e:02e0': 'Xbox 360','045e:028e': 'Xbox One','054c:0ce6': 'PS5','054c:05c4': 'PS4','057e:2009': 'Nintendo Switch Pro Controller','046d:c216': 'Logitech F310','1038:1412': 'SteelSeries Stratus Duo'
      }
      const id = gp.id
      const m = id.match(/Vendor:\s*([0-9a-fA-F]{4})\s*Product:\s*([0-9a-fA-F]{4})/)
      if (m) {
        const vp = `${m[1].toLowerCase()}:${m[2].toLowerCase()}`
        if (map[vp]) return map[vp]
      }
      if (id.toLowerCase().includes('xbox')) return 'Xbox'
      if (id.toLowerCase().includes('nintendo') || id.toLowerCase().includes('switch')) return 'Switch'
      if (id.toLowerCase().includes('logitech')) return 'Logitech'
      if (id.toLowerCase().includes('steelseries')) return 'SteelSeries'
      return 'Generic'
    },
  
    handleButtons(buttons) {
      const names = this.getButtonNames()
      buttons.forEach((btn, i) => {
        const pressure = btn.value
        this.buttonPressures[i] = pressure
        const n = names[i]
        if (btn.pressed) {
          if (!this.buttons.includes(n)) {
            this.buttons.push(n)
            this.emitButtonEvent(i, 'down', pressure)
          } else {
            this.emitButtonEvent(i, 'down', pressure)
          }
        } else {
          const idx = this.buttons.indexOf(n)
          if (idx > -1) {
            this.buttons.splice(idx, 1)
            this.emitButtonEvent(i, 'up', pressure)
          }
        }
      })
    },
  
    emitButtonEvent(i, s, pressure) {
      const names = this.getButtonNames()
      if (typeof i === 'number' && names[i] !== undefined) {
        const name = names[i]
        const eventName = `gamepad${name}${s === 'down' ? 'Pressed' : 'Released'}`
        const evt = new CustomEvent(eventName, { detail:{ s, pressure, buttonName: name } })
        window.dispatchEvent(evt)
        const active = plugin.getActivePlugin()
        if (active && window[active]) {
          const downFn = name+'Button'
          const upFn = name+'ButtonReleased'
          if (s === 'down' && typeof window[active][downFn] === 'function') {
            window[active][downFn](pressure, s)
          } else if (s === 'up' && typeof window[active][upFn] === 'function') {
            window[active][upFn](pressure, s)
          }
        }
      }
    },
  
    getButtonNames() {
      return ['a','b','x','y','l1','r1','l2','r2','select','start','leftStick','rightStick','up','down','left','right']
    },
  
    vibrate(d, strong=1.0, weak=1.0) {
      const gp = navigator.getGamepads()[this.gamepadIndex]
      if (this.isConnected && gp && gp.vibrationActuator) {
        gp.vibrationActuator.playEffect('dual-rumble', { duration:d, startDelay:0, strongMagnitude:strong, weakMagnitude:weak }).catch(err=>console.log(err))
      }
    },
  
    updateButtonImages() {
      const sheet = assets.use('gamepad_buttons')
      let p = this.name || this.defaultPlatform
      if (!this.spritesheetButtonMap[p]) p = this.defaultPlatform
      const arr = this.spritesheetButtonMap[p]
      const row = this.getPlatformRow(p)
      const sc = this.display_size / this.button_size
      if (!arr) return
      arr.forEach((b, i) => {
        const els = document.querySelectorAll(`.gamepad_button_${b}`)
        const x = i*this.button_size
        const y = row*this.button_size
        els.forEach(el => {
          el.style.width = `${this.display_size}px`
          el.style.height = `${this.display_size}px`
          el.style.backgroundImage = `url('${sheet.src}')`
          el.style.backgroundPosition = `-${x*sc}px -${y*sc}px`
          el.style.backgroundSize = `${this.button_size*16*sc}px ${this.button_size*3*sc}px`
          el.style.backgroundRepeat = 'no-repeat'
          el.style.display='inline-block'
        })
      })
    },
  
    clearButtonImages() {
      const arr = this.spritesheetButtonMap.ps5
      arr.forEach(b => {
        const els = document.querySelectorAll(`.gamepad_button_${b}`)
        els.forEach(el => { el.style.backgroundImage='' })
      })
    },
  
    getPlatformRow(p) {
      const map = { ps5:0, xbox:1, switch:2 }
      return map[p] !== undefined ? map[p] : 0
    },
  
    gamepadAButton(e) {
      if (!game.mainSprite) return
      console.log("A button pressed")
    },
  
    gamepadBButton(e) {
      if (!game.mainSprite) return
      console.log("B button pressed")
    },
  
    gamepadXButton(e) {
      if (plugin.overlay.remainingBullets<plugin.overlay.bulletsPerRound && plugin.overlay.remainingRounds>0 && !plugin.overlay.isReloading) {
        plugin.plugin.overlay.startReloading()
      } else if (plugin.overlay.remainingRounds<=0) {
        plugin.audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false)
      }
    },
  
    gamepadXButtonReleased(e) {
      if (plugin.plugin.overlay.isReloading) {
        plugin.plugin.overlay.stopReloading()
      }
      plugin.plugin.overlay.justReloaded = false
    },
  
    gamepadYButtonPressed() {
      if (!game.mainSprite||!game.sprites[game.playerid]) return
      this.isYButtonHeld = true
      const s = game.mainSprite
      const pid = game.playerid
      const r = 32
      if (s.isVehicle) {
        const p = game.sprites[s.riderId||pid]
        if (p) {
          p.x = s.x
          p.y = s.y
          p.activeSprite = true
          game.playerid = p.id
          s.riderId = null
          plugin.show('ui_inventory_window')
        }
      } else {
        const near = Object.values(game.sprites).find(q => {
          if(!q.isVehicle) return false
          const dx = q.x - s.x
          const dy = q.y - s.y
          const dist = Math.sqrt(dx*dx+dy*dy)
          return dist<=r
        })
        if(near) {
          game.playerid = near.id
          near.riderId = pid
          s.activeSprite = false
          plugin.minimize('ui_inventory_window')
          plugin.front('plugin.overlay')
        }
        else console.log("No nearby vehicle within radius.")
      }
      game.mainSprite = game.sprites[game.playerid]
    },
  
    gamepadYButtonReleased(e) {
      this.isYButtonHeld = false
    },
  
    gamepadYButton(e) {},
  
    gamepadLeftTrigger(e) {
      const s = game.mainSprite
      if(!s) return
      const pressure = e?.detail?.pressure||0
      if(s.isVehicle) {
        if(pressure>0) {
          if(s.currentSpeed>0) {
            s.currentSpeed = Math.max(0, s.currentSpeed - s.braking*pressure*(game.deltaTime/16.67))
          } else {
            s.currentSpeed = Math.max(-s.maxSpeed, s.currentSpeed - (s.acceleration*10)*pressure*(game.deltaTime/16.67))
          }
          s.moveVehicle()
        }
      } else {
        if(this.buttons.includes('l2')) {
          if(s) s.targetAim = true
        } else {
          if(s) s.targetAim = false
        }
      }
    },
  
    gamepadRightTrigger(e) {
      const s = game.mainSprite
      if(!s) return
      const pressure = e.detail.pressure||0
      if(s.isVehicle) {
        if(pressure>0) {
          s.currentSpeed = Math.min(s.maxSpeed, s.currentSpeed + s.acceleration*pressure*(game.deltaTime/16.67))
        } else {
          s.currentSpeed = Math.max(0, s.currentSpeed - s.braking*(game.deltaTime/16.67))
        }
        s.moveVehicle()
      } else if(s.canShoot) {
        if(s.targetAim && s.canShoot) {
          if(plugin.overlay.remainingBullets>0) {
            s.dealDamage()
            plugin.plugin.overlay.updateBullets(plugin.overlay.remainingBullets-1)
            plugin.audio.playAudio("machinegun1", assets.use('machinegun1'), 'sfx', true)
            plugin.effects.shakeMap(200,2)
            s.overrideAnimation = 'shooting_gun'
          } else {
            plugin.audio.stopLoopingAudio('machinegun1','sfx',1.0)
            if(plugin.overlay.remainingRounds>0) {
                plugin.audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false)
                plugin.ui.notif("no_bullets_notif","Out of bullets! Press 'X' to reload.",true)
              s.overrideAnimation = null
            } else {
                plugin.audio.playAudio("empty_gun", assets.use('empty_gun'), 'sfx', false)
                plugin.plugin.overlay.noBulletsLeft()
            }
          }
        }
      }
    },
  
    gamepadRightTriggerReleased() {
      if(!game.mainSprite) return
      this.changeSpeed()
      plugin.audio.stopLoopingAudio('machinegun1','sfx',1.0)
      const p = game.mainSprite
      p.changeAnimation('shooting_gun')
    },
  
    changeSpeed() {
      game.mainSprite.speed = 70
      game.mainSprite.overrideAnimation = null
    },
  
    gamepadLeftBumper(e) {
      if(!game.mainSprite) return
      if(this.buttons.includes('l1')) {
        const p = game.mainSprite
        const t = this.findNearestTarget(p.targetX, p.targetY, p.maxRange)
        if(t) {
          const cx = t.x + (t.width ? t.width/2 : 0)
          const cy = t.y + (t.height ? t.height/2 : 0)
          if(this.isWithinMaxRange(t, p)) {
            p.targetX = cx
            p.targetY = cy
            p.targetAim = true
          } else {
            const pcx = p.x + p.width/2
            const pcy = p.y + p.height/2
            const dx = cx - pcx
            const dy = cy - pcy
            const angle = Math.atan2(dy, dx)
            p.targetX = pcx + Math.cos(angle)*p.maxRange
            p.targetY = pcy + Math.sin(angle)*p.maxRange
            p.targetAim = true
          }
        } else {
          p.targetAim = false
        }
      } else {
        game.mainSprite.targetAim = false
      }
    },
  
    handleAxes(axes) {
      this.handleLeftAxes(axes)
      this.handleRightAxes(axes)
    },
  
    handleLeftAxes(axes) {
      const t = 0.1
      const lx = axes[0]
      const ly = axes[1]
      if(!game.mainSprite) return
      const s = game.mainSprite
      if(s.isVehicle) {
        if(Math.abs(lx)>t) s.updateVehicleDirection(lx, game.deltaTime)
        else s.updateVehicleDirection(0, game.deltaTime)
        const r2p = this.axesPressures.rightTrigger||0
        const l2p = this.axesPressures.leftTrigger||0
        if(r2p<t && l2p<t) {
          s.currentSpeed = Math.max(0, s.currentSpeed - s.braking*(game.deltaTime/1000))
        }
        s.moveVehicle()
      } else {
        this.directions = { left:false, right:false, up:false, down:false }
        if(Math.abs(lx)>t || Math.abs(ly)>t) {
          const p = Math.min(1, Math.sqrt(lx**2+ly**2))
          const ms = 10
          const ts = s.topSpeed
          const r2Boost = this.buttons.includes('r2') ? 1.0 : 0.6
          const relSp = ms + (p*(ts-ms)*r2Boost)
          s.speed = Math.min(relSp, ts)
          const angle = Math.atan2(ly, lx)
          this.updateGamepadDirections(angle)
          this.updateSpriteDirections()
          //plugin.effects.dirtCloudEffect.create(s, '#DAF7A6')
        } else {
          this.axesPressures.leftStickX = 0
          this.axesPressures.leftStickY = 0
          if(s) s.speed = 0
          this.updateSpriteDirections()
        }
      }
    },
  
    updateGamepadDirections(a) {
      const up = (a>=-Math.PI/8 && a<Math.PI/8)
      const upRight = (a>=Math.PI/8 && a<3*Math.PI/8)
      const right = (a>=3*Math.PI/8 && a<5*Math.PI/8)
      const downRight = (a>=5*Math.PI/8 && a<7*Math.PI/8)
      const down = (a>=7*Math.PI/8||a<-7*Math.PI/8)
      const downLeft = (a>=-7*Math.PI/8 && a<-5*Math.PI/8)
      const left = (a>=-5*Math.PI/8 && a<-3*Math.PI/8)
      const upLeft = (a>=-3*Math.PI/8 && a<-Math.PI/8)
      if(up) { this.directions.right = true }
      else if(upRight) { this.directions.down = true; this.directions.right = true }
      else if(right) { this.directions.down = true }
      else if(downRight) { this.directions.down = true; this.directions.left = true }
      else if(down) { this.directions.left = true }
      else if(downLeft) { this.directions.up = true; this.directions.left = true }
      else if(left) { this.directions.up = true }
      else if(upLeft) { this.directions.up = true; this.directions.right = true }
    },
  
    handleRightAxes(axes) {
      if(!game.mainSprite) return
      const d = 0.1
      const rx = axes[2]
      const ry = axes[3]
      if(Math.abs(rx)>d || Math.abs(ry)>d) {
        this.axesPressures.rightStickX = Math.abs(rx)
        this.axesPressures.rightStickY = Math.abs(ry)
        if(game.mainSprite && game.mainSprite.targetAim) {
          const as = 10
          const nx = game.mainSprite.targetX + rx*as
          const ny = game.mainSprite.targetY + ry*as
          const dx = nx - (game.mainSprite.x + game.mainSprite.width/2)
          const dy = ny - (game.mainSprite.y + game.mainSprite.height/2)
          const dist = Math.sqrt(dx*dx + dy*dy)
          const ang = Math.atan2(dy, dx)
          if(ang>=-Math.PI/8 && ang<Math.PI/8) game.mainSprite.direction='E'
          else if(ang>=Math.PI/8 && ang<3*Math.PI/8) game.mainSprite.direction='SE'
          else if(ang>=3*Math.PI/8 && ang<5*Math.PI/8) game.mainSprite.direction='S'
          else if(ang>=5*Math.PI/8 && ang<7*Math.PI/8) game.mainSprite.direction='SW'
          else if(ang>=7*Math.PI/8||ang<-7*Math.PI/8) game.mainSprite.direction='W'
          else if(ang>=-7*Math.PI/8 && ang<-5*Math.PI/8) game.mainSprite.direction='NW'
          else if(ang>=-5*Math.PI/8 && ang<-3*Math.PI/8) game.mainSprite.direction='N'
          else if(ang>=-3*Math.PI/8 && ang<-Math.PI/8) game.mainSprite.direction='NE'
          if(dist<=game.mainSprite.maxRange) {
            game.mainSprite.targetX = nx
            game.mainSprite.targetY = ny
          } else {
            const mx = game.mainSprite.x + game.mainSprite.width/2 + Math.cos(ang)*game.mainSprite.maxRange
            const my = game.mainSprite.y + game.mainSprite.height/2 + Math.sin(ang)*game.mainSprite.maxRange
            game.mainSprite.targetX = Math.max(0, Math.min(mx, game.worldWidth))
            game.mainSprite.targetY = Math.max(0, Math.min(my, game.worldHeight))
          }
        }
      } else {
        this.axesPressures.rightStickX = 0
        this.axesPressures.rightStickY = 0
      }
    },
  
    isWithinMaxRange(t, p) {
      const cx = t.x + (t.width ? t.width/2 : 0)
      const cy = t.y + (t.height ? t.height/2 : 0)
      const px = p.x + p.width/2
      const py = p.y + p.height/2
      const dx = cx - px
      const dy = cy - py
      const dist = Math.sqrt(dx*dx + dy*dy)
      return dist<=p.maxRange
    },
  
    toggleSubmenu() {
      const sb = document.getElementById('submenu')
      if(sb) {
        sb.classList.toggle('max-h-0')
        sb.classList.toggle('max-h-[500px]')
      }
    },
  
    flashR3Button() {
      const b = document.getElementById('toggle-submenu')
      if(b) {
        b.classList.add('bg-green-500')
        setTimeout(() => { b.classList.remove('bg-green-500') }, 200)
      }
    },
  
    findNearestTarget(cx, cy, r) {
      let near = null
      let dist = Infinity
      for(let id in game.sprites) {
        const s = game.sprites[id]
        if(s.isEnemy) {
          const sx = s.x + (s.width ? s.width/2 : 0)
          const sy = s.y + (s.height ? s.height/2 : 0)
          const dd = Math.sqrt((cx - sx)**2 + (cy - sy)**2)
          if(dd<dist && dd<=r) {
            dist = dd
            near = s
          }
        }
      }
      if(game.roomData && game.roomData.items) {
        game.roomData.items.forEach(i => {
          const d = assets.use('objectData')[i.id]
          if(d) {
            const ix = i.x[0]*16 + 8
            const iy = i.y[0]*16 + 8
            const dd = Math.sqrt((cx - ix)**2 + (cy - iy)**2)
            if(dd<dist && dd<=r) {
              dist = dd
              near = { ...i, x: ix, y: iy }
            }
          }
        })
      }
      return near
    },
  
    updateSpriteDirections() {
      if(!game.allowControls) return
      const combo = {
        up: (this.directions && this.directions.up) || false,
        down: (this.directions && this.directions.down) || false,
        left: (this.directions && this.directions.left) || false,
        right: (this.directions && this.directions.right) || false
      }
      const arr = ['up','down','left','right']
      arr.forEach(d => {
        if(game.mainSprite) {
          if(combo[d]) game.mainSprite.addDirection(d)
          else game.mainSprite.removeDirection(d)
        }
      })
      if(game.mainSprite && !combo.up && !combo.down && !combo.left && !combo.right) {
        plugin.audio.stopLoopingAudio('footsteps1','sfx',0.5)
      }
    }
  }
  