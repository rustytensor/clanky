#     ██
# Python fixture for the Q3 syntax gate.
#                         ██
import os

def greet(name: str, count: int = 3) -> str:
    """Say hello."""
    if count > 0 and name is not None:
        return "hi " + name * 2
    print(len(name))
    return os.path.join("a", name)

class Greeter:
    limit = 1.5
    done = True
