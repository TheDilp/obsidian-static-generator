(() => {
  const viewport = document.getElementById("canvas-viewport");
  const world = document.getElementById("canvas-world");
  if (!viewport || !world) return;

  const shell = document.getElementById("canvas-shell");
  const status = document.getElementById("canvas-zoom-status");
  const fullscreen = document.getElementById("canvas-fullscreen");
  const bounds = viewport.dataset.bounds.split(" ").map(Number);
  const transform = { x: 0, y: 0, scale: 1 };
  const pointers = new Map();
  let drag = null;
  let pinch = null;

  function draw() {
    world.style.transform = `translate(${transform.x}px, ${transform.y}px) scale(${transform.scale})`;
    status.textContent = `${Math.round(transform.scale * 100)}%`;
  }

  function fit() {
    if (!viewport.clientWidth || !viewport.clientHeight) return;
    transform.scale = Math.min(
      viewport.clientWidth / bounds[2],
      viewport.clientHeight / bounds[3],
      1,
    );
    transform.x = (viewport.clientWidth - bounds[2] * transform.scale) / 2 - bounds[0] * transform.scale;
    transform.y = (viewport.clientHeight - bounds[3] * transform.scale) / 2 - bounds[1] * transform.scale;
    draw();
  }

  function zoom(factor, x = viewport.clientWidth / 2, y = viewport.clientHeight / 2) {
    const next = Math.max(0.01, Math.min(4, transform.scale * factor));
    const ratio = next / transform.scale;
    transform.x = x - (x - transform.x) * ratio;
    transform.y = y - (y - transform.y) * ratio;
    transform.scale = next;
    draw();
  }

  function point(event) {
    const rect = viewport.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  function startPinch() {
    const [a, b] = [...pointers.values()];
    const x = (a.x + b.x) / 2;
    const y = (a.y + b.y) / 2;
    pinch = {
      distance: Math.max(1, Math.hypot(b.x - a.x, b.y - a.y)),
      scale: transform.scale,
      x: (x - transform.x) / transform.scale,
      y: (y - transform.y) / transform.scale,
    };
    drag = null;
  }

  viewport.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    pointers.set(event.pointerId, point(event));
    if (pointers.size === 2) {
      startPinch();
      for (const id of pointers.keys()) viewport.setPointerCapture(id);
      return;
    }
    if (event.target.closest("a, button, [data-canvas-content], [data-canvas-title]")) return;
    drag = { id: event.pointerId, x: event.clientX - transform.x, y: event.clientY - transform.y };
    viewport.setPointerCapture(event.pointerId);
    viewport.focus({ preventScroll: true });
    viewport.classList.add("cursor-grabbing");
    event.preventDefault();
  });

  viewport.addEventListener("pointermove", (event) => {
    if (!pointers.has(event.pointerId)) return;
    pointers.set(event.pointerId, point(event));
    if (pinch && pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      transform.scale = Math.max(0.01, Math.min(4, pinch.scale * Math.hypot(b.x - a.x, b.y - a.y) / pinch.distance));
      transform.x = (a.x + b.x) / 2 - pinch.x * transform.scale;
      transform.y = (a.y + b.y) / 2 - pinch.y * transform.scale;
      draw();
    } else if (drag?.id === event.pointerId) {
      transform.x = event.clientX - drag.x;
      transform.y = event.clientY - drag.y;
      draw();
    }
  });

  function release(event) {
    pointers.delete(event.pointerId);
    pinch = null;
    if (drag?.id === event.pointerId) drag = null;
    if (viewport.hasPointerCapture(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
    viewport.classList.remove("cursor-grabbing");
  }
  viewport.addEventListener("pointerup", release);
  viewport.addEventListener("pointercancel", release);
  viewport.addEventListener("lostpointercapture", release);
  viewport.addEventListener("pointerleave", (event) => {
    if (!viewport.hasPointerCapture(event.pointerId)) release(event);
  });

  viewport.addEventListener("wheel", (event) => {
    const card = event.target.closest("[data-canvas-content]");
    if (card && card.scrollHeight > card.clientHeight && !event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const position = point(event);
    zoom(Math.exp(-event.deltaY * 0.001), position.x, position.y);
  }, { passive: false });

  viewport.addEventListener("keydown", (event) => {
    if (event.target !== viewport || event.altKey || event.ctrlKey || event.metaKey) return;
    const distance = event.shiftKey ? 160 : 60;
    switch (event.key) {
      case "ArrowLeft": transform.x += distance; break;
      case "ArrowRight": transform.x -= distance; break;
      case "ArrowUp": transform.y += distance; break;
      case "ArrowDown": transform.y -= distance; break;
      case "+": case "=": zoom(1.25); break;
      case "-": zoom(0.8); break;
      case "Home": fit(); break;
      default: return;
    }
    event.preventDefault();
    draw();
  });

  viewport.addEventListener("focusin", (event) => {
    if (event.target === viewport) return;
    const view = viewport.getBoundingClientRect();
    const item = event.target.getBoundingClientRect();
    if (item.left < view.left || item.right > view.right) transform.x += (view.left + view.right - item.left - item.right) / 2;
    if (item.top < view.top || item.bottom > view.bottom) transform.y += (view.top + view.bottom - item.top - item.bottom) / 2;
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
    draw();
  });

  document.getElementById("canvas-zoom-in").addEventListener("click", () => zoom(1.25));
  document.getElementById("canvas-zoom-out").addEventListener("click", () => zoom(0.8));
  document.getElementById("canvas-fit").addEventListener("click", fit);
  if (shell.requestFullscreen) {
    fullscreen.addEventListener("click", async () => {
      try {
        if (document.fullscreenElement) await document.exitFullscreen();
        else await shell.requestFullscreen();
      } catch {
        status.textContent = "Fullscreen unavailable";
      }
    });
    document.addEventListener("fullscreenchange", () => {
      fullscreen.setAttribute("aria-pressed", String(document.fullscreenElement === shell));
      fit();
    });
  } else fullscreen.hidden = true;

  document.getElementById("canvas-controls").classList.remove("hidden");
  document.getElementById("canvas-controls").classList.add("flex");
  fit();
  new ResizeObserver(fit).observe(viewport);
})();
