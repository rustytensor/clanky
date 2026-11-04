# M6 masking core results

`v0.3.1-mask.2`: `scripts/gate-m6-masking.sh` passed, exit 0.
The planted `split` fault failed specifically at byte boundary 2, exit 1.
Fault and clean gates each ran once. No expected result was weakened.

Verified 23 generated credential fixtures: all two-chunk byte boundaries,
three chunks, byte-at-a-time input, surrounding UTF-8, literal/AWS adjacency,
short lookalikes, environment selection, longest literal matching, private-key
EOF handling, static errors and bounded streaming of 131,072-byte bodies.
The default fixture configuration holds at most 36 undecided bytes, as predicted
from its longest 37-byte recognition trigger. Generated credential values and
captured child diagnostics stayed in memory; leak scans passed. No raw runtime
output was written to fixture or log files. Internal matcher and hold buffers
use isolated volatile zeroing on drop and have no Debug implementation.

Decisions: extra patterns are named exact literals, not regexes. Unseparated
variable-length token runs are conservatively one mask; literal and fixed AWS
adjacency remain distinct. Slack accepts any ASCII letter after `xox`. Effective
configuration is capped at 128 literals of 65,536 bytes each, with explicit errors
instead of silent omission. No new dependencies.

This is a standalone crate: the server does not yet use it. It does not mask the
person's display. Unknown secret formats, OS swap, caller-owned copies and server
integration are outside this gate; callers must preserve stream order and retain
one masker per command/read stream. Existing terminal/editor gates were not rerun.
