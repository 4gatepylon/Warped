// Keyboard and mouse for the world view: held keys, accumulated mouse movement, pointer lock.

export class Input {
  /**
   * @param {HTMLCanvasElement} canvas  element that captures the mouse
   * @param {object} on  { exit(), key(code) } – exit is called for Esc / losing the pointer lock
   */
  constructor(canvas, on) {
    this.canvas = canvas;
    this.on = on;
    this.keys = new Set();
    this.mouse = [0, 0];
    this.active = false;
    this.dragging = false;

    window.addEventListener('keydown', (e) => {
      if (!this.active || e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
      if (e.code === 'Escape') { this.on.exit(); return; }
      this.keys.add(e.code);
      if (!e.repeat) this.on.key(e.code);
      if (['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.code)) e.preventDefault();
    });
    window.addEventListener('keyup', (e) => this.keys.delete(e.code));
    window.addEventListener('blur', () => this.keys.clear());

    canvas.addEventListener('pointerdown', (e) => {
      if (!this.active) return;
      this.dragging = true;
      try { canvas.setPointerCapture(e.pointerId); } catch (_) { /* ignore */ }
      if (e.pointerType === 'mouse' && !this.locked) this.lock();
    });
    canvas.addEventListener('pointerup', () => { this.dragging = false; });
    canvas.addEventListener('pointermove', (e) => {
      if (!this.active || (!this.locked && !this.dragging)) return;
      this.mouse[0] += e.movementX || 0;
      this.mouse[1] += e.movementY || 0;
    });
    // Esc while the mouse is captured releases it without a keydown event: treat that as exit too.
    document.addEventListener('pointerlockchange', () => {
      if (this.active && !this.locked && this.wasLocked) this.on.exit();
      this.wasLocked = this.locked;
    });
  }

  get locked() { return document.pointerLockElement === this.canvas; }

  lock() {
    try {
      const p = this.canvas.requestPointerLock();
      if (p && p.catch) p.catch(() => { /* not allowed right now: dragging still works */ });
    } catch (_) { /* ignore */ }
  }

  start() { this.active = true; this.keys.clear(); this.mouse = [0, 0]; this.lock(); }

  stop() {
    this.active = false;
    this.keys.clear();
    this.wasLocked = false;
    if (this.locked) document.exitPointerLock();
  }

  held(...codes) { return codes.some((c) => this.keys.has(c)); }

  /** Mouse movement since the last call. */
  takeMouse() { const m = this.mouse; this.mouse = [0, 0]; return m; }
}
