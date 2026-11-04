# Release-candidate gate

`scripts/gate-rc.sh` builds the gate executables once, then runs the listed gate
scripts in the owner's order. It uses the shared build lock, Cargo cache and
software Vulkan driver. Child gates run without a planted-fault variable. Each
listed script is invoked at most once; in this aggregate run, M9 omits its
nested M7 check and session polish omits its nested M9 check because the RC gate
already invokes those checks directly. Standalone gates retain their regressions.
Future aggregate-aware gates must honor the same `CLANKY_RC_NO_NESTED=1` setting.

In a normal run, absent scripts are reported as SKIP. The M5 gate is preferred;
if absent, report it missing and run M4 instead. `--require-all` marks every
missing listed gate as FAIL, including M5 even when M4 can run as a fallback:
M5 is required for a 1.0 release candidate. A child failure or build failure
makes the final verdict FAIL; other available gates still report their result.

For the planted fault, `CLANKY_GATE_FAULT=rc` makes the docs gate report FAIL
whether its script is present or absent. The final verdict must be FAIL, while
other available gates execute normally. No expected child result changes.
