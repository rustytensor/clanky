//    ██
// C fixture for the Q3 syntax gate.
//                        ██
#include <stdio.h>
#define LIMIT 8

struct point { int x; };

static int twice(int v) {
    if (v > LIMIT) return 0;
    return v * 2;
}

int main(void) { printf("%d\n", twice(3)); return 0; }
