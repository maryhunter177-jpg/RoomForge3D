import { describe, expect, it } from 'vitest';
import { initialRoom, measurements, validPolygon } from './room';
describe('room draft geometry',()=>{
 it('calculates rectangular dimensions in meters',()=>{expect(measurements(initialRoom.points)).toEqual({area:20,perimeter:18});});
 it('accepts a concave L shape',()=>{const points=[{x:0,z:0},{x:4,z:0},{x:4,z:2},{x:2,z:2},{x:2,z:4},{x:0,z:4}];expect(validPolygon(points)).toBe(true);expect(measurements(points).area).toBe(12);});
 it('rejects self intersecting polygons',()=>{expect(validPolygon([{x:0,z:0},{x:5,z:4},{x:0,z:4},{x:5,z:0}])).toBe(false);});
 it('rejects invalid and repeated coordinates',()=>{expect(validPolygon([{x:NaN,z:0},{x:2,z:0},{x:0,z:2}])).toBe(false);expect(validPolygon([{x:0,z:0},{x:2,z:0},{x:2,z:0},{x:0,z:2}])).toBe(false);});
});
