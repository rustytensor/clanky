//    ██
// Go fixture for the Q3 syntax gate.
//                        ██
package main

import "fmt"

type Point struct{ X int }

func double(p Point) int {
	if p.X > 9 {
		return 0
	}
	return p.X * 2
}

func main() { fmt.Println(double(Point{X: 3}), "ok", nil) }
