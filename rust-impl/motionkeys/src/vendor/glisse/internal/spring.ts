export interface Spring {
  paint(): void;
  set(target: number, instant?: boolean): void;
  tune(stiffness: number, damping: number): void;
  stop(): void;
}

interface State {
  value: number;
  target: number;
  velocity: number;
  stiffness: number;
  damping: number;
  paint: (value: number) => void;
}

const moving = new Set<State>();
let frame = 0;
let last = 0;

function advance(s: State, dt: number): void {
  const x = s.value - s.target;
  const v = s.velocity;
  const half = s.damping / 2;
  const discriminant = half * half - s.stiffness;

  if (Math.abs(discriminant) < 1e-4) {
    const decay = Math.exp(-half * dt);
    const b = v + half * x;
    s.value = s.target + decay * (x + b * dt);
    s.velocity = decay * (v - half * b * dt);
  } else if (discriminant > 0) {
    const root = Math.sqrt(discriminant);
    const r1 = -half + root;
    const r2 = -half - root;
    const a = (v - r2 * x) / (r1 - r2);
    const b = x - a;
    s.value = s.target + a * Math.exp(r1 * dt) + b * Math.exp(r2 * dt);
    s.velocity = a * r1 * Math.exp(r1 * dt) + b * r2 * Math.exp(r2 * dt);
  } else {
    const omega = Math.sqrt(-discriminant);
    const decay = Math.exp(-half * dt);
    const cos = Math.cos(omega * dt);
    const sin = Math.sin(omega * dt);
    s.value = s.target + decay * (x * cos + ((v + half * x) / omega) * sin);
    s.velocity = decay * (v * cos - ((half * v + s.stiffness * x) / omega) * sin);
  }
}

function tick(time: number): void {
  frame = 0;
  const dt = Math.min(0.1, Math.max(0, (time - last) / 1000));
  last = time;
  for (const s of moving) {
    advance(s, dt);
    if (Math.abs(s.value - s.target) < 1e-4 && Math.abs(s.velocity) < 1e-3) {
      s.value = s.target;
      s.velocity = 0;
      moving.delete(s);
    }
    s.paint(s.value);
  }
  if (moving.size) frame = requestAnimationFrame(tick);
}

export function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function createSpring(
  value: number,
  stiffness: number,
  damping: number,
  paint: (value: number) => void,
): Spring {
  const s: State = { value, target: value, velocity: 0, stiffness, damping, paint };
  return {
    paint: () => paint(s.value),
    set(target, instant = false) {
      s.target = target;
      if (instant || document.hidden || prefersReducedMotion()) {
        s.value = target;
        s.velocity = 0;
        moving.delete(s);
        paint(target);
        return;
      }
      moving.add(s);
      if (!frame) {
        // Measure from the start of the current frame, so the next one
        // already shows a frame's worth of motion.
        last = Number(document.timeline?.currentTime ?? performance.now());
        frame = requestAnimationFrame(tick);
      }
    },
    tune(nextStiffness, nextDamping) {
      s.stiffness = nextStiffness;
      s.damping = nextDamping;
    },
    stop: () => {
      moving.delete(s);
    },
  };
}
