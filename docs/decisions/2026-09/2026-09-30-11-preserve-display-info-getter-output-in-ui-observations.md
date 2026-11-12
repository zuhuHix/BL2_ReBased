## 2026-09-30: Preserve display-info getter output in UI observations

AI-assisted trace instrumentation fix. Existing UI Trace SDK 0.2.0 records
GetDisplayInfo's input D but omits its post-call out parameter; its return value
alone is unset. Thus earlier zero-valued getter input records do not prove a
flat original transform. Version 0.2.1 adds return object identity and out.D for
that getter only, using the existing WrappedStruct serializer, with no extra
getter invocation/property writes or new extraction tool. Normal return format
remains compatible; exhausted budgets and callback errors retain isolation.
The SDK API is referenced from primary bl-sdk/pyunrealsdk documentation; no
external source copied. Native post-hook output timing remains UNVERIFIED
until a fresh game run, not established by synthetic mocks.

Three synthetic callback-contract checks pass (completed output/identity,
unrelated return, budget/error isolation). CTest 6/6 (8.53 s); all nine package
checks match, Python syntax/diff pass. Updated the already installed local
OpenWillow tracer after checking its identity, preserving trace_dir.txt and
backing up the prior own script under ignored local/ui/tool-backups. Installed
and repository script SHA256 match. No original game is running and no new
native trace was captured. No UE runtime change or new engine run. The
projection gap and full 1:1 goal remain unresolved; other work can continue.

