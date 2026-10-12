import { roomMetrics, validateRoom } from '../engine/geometry';
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
export type Point = { x: number; z: number };
export type Room = { points: Point[]; wallHeight: number; wallThickness: number };
export type View = 'top' | 'front' | 'side' | 'iso' | 'free';
export const initialRoom: Room = { points: [{x:0,z:0},{x:5,z:0},{x:5,z:4},{x:0,z:4}], wallHeight:2.7, wallThickness:.15 };
export { roomMetrics as measurements } from '../engine/geometry';
export function validPolygon(points: Point[]) { return validateRoom(points) === null && roomMetrics(points).area >= .25; }
type State={room:Room;view:View;cutaway:boolean;showGrid:boolean;showDimensions:boolean;setRoom:(room:Room)=>void;setView:(view:View)=>void;toggle:(key:'cutaway'|'showGrid'|'showDimensions')=>void};
export const useRoom=create<State>()(persist((set)=>({room:initialRoom,view:'iso',cutaway:true,showGrid:true,showDimensions:true,setRoom:(room)=>set({room}),setView:(view)=>set({view}),toggle:(key)=>set(state=>({[key]:!state[key]}))}),{name:'roomforge-room-draft-v1',partialize:state=>({room:state.room})}));


