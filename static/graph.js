function readThemeColor(name, fallback) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

const LABEL_ZOOM_THRESHOLD = 1.6;

const FORCE_DEFAULTS = {
  center: 0,
  repel: 100,
  linkStrength: 0.5,
  linkDistance: 64,
};

function initGraphPage() {
  const viewport = document.getElementById("graph-viewport");
  const canvas = document.getElementById("graph-canvas");
  const filterInput = document.getElementById("graph-filter-input");
  const resetButton = document.getElementById("graph-reset-view");
  const tooltip = document.getElementById("graph-tooltip");
  const emptyState = document.getElementById("graph-empty-state");
  const forcesToggle = document.getElementById("graph-forces-toggle");
  const forcesChevron = document.getElementById("graph-forces-chevron");
  const forcesBody = document.getElementById("graph-forces-body");
  const centerInput = document.getElementById("graph-force-center");
  const centerValue = document.getElementById("graph-force-center-value");
  const repelInput = document.getElementById("graph-force-repel");
  const repelValue = document.getElementById("graph-force-repel-value");
  const linkStrengthInput = document.getElementById("graph-force-link-strength");
  const linkStrengthValue = document.getElementById("graph-force-link-strength-value");
  const linkDistanceInput = document.getElementById("graph-force-link-distance");
  const linkDistanceValue = document.getElementById("graph-force-link-distance-value");
  if (!viewport || !canvas) return;

  const ctx = canvas.getContext("2d");
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const theme = {
    ink: readThemeColor("--color-ink", "#0c0a09"),
    gold: readThemeColor("--color-gold", "#fbbf24"),
    parchment: readThemeColor("--color-parchment", "#f5f5f4"),
    hairline: readThemeColor("--color-hairline", "#292524"),
    hairlineStrong: readThemeColor("--color-hairline-strong", "#44403c"),
    lexicon: readThemeColor("--color-lexicon", "#d6d3d1"),
    lexiconDim: readThemeColor("--color-lexicon-dim", "#a8a29e"),
    lexiconFaint: readThemeColor("--color-lexicon-faint", "#57534e"),
  };

  let width = 0;
  let height = 0;
  let dpr = window.devicePixelRatio || 1;
  const transform = { x: 0, y: 0, k: 1 };

  let nodes = [];
  let links = [];
  let adjacency = new Map();
  let simulation = null;

  let hoveredNode = null;
  let selectedNode = null;
  let lastTappedNodeId = null;
  let filterQuery = "";

  let drawScheduled = false;
  function scheduleDraw() {
    if (drawScheduled) return;
    drawScheduled = true;
    window.requestAnimationFrame(() => {
      drawScheduled = false;
      draw();
    });
  }

  function nodeRadius(node) {
    return 3.5 + Math.sqrt(node.degree || 0) * 2.2;
  }

  function neighborsOf(node) {
    return adjacency.get(node.id) || new Set();
  }

  function isDimmed(node) {
    if (filterQuery && !node.title.toLowerCase().includes(filterQuery)) return true;
    const focusNode = selectedNode || hoveredNode;
    if (focusNode && focusNode !== node && !neighborsOf(focusNode).has(node.id)) return true;
    return false;
  }

  function edgeIsHighlighted(link) {
    const focusNode = selectedNode || hoveredNode;
    if (!focusNode) return false;
    return link.source.id === focusNode.id || link.target.id === focusNode.id;
  }

  function draw() {
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.translate(transform.x, transform.y);
    ctx.scale(transform.k, transform.k);

    for (const link of links) {
      const highlighted = edgeIsHighlighted(link);
      ctx.beginPath();
      ctx.moveTo(link.source.x, link.source.y);
      ctx.lineTo(link.target.x, link.target.y);
      ctx.strokeStyle = highlighted ? theme.gold : theme.lexiconDim;
      ctx.globalAlpha = highlighted ? 0.9 : 0.75;
      ctx.lineWidth = (highlighted ? 1.4 : 1) / transform.k;
      ctx.stroke();
    }

    for (const node of nodes) {
      const dimmed = isDimmed(node);
      const focused = node === hoveredNode || node === selectedNode;
      const radius = nodeRadius(node);

      ctx.globalAlpha = dimmed ? 0.25 : 1;
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      ctx.fillStyle = focused ? theme.gold : dimmed ? theme.lexiconFaint : theme.parchment;
      if (focused) {
        ctx.shadowColor = theme.gold;
        ctx.shadowBlur = 12 / transform.k;
      } else {
        ctx.shadowBlur = 0;
      }
      ctx.fill();
      ctx.shadowBlur = 0;

      if (focused) {
        ctx.lineWidth = 1.5 / transform.k;
        ctx.strokeStyle = theme.gold;
        ctx.globalAlpha = 1;
        ctx.stroke();
      }
    }

    if (transform.k >= LABEL_ZOOM_THRESHOLD) {
      const fontSize = 11 / transform.k;
      ctx.font = `${fontSize}px ui-monospace, monospace`;
      ctx.textBaseline = "middle";
      for (const node of nodes) {
        const dimmed = isDimmed(node);
        const focused = node === hoveredNode || node === selectedNode;
        ctx.globalAlpha = dimmed ? 0.25 : 0.9;
        ctx.fillStyle = focused ? theme.gold : theme.lexicon;
        ctx.fillText(node.title, node.x + nodeRadius(node) + 4 / transform.k, node.y);
      }
    }

    ctx.restore();
    ctx.globalAlpha = 1;
  }

  function resizeCanvas() {
    const rect = viewport.getBoundingClientRect();
    width = rect.width;
    height = rect.height;
    dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    if (simulation) {
      simulation.force("center", d3.forceCenter(width / 2, height / 2));
      simulation.force("x", d3.forceX(width / 2).strength(simulation.force("x").strength()));
      simulation.force("y", d3.forceY(height / 2).strength(simulation.force("y").strength()));
    }
    scheduleDraw();
  }

  function fitView() {
    if (!nodes.length) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
      minX = Math.min(minX, node.x);
      minY = Math.min(minY, node.y);
      maxX = Math.max(maxX, node.x);
      maxY = Math.max(maxY, node.y);
    }
    const graphWidth = Math.max(maxX - minX, 1);
    const graphHeight = Math.max(maxY - minY, 1);
    const padding = 64;
    const scale = Math.min(
      (width - padding) / graphWidth,
      (height - padding) / graphHeight,
      2,
    );
    transform.k = Math.max(scale, 0.1);
    transform.x = width / 2 - ((minX + maxX) / 2) * transform.k;
    transform.y = height / 2 - ((minY + maxY) / 2) * transform.k;
    scheduleDraw();
  }

  function screenToWorld(clientX, clientY) {
    const rect = canvas.getBoundingClientRect();
    const screenX = clientX - rect.left;
    const screenY = clientY - rect.top;
    return {
      x: (screenX - transform.x) / transform.k,
      y: (screenY - transform.y) / transform.k,
      screenX,
      screenY,
    };
  }

  function findNodeAt(worldX, worldY) {
    let closest = null;
    let closestDist = Infinity;
    for (const node of nodes) {
      const dx = node.x - worldX;
      const dy = node.y - worldY;
      const dist = Math.sqrt(dx * dx + dy * dy);
      const hitRadius = nodeRadius(node) + 6 / transform.k;
      if (dist <= hitRadius && dist < closestDist) {
        closest = node;
        closestDist = dist;
      }
    }
    return closest;
  }

  function showTooltip(node, hint) {
    if (!tooltip) return;
    const rect = canvas.getBoundingClientRect();
    const screenX = node.x * transform.k + transform.x;
    const screenY = node.y * transform.k + transform.y;
    tooltip.textContent = hint ? `${node.title} — ${hint}` : node.title;
    tooltip.style.left = `${Math.min(screenX + 14, rect.width - 16)}px`;
    tooltip.style.top = `${Math.max(screenY - 12, 8)}px`;
    tooltip.classList.remove("hidden");
  }

  function hideTooltip() {
    if (!tooltip) return;
    tooltip.classList.add("hidden");
  }

  function navigateTo(node) {
    window.location.href = `/${node.path}`;
  }

  const pointers = new Map();
  let pinchStartDistance = null;
  let pinchStartScale = 1;
  let dragTarget = null;
  let dragMoved = false;
  let panPointerId = null;
  let panStart = null;

  function pointerDistance() {
    const points = Array.from(pointers.values());
    const dx = points[0].clientX - points[1].clientX;
    const dy = points[0].clientY - points[1].clientY;
    return Math.sqrt(dx * dx + dy * dy);
  }

  function onPointerDown(event) {
    canvas.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, event);

    if (pointers.size === 2) {
      pinchStartDistance = pointerDistance();
      pinchStartScale = transform.k;
      dragTarget = null;
      panPointerId = null;
      return;
    }

    const world = screenToWorld(event.clientX, event.clientY);
    const node = findNodeAt(world.x, world.y);
    dragMoved = false;

    if (node) {
      dragTarget = node;
      node.fx = node.x;
      node.fy = node.y;
      if (simulation) simulation.alphaTarget(0.3).restart();
    } else {
      panPointerId = event.pointerId;
      panStart = { x: event.clientX - transform.x, y: event.clientY - transform.y };
    }
  }

  function onPointerMove(event) {
    if (!pointers.has(event.pointerId) && event.pointerType !== "mouse") return;
    if (pointers.has(event.pointerId)) pointers.set(event.pointerId, event);

    if (pointers.size === 2 && pinchStartDistance) {
      const distance = pointerDistance();
      transform.k = Math.min(Math.max((pinchStartScale * distance) / pinchStartDistance, 0.15), 4);
      scheduleDraw();
      return;
    }

    if (dragTarget) {
      dragMoved = true;
      const world = screenToWorld(event.clientX, event.clientY);
      dragTarget.fx = world.x;
      dragTarget.fy = world.y;
      scheduleDraw();
      return;
    }

    if (panPointerId === event.pointerId && panStart) {
      transform.x = event.clientX - panStart.x;
      transform.y = event.clientY - panStart.y;
      scheduleDraw();
      return;
    }

    if (event.pointerType === "mouse") {
      const world = screenToWorld(event.clientX, event.clientY);
      const node = findNodeAt(world.x, world.y);
      if (node !== hoveredNode) {
        hoveredNode = node;
        canvas.style.cursor = node ? "pointer" : "grab";
        scheduleDraw();
      }
      if (node) {
        showTooltip(node);
      } else {
        hideTooltip();
      }
    }
  }

  function onPointerUp(event) {
    pointers.delete(event.pointerId);
    if (pointers.size < 2) pinchStartDistance = null;

    if (dragTarget) {
      const node = dragTarget;
      node.fx = null;
      node.fy = null;
      if (simulation) simulation.alphaTarget(0);
      dragTarget = null;

      if (!dragMoved) {
        handleNodeTap(node, event.pointerType);
      }
      scheduleDraw();
      return;
    }

    if (panPointerId === event.pointerId) {
      panPointerId = null;
      panStart = null;
    }
  }

  function handleNodeTap(node, pointerType) {
    if (pointerType === "touch" || pointerType === "pen") {
      if (lastTappedNodeId === node.id) {
        navigateTo(node);
        return;
      }
      lastTappedNodeId = node.id;
      selectedNode = node;
      showTooltip(node, "tap again to open");
      return;
    }
    navigateTo(node);
  }

  function onWheel(event) {
    event.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const screenX = event.clientX - rect.left;
    const screenY = event.clientY - rect.top;
    const worldX = (screenX - transform.x) / transform.k;
    const worldY = (screenY - transform.y) / transform.k;

    const zoomFactor = Math.exp(-event.deltaY * 0.001);
    const nextK = Math.min(Math.max(transform.k * zoomFactor, 0.15), 4);

    transform.x = screenX - worldX * nextK;
    transform.y = screenY - worldY * nextK;
    transform.k = nextK;
    scheduleDraw();
  }

  function onFilterInput(event) {
    filterQuery = String(event.currentTarget.value || "").trim().toLowerCase();
    scheduleDraw();
  }

  function buildAdjacency() {
    adjacency = new Map();
    for (const node of nodes) adjacency.set(node.id, new Set());
    for (const link of links) {
      adjacency.get(link.source.id ?? link.source).add(link.target.id ?? link.target);
      adjacency.get(link.target.id ?? link.target).add(link.source.id ?? link.source);
    }
  }

  function startSimulation() {
    simulation = d3
      .forceSimulation(nodes)
      .force(
        "link",
        d3
          .forceLink(links)
          .id((d) => d.id)
          .distance(FORCE_DEFAULTS.linkDistance)
          .strength(FORCE_DEFAULTS.linkStrength),
      )
      .force("charge", d3.forceManyBody().strength(-FORCE_DEFAULTS.repel).distanceMax(220))
      .force("collide", d3.forceCollide((d) => nodeRadius(d) + 3))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force("x", d3.forceX(width / 2).strength(FORCE_DEFAULTS.center))
      .force("y", d3.forceY(height / 2).strength(FORCE_DEFAULTS.center))
      .on("tick", () => {
        buildAdjacency();
        scheduleDraw();
      });

    if (reducedMotion) {
      simulation.stop();
      for (let i = 0; i < 300; i++) simulation.tick();
      buildAdjacency();
      fitView();
    } else {
      simulation.on("end", fitView);
    }
  }

  function attachEvents() {
    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerleave", () => {
      if (!dragTarget) {
        hoveredNode = null;
        hideTooltip();
        scheduleDraw();
      }
    });

    if (filterInput) filterInput.addEventListener("input", onFilterInput);
    if (resetButton) {
      resetButton.addEventListener("click", () => {
        selectedNode = null;
        lastTappedNodeId = null;
        fitView();
      });
    }

    bindForceControls();

    const resizeObserver = new ResizeObserver(resizeCanvas);
    resizeObserver.observe(viewport);
  }

  function reheatSimulation() {
    if (simulation) simulation.alpha(0.6).restart();
  }

  function bindForceControls() {
    if (forcesToggle && forcesBody) {
      forcesToggle.addEventListener("click", () => {
        const expanded = forcesToggle.getAttribute("aria-expanded") === "true";
        forcesToggle.setAttribute("aria-expanded", String(!expanded));
        forcesBody.classList.toggle("hidden", expanded);
        if (forcesChevron) forcesChevron.style.transform = expanded ? "rotate(-90deg)" : "rotate(0deg)";
      });
    }

    if (centerInput) {
      centerInput.addEventListener("input", (event) => {
        const value = parseFloat(event.currentTarget.value);
        if (centerValue) centerValue.textContent = value.toFixed(2);
        if (simulation) {
          simulation.force("x").strength(value);
          simulation.force("y").strength(value);
          reheatSimulation();
        }
      });
    }

    if (repelInput) {
      repelInput.addEventListener("input", (event) => {
        const value = parseFloat(event.currentTarget.value);
        if (repelValue) repelValue.textContent = value.toFixed(0);
        if (simulation) {
          simulation.force("charge").strength(-value);
          reheatSimulation();
        }
      });
    }

    if (linkStrengthInput) {
      linkStrengthInput.addEventListener("input", (event) => {
        const value = parseFloat(event.currentTarget.value);
        if (linkStrengthValue) linkStrengthValue.textContent = value.toFixed(2);
        if (simulation) {
          simulation.force("link").strength(value);
          reheatSimulation();
        }
      });
    }

    if (linkDistanceInput) {
      linkDistanceInput.addEventListener("input", (event) => {
        const value = parseFloat(event.currentTarget.value);
        if (linkDistanceValue) linkDistanceValue.textContent = value.toFixed(0);
        if (simulation) {
          simulation.force("link").distance(value);
          reheatSimulation();
        }
      });
    }
  }

  async function loadGraph() {
    const url = window.__GRAPH_DATA_URL__;
    if (!url) return;

    let data;
    try {
      const response = await fetch(url);
      data = await response.json();
    } catch (err) {
      return;
    }

    nodes = data.nodes || [];
    links = data.links || [];

    if (!nodes.length || !links.length) {
      if (emptyState) {
        emptyState.classList.remove("hidden");
        emptyState.classList.add("flex");
      }
      return;
    }

    resizeCanvas();
    attachEvents();
    startSimulation();
  }

  loadGraph();
}

initGraphPage();
