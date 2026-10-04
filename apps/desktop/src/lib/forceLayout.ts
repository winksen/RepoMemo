export type LayoutNode = { id: string; radius: number };
export type LayoutLink = { source: string; target: string; strength: number };
export type Point = { x: number; y: number };

/**
 * Places nodes so linked ones sit close and everything else spreads out
 * (a Fruchterman–Reingold force layout). It runs to completion in one call
 * and is deterministic: nodes start on a spiral in input order, so the same
 * workspace always draws the same picture. Fine for a few hundred nodes.
 */
export function layoutGraph(nodes: LayoutNode[], links: LayoutLink[], width: number, height: number): Map<string, Point> {
  const count = nodes.length;
  // Each step costs count²; large graphs settle well enough in fewer steps.
  const iterations = count > 150 ? 120 : 300;
  const positions = new Map<string, Point>();
  if (!count) return positions;
  const index = new Map(nodes.map((node, position) => [node.id, position]));
  const area = width * height;
  const ideal = Math.sqrt(area / count) * 0.75;
  const xs = new Float64Array(count);
  const ys = new Float64Array(count);
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < count; i += 1) {
    const radius = ideal * 0.5 * Math.sqrt(i + 0.5);
    xs[i] = Math.cos(i * golden) * radius;
    ys[i] = Math.sin(i * golden) * radius;
  }
  const edges = links
    .map((link) => ({ a: index.get(link.source), b: index.get(link.target), strength: link.strength }))
    .filter((edge): edge is { a: number; b: number; strength: number } => edge.a !== undefined && edge.b !== undefined);

  const dx = new Float64Array(count);
  const dy = new Float64Array(count);
  let temperature = ideal * 2;
  const cooling = temperature / (iterations + 1);
  for (let step = 0; step < iterations; step += 1) {
    dx.fill(0);
    dy.fill(0);
    for (let i = 0; i < count; i += 1) {
      for (let j = i + 1; j < count; j += 1) {
        let ox = xs[i] - xs[j];
        let oy = ys[i] - ys[j];
        let distance = Math.hypot(ox, oy);
        if (distance < 0.01) { ox = 0.01 * (i - j); oy = 0.01; distance = Math.hypot(ox, oy); }
        const push = (ideal * ideal) / distance;
        dx[i] += (ox / distance) * push; dy[i] += (oy / distance) * push;
        dx[j] -= (ox / distance) * push; dy[j] -= (oy / distance) * push;
      }
    }
    for (const { a, b, strength } of edges) {
      const ox = xs[a] - xs[b];
      const oy = ys[a] - ys[b];
      const distance = Math.max(Math.hypot(ox, oy), 0.01);
      const pull = ((distance * distance) / ideal) * strength;
      dx[a] -= (ox / distance) * pull; dy[a] -= (oy / distance) * pull;
      dx[b] += (ox / distance) * pull; dy[b] += (oy / distance) * pull;
    }
    for (let i = 0; i < count; i += 1) {
      // Gravity keeps unlinked nodes from drifting off the canvas.
      dx[i] -= xs[i] * 0.12;
      dy[i] -= ys[i] * 0.12;
      const length = Math.hypot(dx[i], dy[i]);
      if (length > 0) {
        const move = Math.min(length, temperature);
        xs[i] += (dx[i] / length) * move;
        ys[i] += (dy[i] / length) * move;
      }
    }
    temperature = Math.max(temperature - cooling, 0.5);
  }

  // Pull strays back toward the crowd: a lone unlinked file should sit near
  // the rest, not shrink the whole picture to make room for itself.
  if (count > 3) {
    const cx = xs.reduce((sum, value) => sum + value, 0) / count;
    const cy = ys.reduce((sum, value) => sum + value, 0) / count;
    const distances = Array.from(xs, (value, i) => Math.hypot(value - cx, ys[i] - cy)).sort((a, b) => a - b);
    const limit = Math.max(distances[Math.floor(count / 2)] * 2.2, ideal * 2);
    for (let i = 0; i < count; i += 1) {
      const distance = Math.hypot(xs[i] - cx, ys[i] - cy);
      if (distance > limit) {
        xs[i] = cx + ((xs[i] - cx) / distance) * limit;
        ys[i] = cy + ((ys[i] - cy) / distance) * limit;
      }
    }
  }

  // Fit the result into the canvas, keeping room for the largest node.
  const margin = Math.max(...nodes.map((node) => node.radius)) + 24;
  const minX = Math.min(...xs), maxX = Math.max(...xs);
  const minY = Math.min(...ys), maxY = Math.max(...ys);
  const scale = Math.min((width - margin * 2) / Math.max(maxX - minX, 1), (height - margin * 2) / Math.max(maxY - minY, 1));
  const offsetX = (width - (maxX - minX) * scale) / 2;
  const offsetY = (height - (maxY - minY) * scale) / 2;
  const px = nodes.map((_, i) => offsetX + (xs[i] - minX) * scale);
  const py = nodes.map((_, i) => offsetY + (ys[i] - minY) * scale);

  // Tight clusters attract their members onto each other; nudge dots apart
  // until none overlap, then keep them on the canvas.
  const gap = 3;
  for (let pass = 0; pass < 40; pass += 1) {
    let moved = false;
    for (let i = 0; i < count; i += 1) {
      for (let j = i + 1; j < count; j += 1) {
        const minimum = nodes[i].radius + nodes[j].radius + gap;
        let ox = px[j] - px[i];
        let oy = py[j] - py[i];
        let distance = Math.hypot(ox, oy);
        if (distance >= minimum) continue;
        if (distance < 0.01) { ox = 1; oy = 0; distance = 1; }
        const shift = (minimum - distance) / 2;
        px[i] -= (ox / distance) * shift; py[i] -= (oy / distance) * shift;
        px[j] += (ox / distance) * shift; py[j] += (oy / distance) * shift;
        moved = true;
      }
    }
    for (let i = 0; i < count; i += 1) {
      const edge = nodes[i].radius + 4;
      px[i] = Math.min(Math.max(px[i], edge), width - edge);
      py[i] = Math.min(Math.max(py[i], edge), height - edge);
    }
    if (!moved) break;
  }
  nodes.forEach((node, i) => positions.set(node.id, { x: px[i], y: py[i] }));
  return positions;
}
