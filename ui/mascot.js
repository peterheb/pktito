/* The mascot on the About view: a budgie of voxels, its model made by
   scripts/mascot.py into mascot-model.js and drawn here with WebGL 2 and
   nothing else. The first time it is shown it turns once; then it glances
   toward the pointer, turns under a drag or the arrow keys, and hops and
   talks at a click, Enter or Space. It draws only while something moves,
   and nothing while hidden. app.js loads both files the first time About
   opens; this one's one global is drawMascot, as app.js has the others. */
"use strict";

/* The mascot on canvas, which fills stage: { show, hide }, or null where
   there is no WebGL 2. */
function drawMascot(stage, canvas) {
  const gl = canvas.getContext("webgl2", { antialias: true, alpha: true, premultipliedAlpha: true, powerPreference: "low-power" });
  if (!gl) return null;
  const rad = (d) => (d * Math.PI) / 180;
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const calm = matchMedia("(prefers-reduced-motion: reduce)");
  const dark = matchMedia("(prefers-color-scheme: dark)");
  const PAL = MASCOT.palette;

  /* A chunk's cells, x slowest and z fastest, from its runs:
     [count, colour, count, colour, ...] */
  function cells(c) {
    const out = new Uint8Array(c.d[0] * c.d[1] * c.d[2]), r = c.runs;
    for (let i = 0, o = 0; i < r.length; i += 2) {
      if (r[i + 1]) out.fill(r[i + 1], o, o + r[i]);
      o += r[i];
    }
    return out;
  }

  /* ------------------------------ mesh: hidden faces culled, corners shaded -- */

  /* a face's normal and the two edges along it; its index is the shader's */
  const FACES = [
    { n: [1, 0, 0], u: [0, 1, 0], v: [0, 0, 1] },
    { n: [-1, 0, 0], u: [0, 0, 1], v: [0, 1, 0] },
    { n: [0, 1, 0], u: [0, 0, 1], v: [1, 0, 0] },
    { n: [0, -1, 0], u: [1, 0, 0], v: [0, 0, 1] },
    { n: [0, 0, 1], u: [1, 0, 0], v: [0, 1, 0] },
    { n: [0, 0, -1], u: [0, 1, 0], v: [1, 0, 0] },
  ];

  /* Every face of a filled cell with an empty one beside it, as a quad: per
     corner its position (3 floats) and 8 bytes, the face, how open the
     corner is from 0 to 3 (ambient occlusion), a jitter of the cell's
     shade, and its colour. */
  function meshGrid(g, dx, dy, dz, ox, oy, oz) {
    const sx = dy * dz, sy = dz;
    const solid = (x, y, z) => (x < 0 || y < 0 || z < 0 || x >= dx || y >= dy || z >= dz ? 0 : g[x * sx + y * sy + z] ? 1 : 0);
    let nf = 0;
    for (let x = 0; x < dx; x++) for (let y = 0; y < dy; y++) for (let z = 0; z < dz; z++) {
      if (!g[x * sx + y * sy + z]) continue;
      for (const F of FACES) if (!solid(x + F.n[0], y + F.n[1], z + F.n[2])) nf++;
    }
    const buf = new ArrayBuffer(nf * 4 * 20);
    const f32 = new Float32Array(buf), u8 = new Uint8Array(buf);
    const ind = new Uint32Array(nf * 6);
    let vi = 0, ii = 0;
    for (let x = 0; x < dx; x++) for (let y = 0; y < dy; y++) for (let z = 0; z < dz; z++) {
      const val = g[x * sx + y * sy + z];
      if (!val) continue;
      const rgb = PAL[val];
      const jit = ((x * 73856093) ^ (y * 19349663) ^ (z * 83492791)) & 255;
      for (let f = 0; f < 6; f++) {
        const F = FACES[f];
        const nx = x + F.n[0], ny = y + F.n[1], nz = z + F.n[2];
        if (solid(nx, ny, nz)) continue;
        const bx = x + (F.n[0] > 0 ? 1 : 0), by = y + (F.n[1] > 0 ? 1 : 0), bz = z + (F.n[2] > 0 ? 1 : 0);
        const ao = [0, 0, 0, 0];
        for (let k = 0; k < 4; k++) {
          const du = k === 1 || k === 2 ? 1 : 0, dv = k === 2 || k === 3 ? 1 : 0;
          const su = du ? 1 : -1, sv = dv ? 1 : -1;
          const s1 = solid(nx + su * F.u[0], ny + su * F.u[1], nz + su * F.u[2]);
          const s2 = solid(nx + sv * F.v[0], ny + sv * F.v[1], nz + sv * F.v[2]);
          const c = solid(nx + su * F.u[0] + sv * F.v[0], ny + su * F.u[1] + sv * F.v[1], nz + su * F.u[2] + sv * F.v[2]);
          ao[k] = s1 && s2 ? 0 : 3 - (s1 + s2 + c);
          const o = (vi + k) * 5, b = (vi + k) * 20;
          f32[o] = ox + bx + du * F.u[0] + dv * F.v[0];
          f32[o + 1] = oy + by + du * F.u[1] + dv * F.v[1];
          f32[o + 2] = oz + bz + du * F.u[2] + dv * F.v[2];
          u8[b + 12] = f;
          u8[b + 13] = ao[k];
          u8[b + 14] = jit;
          u8[b + 16] = rgb[0];
          u8[b + 17] = rgb[1];
          u8[b + 18] = rgb[2];
        }
        // the quad cut along the diagonal between its darker corners, as
        // voxel renderers do, so that the shading runs evenly across it
        const q = ao[0] + ao[2] > ao[1] + ao[3] ? [1, 2, 3, 1, 3, 0] : [0, 1, 2, 0, 2, 3];
        for (const k of q) ind[ii++] = vi + k;
        vi += 4;
      }
    }
    return { buf, ind, count: nf * 6 };
  }

  /* ---------------------------------------------------------------- shaders -- */

  function program(vs, fs) {
    const p = gl.createProgram();
    for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      gl.attachShader(p, s);
    }
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    return p;
  }

  /* a key light from the upper left, a weak fill from the right, and a sky
     over a ground for the ambient */
  const voxProg = program(`#version 300 es
    precision highp float;
    layout(location=0) in vec3 aPos;
    layout(location=1) in vec4 aMeta;
    layout(location=2) in vec4 aCol;
    uniform mat4 uMVP;
    uniform mat4 uMV;
    uniform vec3 uOffset;
    uniform float uJit;
    out vec3 vCol; out float vAO; out vec3 vN;
    const vec3 N[6] = vec3[6](vec3(1,0,0), vec3(-1,0,0), vec3(0,1,0), vec3(0,-1,0), vec3(0,0,1), vec3(0,0,-1));
    void main() {
      gl_Position = uMVP * vec4(aPos + uOffset, 1.0);
      int f = int(aMeta.x * 255.0 + 0.5);
      vN = mat3(uMV) * N[f];
      vAO = aMeta.y * 255.0 / 3.0;
      vCol = aCol.rgb * (1.0 + (aMeta.z - 0.5) * uJit);
    }`, `#version 300 es
    precision highp float;
    in vec3 vCol; in float vAO; in vec3 vN;
    uniform vec3 uL1; uniform vec3 uL2;
    out vec4 outColor;
    void main() {
      vec3 n = normalize(vN);
      float d1 = max(dot(n, uL1), 0.0);
      float d2 = max(dot(n, uL2), 0.0);
      float hemi = n.y * 0.5 + 0.5;
      float ao = mix(0.60, 1.0, smoothstep(0.0, 1.0, vAO));
      vec3 amb = mix(vec3(0.62, 0.68, 0.82), vec3(0.88, 0.90, 0.96), hemi);
      vec3 lit = amb * 0.80 + vec3(1.0, 0.97, 0.90) * d1 * 0.52 + vec3(0.72, 0.82, 1.0) * d2 * 0.14;
      outColor = vec4(min(vCol * lit * ao * 1.10, vec3(1.0)), 1.0);
    }`);
  const vu = (n) => gl.getUniformLocation(voxProg, n);
  const U = { mvp: vu("uMVP"), mv: vu("uMV"), off: vu("uOffset"), jit: vu("uJit"), l1: vu("uL1"), l2: vu("uL2") };

  /* the shadow on the ground, a soft blob under the bird, on one quad */
  const gndProg = program(`#version 300 es
    precision highp float;
    layout(location=0) in vec2 aXZ;
    uniform mat4 uMVP;
    out vec2 vXZ;
    void main() { vXZ = aXZ; gl_Position = uMVP * vec4(aXZ.x, 0.04, aXZ.y, 1.0); }`,
  `#version 300 es
    precision highp float;
    in vec2 vXZ;
    uniform vec4 uBlob;
    uniform float uStr;
    uniform vec4 uShadow;
    out vec4 outColor;
    void main() {
      vec2 d = (vXZ - uBlob.xy) / uBlob.zw;
      float a = smoothstep(1.0, 0.15, length(d)) * uStr * uShadow.a;
      outColor = vec4(uShadow.rgb * a, a);
    }`);
  const gu = (n) => gl.getUniformLocation(gndProg, n);
  const G = { mvp: gu("uMVP"), blob: gu("uBlob"), str: gu("uStr"), shadow: gu("uShadow") };

  /* ----------------------------------------------------------------- meshes -- */

  function makeMesh(m, dynamic) {
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const vb = gl.createBuffer(), ib = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 4, gl.UNSIGNED_BYTE, true, 20, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.UNSIGNED_BYTE, true, 20, 16);
    const mesh = { vao, vb, ib, count: 0, dynamic };
    uploadMesh(mesh, m);
    gl.bindVertexArray(null);
    return mesh;
  }

  function uploadMesh(mesh, m) {
    const use = mesh.dynamic ? gl.DYNAMIC_DRAW : gl.STATIC_DRAW;
    gl.bindVertexArray(mesh.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, mesh.vb);
    gl.bufferData(gl.ARRAY_BUFFER, m.buf, use);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, mesh.ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, m.ind, use);
    mesh.count = m.count;
    gl.bindVertexArray(null);
  }

  const C = MASCOT.chunks;
  const birdMesh = makeMesh(meshGrid(cells(C.bird), ...C.bird.d, ...C.bird.o), false);
  const propsMesh = makeMesh(meshGrid(cells(C.props), ...C.props.d, ...C.props.o), false);
  const bubbleBase = cells(C.bubble);
  const bubbleMesh = makeMesh({ buf: new ArrayBuffer(0), ind: new Uint32Array(0), count: 0 }, true);

  /* The waveform's bars, stamped into the bubble's cells and meshed again
     for each new set of heights. */
  const BARS = MASCOT.bars;
  let bubbleKey = "";
  function setBars(heights) {
    const key = heights.join(",");
    if (key === bubbleKey) return;
    bubbleKey = key;
    const g = bubbleBase.slice();
    const [dx, dy, dz] = C.bubble.d, [ox, oy, oz] = C.bubble.o;
    const sx = dy * dz, sy = dz;
    for (let i = 0; i < BARS.x0.length; i++) {
      const h = heights[i];
      const y0 = Math.round(BARS.yc - h / 2) - oy;
      for (let x = BARS.x0[i]; x < BARS.x0[i] + BARS.w; x++)
        for (let y = y0; y < y0 + h; y++)
          for (let z = BARS.z0; z < BARS.z1; z++) g[(x - ox) * sx + y * sy + (z - oz)] = BARS.colors[i];
    }
    uploadMesh(bubbleMesh, meshGrid(g, dx, dy, dz, ox, oy, oz));
  }
  const restHeights = BARS.base.slice();
  setBars(restHeights);

  const gndVao = gl.createVertexArray();
  gl.bindVertexArray(gndVao);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-90, -70, 110, -70, -90, 70, 110, 70]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  /* ---------------------------------------------- matrices, column-major -- */

  const mul = (a, b) => {
    const o = new Float32Array(16);
    for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
      o[c * 4 + r] = s;
    }
    return o;
  };
  const trans = (x, y, z) => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]);
  const rotX = (a) => { const c = Math.cos(a), s = Math.sin(a); return new Float32Array([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]); };
  const rotY = (a) => { const c = Math.cos(a), s = Math.sin(a); return new Float32Array([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]); };
  const persp = (fovy, asp, n, f) => {
    const t = 1 / Math.tan(fovy / 2);
    return new Float32Array([t / asp, 0, 0, 0, 0, t, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, (2 * f * n) / (n - f), 0]);
  };
  const norm3 = (v) => { const l = Math.hypot(...v); return v.map((x) => x / l); };

  /* ------------------------------------------------------------------ state -- */

  const HOME_PITCH = rad(7), PITCH_MIN = rad(-6), PITCH_MAX = rad(48);
  const dragPitch = (v) => clamp(v, PITCH_MIN - HOME_PITCH, PITCH_MAX - HOME_PITCH);
  const FOV = rad(24);
  const PIVOT = MASCOT.pivot, RXZ = MASCOT.rxz;
  const S = {
    intro: 0, introStart: -1, introDone: calm.matches,
    yawDrag: 0, pitchDrag: 0, vel: 0,
    gx: 0, gy: 0, tgx: 0, tgy: 0,
    dragging: false, lastInteract: -1e9,
    hopStart: -1, talkUntil: 0,
  };
  let shown = false, running = false, last = 0, down = null;
  const wake = () => {
    if (!shown || running) return;
    running = true;
    last = performance.now();
    requestAnimationFrame(tick);
  };
  const shadowCol = () => (dark.matches ? [0.01, 0.03, 0.09, 0.55] : [0.22, 0.36, 0.64, 0.36]);

  function resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.round(stage.clientWidth * dpr)), h = Math.max(1, Math.round(stage.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    wake();
  }
  const sized = new ResizeObserver(resize);
  dark.addEventListener("change", wake);

  /* ---------------------------------------------------------------- drawing -- */

  const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

  function draw(now) {
    const w = canvas.width, h = canvas.height, aspect = w / h;
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

    // far enough back that the whole scene fits whichever way it turns
    const halfH = 70 * 1.08, halfW = RXZ * 1.07;
    const dist = Math.max(halfH, halfW / aspect) / Math.tan(FOV / 2);
    const P = persp(FOV, aspect, dist * 0.35, dist * 2.4);
    const yaw = S.intro + S.yawDrag + S.gx;
    const pitch = clamp(HOME_PITCH + S.pitchDrag + S.gy, PITCH_MIN, PITCH_MAX);
    const MV = mul(trans(0, -2, -dist), mul(rotX(pitch), mul(rotY(yaw), trans(-PIVOT[0], -PIVOT[1], -PIVOT[2]))));
    const MVP = mul(P, MV);

    let hop = 0;
    if (S.hopStart >= 0) {
      const t = (now - S.hopStart) / 520;
      if (t >= 1) S.hopStart = -1;
      else hop = 7 * Math.sin(Math.PI * t) * (1 - 0.25 * t);
    }

    // the shadow first, without depth, then the voxels
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(gndProg);
    gl.uniformMatrix4fv(G.mvp, false, MVP);
    gl.uniform4fv(G.blob, new Float32Array(MASCOT.ground));
    // lighter while the bird is in the air
    gl.uniform1f(G.str, 1 - hop / 16);
    gl.uniform4fv(G.shadow, new Float32Array(shadowCol()));
    gl.bindVertexArray(gndVao);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    gl.disable(gl.BLEND);
    gl.enable(gl.DEPTH_TEST);
    gl.enable(gl.CULL_FACE);
    gl.useProgram(voxProg);
    gl.uniformMatrix4fv(U.mvp, false, MVP);
    gl.uniformMatrix4fv(U.mv, false, MV);
    gl.uniform3fv(U.l1, norm3([-0.5, 0.72, 0.62]));
    gl.uniform3fv(U.l2, norm3([0.78, 0.12, 0.3]));
    const drawMesh = (m, oy, jit) => {
      gl.uniform3f(U.off, 0, oy, 0);
      gl.uniform1f(U.jit, jit);
      gl.bindVertexArray(m.vao);
      gl.drawElements(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0);
    };
    drawMesh(propsMesh, 0, 0.05);
    drawMesh(bubbleMesh, 0, 0.025);
    drawMesh(birdMesh, hop, 0.11);
    gl.bindVertexArray(null);
  }

  /* --------------------------- the loop, which runs while something moves -- */

  function barsFor(now) {
    const t = now / 1000;
    return restHeights.map((b, i) => {
      const k = 0.5 + 0.5 * Math.sin(t * 9 + i * 1.7) * Math.cos(t * 5.3 + i * 0.9);
      return Math.max(2, Math.round(b * (0.35 + 0.95 * k)));
    });
  }

  function tick(now) {
    if (!shown) {
      running = false;
      return;
    }
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    let busy = false;

    // one turn, the first time it is shown
    if (!S.introDone && S.introStart >= 0) {
      const p = (now - S.introStart) / 3800;
      if (p >= 1) {
        S.intro = 0;
        S.introDone = true;
        S.talkUntil = Math.max(S.talkUntil, now + 250);
      } else {
        S.intro = Math.PI * 2 * ease(p);
        busy = true;
      }
    }

    // the glance toward the pointer
    const gk = 1 - Math.exp(-dt * 5);
    S.gx += (S.tgx - S.gx) * gk;
    S.gy += (S.tgy - S.gy) * gk;
    if (Math.abs(S.tgx - S.gx) > 1e-4 || Math.abs(S.tgy - S.gy) > 1e-4) busy = true;

    // a drag's spin running down, then an unhurried turn back home
    if (!S.dragging) {
      if (Math.abs(S.vel) > 0.02) {
        S.yawDrag += S.vel * dt;
        S.vel *= Math.exp(-dt * 3.2);
        busy = true;
      } else S.vel = 0;
      if (now - S.lastInteract > 4500 && (S.yawDrag !== 0 || S.pitchDrag !== 0)) {
        const home = Math.round(S.yawDrag / (Math.PI * 2)) * Math.PI * 2;
        const rk = 1 - Math.exp(-dt * 2.2);
        S.yawDrag += (home - S.yawDrag) * rk;
        S.pitchDrag += (0 - S.pitchDrag) * rk;
        if (Math.abs(home - S.yawDrag) < 5e-4 && Math.abs(S.pitchDrag) < 5e-4) {
          S.yawDrag = 0;
          S.pitchDrag = 0;
        }
        busy = true;
      } else if (S.yawDrag !== 0 || S.pitchDrag !== 0) busy = true;
    }

    // the bubble's waveform talks during the turn and after a chirp
    if ((!S.introDone && S.introStart >= 0) || now < S.talkUntil) {
      setBars(barsFor(Math.floor(now / 85) * 85));
      busy = true;
    } else setBars(restHeights);

    if (S.hopStart >= 0) busy = true;

    draw(now);
    if (busy) requestAnimationFrame(tick);
    else running = false;
  }

  /* ------------------------------------------------------------------ input -- */

  canvas.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    down = { x: e.clientX, y: e.clientY, moved: 0 };
    S.dragging = true;
    S.vel = 0;
    S.introDone = true;
    S.intro = 0;
    S.lastInteract = performance.now();
    canvas.setPointerCapture(e.pointerId);
    wake();
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!down || !S.dragging) return;
    const dx = e.clientX - down.x, dy = e.clientY - down.y;
    down.x = e.clientX;
    down.y = e.clientY;
    down.moved += Math.abs(dx) + Math.abs(dy);
    const dyaw = dx * 0.0105;
    S.yawDrag += dyaw;
    S.pitchDrag = dragPitch(S.pitchDrag + dy * 0.006);
    S.vel = S.vel * 0.6 + (dyaw / Math.max(0.008, (performance.now() - (down.lt || performance.now() - 16)) / 1000)) * 0.4;
    down.lt = performance.now();
    S.lastInteract = performance.now();
    wake();
  });
  canvas.addEventListener("pointerup", () => {
    if (!down) return;
    const moved = down.moved;
    down = null;
    S.dragging = false;
    S.lastInteract = performance.now();
    // under 6 px is a click, not a drag
    if (moved < 6) {
      S.vel = 0;
      chirp();
    } else S.vel = clamp(S.vel, -6, 6);
    wake();
  });
  canvas.addEventListener("pointercancel", () => {
    down = null;
    S.dragging = false;
    wake();
  });

  function chirp() {
    if (calm.matches) return;
    const now = performance.now();
    S.hopStart = now;
    S.talkUntil = now + 1400;
    wake();
  }

  // a glance toward the mouse, by small angles; added while shown
  function gaze(e) {
    if (e.pointerType !== "mouse" || S.dragging || calm.matches) return;
    const r = stage.getBoundingClientRect();
    const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
    S.tgx = clamp((e.clientX - cx) / (innerWidth / 2), -1, 1) * rad(20);
    S.tgy = clamp((e.clientY - cy) / (innerHeight / 2), -1, 1) * rad(7);
    wake();
  }
  function ungaze() {
    S.tgx = 0;
    S.tgy = 0;
    wake();
  }

  canvas.addEventListener("keydown", (e) => {
    const step = rad(15);
    let used = true;
    if (e.key === "ArrowLeft") S.yawDrag -= step;
    else if (e.key === "ArrowRight") S.yawDrag += step;
    else if (e.key === "ArrowUp") S.pitchDrag = dragPitch(S.pitchDrag - rad(6));
    else if (e.key === "ArrowDown") S.pitchDrag = dragPitch(S.pitchDrag + rad(6));
    else if (e.key === "Home") {
      S.yawDrag = Math.round(S.yawDrag / (Math.PI * 2)) * Math.PI * 2;
      S.pitchDrag = 0;
    } else if (e.key === "Enter" || e.key === " ") chirp();
    else used = false;
    if (used) {
      e.preventDefault();
      S.introDone = true;
      S.intro = 0;
      S.lastInteract = performance.now();
      wake();
    }
  });

  /* ------------------------------------------------------- shown and hidden -- */

  return {
    show() {
      shown = true;
      sized.observe(stage);
      addEventListener("pointermove", gaze, { passive: true });
      document.documentElement.addEventListener("pointerleave", ungaze);
      resize();
      if (!S.introDone && S.introStart < 0) {
        setTimeout(() => {
          if (!shown || S.introDone || S.introStart >= 0) return;
          S.introStart = performance.now();
          wake();
        }, 350);
      }
    },
    // home, for the next time; a turn under way is not begun again
    hide() {
      shown = false;
      sized.disconnect();
      removeEventListener("pointermove", gaze);
      document.documentElement.removeEventListener("pointerleave", ungaze);
      if (S.introStart >= 0) S.introDone = true;
      Object.assign(S, { intro: 0, yawDrag: 0, pitchDrag: 0, vel: 0, gx: 0, gy: 0, tgx: 0, tgy: 0, dragging: false, hopStart: -1, talkUntil: 0 });
      down = null;
    },
  };
}
