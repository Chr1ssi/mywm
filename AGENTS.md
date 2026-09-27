# Repository workflow

- After changes have been fully tested, commit and push them without asking for
  an additional confirmation so the user can rebuild and test them immediately.
- When `mywm-shell` changes, update and test `flake.lock` before committing and
  pushing the dependent `mywm` revision.
- Do not push changes that have not passed the relevant checks; report blockers
  instead.
