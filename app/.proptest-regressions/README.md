# Proptest Regression Seeds

This directory is intentionally versioned. Proptest uses
`FileFailurePersistence::SourceParallel(".proptest-regressions")`, so failing
property tests write minimized repro cases here beside the app crate instead of
inside `src/`.

Commit files in this tree only when they come from real historical failures and
are useful repro seeds.
