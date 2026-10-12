import { describe, expect, it } from 'vitest';
import { rectanglePoints, roomMetrics, validateRoom, wallOutwardNormal } from './geometry';
describe('room geometry in meters', () => {
  it('computes rectangle area and perimeter', () => { expect(roomMetrics(rectanglePoints(4, 3))).toEqual({ area: 12, perimeter: 14 }); });
  it('computes concave L area without filling the missing corner', () => { const p = [{x:0,z:0},{x:4,z:0},{x:4,z:2},{x:2,z:2},{x:2,z:4},{x:0,z:4}]; expect(validateRoom(p)).toBeNull(); expect(roomMetrics(p)).toEqual({ area: 12, perimeter: 16 }); });
  it('rejects crossing walls', () => { expect(validateRoom([{x:0,z:0},{x:4,z:4},{x:0,z:4},{x:4,z:0}])).not.toBeNull(); });
  it('rejects overlapping adjacent walls and repeated vertices', () => { expect(validateRoom([{x:0,z:0},{x:4,z:0},{x:2,z:0},{x:2,z:2}])).not.toBeNull(); expect(validateRoom([{x:0,z:0},{x:0,z:0},{x:1,z:1}])).not.toBeNull(); });
  it('rejects non finite and zero area coordinates', () => { expect(validateRoom(rectanglePoints(Infinity,3))).not.toBeNull(); expect(validateRoom([{x:0,z:0},{x:1,z:0},{x:2,z:0}])).not.toBeNull(); });
  it('keeps outward normals correct under reversed winding', () => { const p=rectanglePoints(4,3); expect(wallOutwardNormal(p,0)).toEqual({x:0,z:-1}); const reversed=[...p].reverse(); expect(wallOutwardNormal(reversed,2)).toEqual({x:0,z:-1}); });
});
