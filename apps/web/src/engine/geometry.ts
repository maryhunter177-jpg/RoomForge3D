export type Point = { x: number; z: number };
const EPS = 1e-8;
export function signedArea(points: Point[]): number {
  return points.reduce((sum, p, i) => { const q = points[(i + 1) % points.length]; return sum + p.x * q.z - q.x * p.z; }, 0) / 2;
}
export function roomMetrics(points: Point[]): { area: number; perimeter: number } {
  return { area: Math.abs(signedArea(points)), perimeter: points.reduce((sum, p, i) => { const q = points[(i + 1) % points.length]; return sum + Math.hypot(q.x - p.x, q.z - p.z); }, 0) };
}
export function rectanglePoints(width: number, depth: number): Point[] {
  return [{ x: 0, z: 0 }, { x: width, z: 0 }, { x: width, z: depth }, { x: 0, z: depth }];
}
function cross(a: Point, b: Point, c: Point): number { return (b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x); }
function onSegment(a: Point, b: Point, p: Point): boolean {
  return Math.abs(cross(a, b, p)) < EPS && p.x >= Math.min(a.x, b.x) - EPS && p.x <= Math.max(a.x, b.x) + EPS && p.z >= Math.min(a.z, b.z) - EPS && p.z <= Math.max(a.z, b.z) + EPS;
}
function intersects(a: Point, b: Point, c: Point, d: Point): boolean {
  const abC = cross(a, b, c), abD = cross(a, b, d), cdA = cross(c, d, a), cdB = cross(c, d, b);
  return (abC * abD < -EPS && cdA * cdB < -EPS) || onSegment(a, b, c) || onSegment(a, b, d) || onSegment(c, d, a) || onSegment(c, d, b);
}
export function validateRoom(points: Point[]): string | null {
  if (points.length < 3 || points.length > 64) return 'Use entre 3 e 64 pontos para o ambiente.';
  if (points.some(p => !Number.isFinite(p.x) || !Number.isFinite(p.z) || Math.abs(p.x) > 100 || Math.abs(p.z) > 100)) return 'As coordenadas devem ser finitas e estar entre -100 e 100 metros.';
  for (let i = 0; i < points.length; i++) {
    const a = points[i], b = points[(i + 1) % points.length];
    if (Math.hypot(b.x - a.x, b.z - a.z) < 0.05) return 'Cada parede precisa ter pelo menos 5 cm.';
    for (let j = i + 1; j < points.length; j++) {
      if (j === i + 1 || (i === 0 && j === points.length - 1)) continue;
      if (intersects(a, b, points[j], points[(j + 1) % points.length])) return 'As paredes nao podem se cruzar ou se sobrepor.';
    }
    const previous = points[(i + points.length - 1) % points.length];
    if (Math.abs(cross(previous, a, b)) < EPS && (previous.x - a.x) * (b.x - a.x) + (previous.z - a.z) * (b.z - a.z) > 0) return 'Paredes consecutivas nao podem se sobrepor.';
  }
  if (Math.abs(signedArea(points)) < 0.01) return 'O ambiente precisa ter uma area maior que 0,01 m².';
  return null;
}
export function wallOutwardNormal(points: Point[], index: number): Point {
  const a = points[index], b = points[(index + 1) % points.length];
  const length = Math.hypot(b.x - a.x, b.z - a.z), direction = signedArea(points) >= 0 ? 1 : -1;
  return { x: direction * (b.z - a.z) / length || 0, z: direction * (a.x - b.x) / length || 0 };
}

