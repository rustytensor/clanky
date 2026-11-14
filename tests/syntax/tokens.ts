//    ██
// TypeScript fixture for the Q3 syntax gate.
//                        ██
interface Point { x: number; label: string }
const limit = 42;
export function scale(p: Point, k: number): Point {
  if (k > 10) { return { x: p.x * 2, label: "big" }; }
  return { x: p.x * k, label: p.label.trim() };
}
let done: boolean = true;
