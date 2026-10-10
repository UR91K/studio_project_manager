# 0070. Back up the database before discarding it

- **Status:** Accepted
- **Recognized:** 2026-10-08, planning the first alpha of the rewrite for testers
- **Decided:** 2026-10-09
- **Recorded:** 2026-10-09
- **Confidence:** decided now
- **Evidence:** ADR-0011 (a database from an older schema is deleted and rebuilt);
  ADR-0068 (testers will have real libraries, and the alpha may bump the schema);
  `src/database/core.rs` (`open_at_current_schema`, `back_up`);
  `tests/database/plugin_scan.rs`

## Context

ADR-0011 chose to recreate the whole database when the schema version changes, because
Seula was pre-1.0 and the maintainer was its only user. It warned that tags, collections,
tasks and notes are lost, and that no rescan recovers them.

The first alpha of the rewrite goes to a few testers who will build libraries in it. A
schema bump between alphas would delete their work with nothing kept. ADR-0068 accepted
that risk only "if testers are told", and named a backup as the cheap fix.

## Decision

**Before the database is deleted, it is copied.** The copy is made on the open
connection with `VACUUM INTO`, so it includes writes still in the `-wal` file, and it is
one ordinary SQLite file that any SQLite tool opens. It sits beside the database, named
for the schema it was written by and the moment: `seula.backup-v2-20261009-152400.db`.

**If the copy fails, the database is not deleted and the open fails.** The error names
the backup path and the cause, and says the database was left as it was.

**Only a database with something in it is backed up.** A new file, an in-memory
database and one already at the current schema are opened as before. A database newer
than this build is still refused, as ADR-0011's code already did.

**The discard itself is unchanged.** ADR-0011 stands. The old database is still
deleted and a new one built, and a rescan still recovers projects.

**The message that the library was rebuilt is logged at `error`.** The default log level
is `error`, and at `warn` the message would not appear for anyone who had not changed
it. It is the only explanation a tester gets for an empty library. It now carries the
old and new schema versions and the backup's path.

## Rejected alternatives

- **Warn and discard anyway when the copy fails.** It would keep the app starting on a
  full or read-only disk, and bring back exactly the loss this record exists to prevent.
  A user who cannot start the app sees the error and the path; a user whose work is
  silently gone sees nothing.
- **Copy the file.** It can miss writes still in `-wal`, and then has to be told about
  `-shm`. `VACUUM INTO` is one statement and yields a consistent, compacted file.
- **Real migrations.** The right answer before a beta, and ADR-0011 left room for it. It
  is far more work than the alpha needs, and every schema change would need its own step.
- **Drop only the plugin tables and keep user data.** The option ADR-0011 recommended and
  the maintainer declined, with the trap that stored project hashes would also have to be
  cleared. A backup keeps the user's data without taking that on.
- **A restore command.** Not built. Restoring is copying the backup over `seula.db` while
  the daemon is stopped, and only works with a build at the same schema version.

## Consequences

A schema bump no longer destroys user-authored data. It moves it to a file the user can
open and read.

The costs:

- **Backups accumulate.** Each bump leaves one file the size of the old database, and
  nothing prunes them. Alphas bump rarely, so the cost is small. Pruning can come with
  migrations.
- **Media is not backed up.** Cover art and audio files live outside the database and are
  not touched by the discard, but the rebuilt database no longer refers to them. They stay
  on disk, unreferenced.
- **The database can now refuse to open for a new reason.** A tester with no free disk
  space or no write access to the folder cannot start the app until that is fixed.
- **No automatic restore.** Getting the data back into a running library is manual.
- Only the helper that makes the copy is tested for failure. The refusal to delete when
  it fails rests on the `?` between the copy and the delete.

## Notes

Reversing is cheap: remove the `back_up` call and the discard behaves as ADR-0011
describes. When migrations replace the discard, this record should be superseded, not
edited.
