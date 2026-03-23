// Orbit camera around a target entity
  // Attach this script to the camera entity

  fn props() {
      #{
          orbit_speed: #{ value: 30.0, hint: "Degrees per second" },
          orbit_radius: #{ value: 8.0, hint: "Distance from target" },
          orbit_height: #{ value: 4.0, hint: "Height above target" },
          target: #{ value: "Cube", hint: "Name of entity to orbit around" },
          angle: #{ value: 0.0, hint: "Internal: current angle" },
      }
  }

  fn on_update() {
      angle += orbit_speed * delta;

      if angle >= 360.0 {
          angle -= 360.0;
      }

      // Read target entity's position
      let tx = get_on(target, "transform.translation.x");
      let ty = get_on(target, "transform.translation.y");
      let tz = get_on(target, "transform.translation.z");

      if tx == () { tx = 0.0; }
      if ty == () { ty = 0.0; }
      if tz == () { tz = 0.0; }

      let rad = angle * 3.14159265358979 / 180.0;
      let x = tx + cos(rad) * orbit_radius;
      let z = tz + sin(rad) * orbit_radius;

      set_position(x, ty + orbit_height, z);
      look_at(tx, ty, tz);
  }