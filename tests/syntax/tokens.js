//    ██
// JavaScript fixture for the Q3 syntax gate.
//                        ██
const names = ["a", "b"];
function count(list) {
  let total = 0;
  for (const item of list) { total += item.length; }
  return total > 5 ? null : total;
}
console.log(count(names));
