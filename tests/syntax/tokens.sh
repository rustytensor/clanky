#     ██
# Bash fixture for the Q3 syntax gate.
#                         ██
set -eu
NAME="world"
greet() {
    local who=$1
    if [ -n "$who" ]; then echo "hello $who"; fi
}
for i in 1 2 3; do greet "$NAME"; done
exit 0
