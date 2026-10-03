# Quarantine crash recovery proposal

Status: proposed design for [#196](https://github.com/Coding-Moves/diskern/issues/196).
This document changes no runtime behavior. The current crash window remains until
an implementation passes the recovery tests and platform gates below.

## Failure being addressed

In [actions.rs](../crates/diskern-core/src/actions.rs), `quarantine()` encodes a
record, calls `move_file()`, then appends the record to `manifest.jsonl`.
A returned append error triggers a best-effort move back. Process termination
between those steps bypasses rollback: the file exists in quarantine but `list()`
cannot find its original path. Flattened filenames cannot reconstruct that path.
The cross-filesystem fallback copies and removes before recording either step.

The existing manifest mutex protects cooperating threads, not other processes,
and is acquired after destination selection and movement. Fixing that collision
race is the separate prerequisite [#195](https://github.com/Coding-Moves/diskern/issues/195).
A lock alone does not fix a process dying while it holds the lock.

## Decision and boundaries

Use a versioned, per-operation record published **before** touching the source.
Each record owns a unique payload directory. Record updates use complete temporary
files followed by atomic replacement; recovery reconciles the last valid record
with observed files. Do not infer ownership from a filename or file size alone.

Keep legacy `manifest.jsonl` readable. New records are authoritative in a separate
v2 store; do not dual-write them into the legacy manifest. This avoids two sources
of truth and prevents an old binary from treating a pending v2 payload as purgable.
No automatic migration or deletion of legacy records is part of the first rollout.

Scope: ordinary regular files selected through the existing authoritative report
and fresh rule checks. Reject symlinks and unsupported special files in the new
transaction path before recording intent. Preserve existing platform path-length
limits. This is not a filesystem snapshot or protection against a malicious process
able to edit Diskern's private storage or race arbitrary writes to the source.

## Required invariants

1. Before any source move/removal, a complete intent record identifies its original
   path, reserved destination, operation ID, source identity, and content digest.
2. Never overwrite an existing source, payload, staging file, or unrelated record.
3. Never remove the source of a cross-filesystem transfer before a verified complete
   destination and its recovery record have been durably published.
4. Recovery does not repeat quarantine, restore, or purge just because it finds a
   pending record. Pending operations cannot enter the ordinary purge selection.
5. A recovered completed record is listed once. Repeated recovery is idempotent.
6. Missing, corrupt, changed, or inaccessible evidence produces an explicit recovery
   item. Preserve all available copies; never guess a path or silently drop a record.
7. New destructive actions remain authorized by the engine. Recovery is not a way
   for the frontend to supply a verdict or bypass scan-report authorization.

## Storage and locking

Proposed layout within the current quarantine directory:

```text
manifest.jsonl                legacy records, unchanged
v2/
  format.json                 supported format and writer version
  writer.lock                 OS advisory lock for cooperating v2 processes
  transactions/
    <random-operation-id>/    reserved with exclusive directory creation
      record.json             latest complete, versioned record
      record.<nonce>.tmp       incomplete update; never authoritative
      payload.part            exclusively created copy staging file
      payload                 published complete payload
```

Hold the existing in-process mutex and an OS-backed exclusive writer lock, in that
order, across recovery and all v2 mutations. Lock acquisition must be bounded and
return a useful busy error; never delete a lock file to break a lock. A process
exit releases the OS lock. A lock file's existence is not itself a lock.
Readers use a shared lock or the same exclusive lock initially. Validate directory
ownership and reject symlinked transaction/control paths. On filesystems where the
required lock and publication primitives cannot be established, refuse v2 writes.

Operation IDs prevent accidental collisions but do not replace exclusive creation.
Temporary and payload names are confined to the reserved operation directory.
Implement destination publication with a tested no-replace operation, not
`exists()` followed by an overwriting rename. Directory creation and record
publication must finish before the source can be touched.

Illustrative record (schema proposal, not a new public API):

```json
{
  "version": 2,
  "id": "operation-id",
  "generation": 1,
  "state": "prepared",
  "original": "/home/user/cache/item",
  "payload": "payload",
  "at_epoch": 1700000000,
  "transfer": "rename",
  "source": {
    "size": 1024,
    "modified_ns": 1700000000000000000,
    "identity": { "platform": "unix", "device": 1, "inode": 42 },
    "blake3": "full-content-digest"
  }
}
```

Use real platform identity types (including volume/file IDs on Windows) and retain
full available timestamp precision. Size/time alone is not an identity proof.
Read and hash from an opened source handle; check identity and metadata before and
after reading and again before mutation. Reject changes. Hashing adds I/O at action
time, so display a preparing state and permit cancellation before source mutation.
Concurrent external writes are not fully prevented by these checks; document that
limit rather than claiming snapshot semantics. Restrict record permissions because
original paths can expose private information. Reject unsupported versions and
malformed fields; preserve their files for manual resolution.

## Transaction states and move protocol

```text
Prepared -> PayloadReady -> Committed
    |            |
    +------------+------> NeedsAttention (derived recovery result)
    |
    +-------------------> Aborted (only after proving source is unchanged)
```

`Prepared` is durable intent; `PayloadReady` proves that verified destination bytes
were published; `Committed` means the original name was removed and its directory
change completed. A lagging state after a crash is expected: inspect evidence,
not only the enum. Do not erase terminal records as part of startup recovery.

### Same filesystem

1. Acquire locks, validate report authority and source, reserve the operation
   directory, and durably publish `Prepared` with `transfer: rename`.
2. Revalidate authority/source and move to `payload` with no-replace semantics.
   Only a confirmed cross-device error may select the copy protocol; permission
   or collision errors must not silently fall back to copying.
3. Synchronize payload data and affected directories before publishing
   `PayloadReady`, then `Committed`. The source has already moved in step 2,
   so recovery must also handle a `Prepared` record with payload-only evidence.
4. Report success only after the committed record is published. If later record
   publication fails, return a recovery-required result with the operation ID;
   do not blindly move back over a potentially recreated source path.

If the move reports a cross-device error, atomically update the prepared record to
`transfer: copy` before any copy starts. If this update fails, leave source intact.

### Different filesystems

1. Publish `Prepared` with `transfer: copy`, leaving source untouched.
2. Copy from the validated source handle into exclusive `payload.part`. Preserve
   required permissions/metadata; verify length and BLAKE3 against the prepared
   digest and recheck the source. A partial or changed copy is never complete.
3. Sync the staged file, publish it as `payload` without replacement, sync its
   directory, and durably publish `PayloadReady`.
4. Revalidate source identity/content and authority before removing its original
   name. If validation fails, retain both and require attention. Sync the source
   parent after removal, then durably publish `Committed`.

Cancellation before source removal keeps the source and records an aborted or
attention state. Cancellation after removal finishes metadata publication or
returns recovery-required; it cannot claim that nothing was moved.

## Durability contract

Process termination and power loss are different guarantees. Child-process kill
checks verify recovery after userspace stops while the OS continues. They do not
verify disk ordering under sudden power loss.

A durable publication means: write all bytes to a fresh temp file, flush any
userspace buffers, check `File::sync_all`, atomically replace `record.json`, then
sync its parent directory. The initial directory tree and format marker also need
parent synchronization. Source removal/rename requires syncing the source parent;
destination publication requires syncing its parent. Same-filesystem source bytes
must be synchronized before moving if the implementation promises power-loss
preservation of the prepared content.

Rust documents the explicit [file synchronization API](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all).
On Linux, [fsync documentation](https://man7.org/linux/man-pages/man2/fsync.2.html)
explains why syncing a file is insufficient to persist its directory entry.
The implementation needs separately reviewed Windows and macOS adapters for
replacement, file synchronization, and directory durability; do not assume Unix
calls provide the same contract everywhere.

Power-loss guarantees are conditional on validated platform/filesystem primitives
and storage honoring flushes. A sync error stops the transaction at the safest
remaining state and preserves evidence. Filesystems lacking these primitives
must refuse the power-loss-safe write path, not quietly weaken its promise.
The first release may document only process-crash recovery on platforms whose
power-loss behavior remains unvalidated; enabling that mode must be explicit and
must never bypass a reported I/O/synchronization error.

## Startup reconciliation

Run reconciliation under both locks before allowing any v2 mutations, not inside
the read-only scan pipeline. Inspect paths without following symlinks. An
inaccessible path is not a missing path. Compare source identity and payload
digest to the record; identical bytes alone do not authorize deleting a source
that may have been independently recreated. Recovery may finalize metadata, but
must not automatically delete or overwrite either source or payload.

| Durable record / observed files | Recovery decision |
| --- | --- |
| `Prepared`; unchanged source only, no staging/payload | Mark `Aborted`; retain the record; leave source untouched. |
| `Prepared`; source plus partial staging | Attention: keep both. Offer an explicit discard of the owned partial copy only after verifying source; do not treat it as a complete restore candidate. |
| `Prepared` or `PayloadReady`; no source, verified payload | Publish `Committed` idempotently and expose one restoreable item. Revalidate relevant directory durability before finalizing. |
| `Prepared` or `PayloadReady`; verified source and verified payload | Attention: preserve both. Offer keeping the source and discarding the owned extra copy, or resuming quarantine after fresh authorization. Neither option is automatic. |
| Any nonterminal record; source replaced or payload mismatches | Attention with both paths and reason; no automatic mutation of either file. |
| Any active record; both source and payload absent | Attention: retain original-path evidence and report missing data; never claim successful recovery. |
| `Committed`; verified payload, no source | List normally. Repeated recovery changes nothing. |
| `Committed`; source also present | Keep payload, warn of restore conflict; never overwrite the source. |
| `Committed`; payload missing or changed | Attention; do not silently remove record or offer ordinary purge. |
| Invalid/unknown record version, symlink, unreadable directory, or unexplained files | Attention; preserve everything. Unknown files must never enter a cleanup batch. |

A complete `record.json` wins over leftover temporary records; a temp file alone
is insufficient authority to recover a move. If initial intent publication did
not finish, source mutation was forbidden. Malformed records require manual
resolution, not optimistic parsing followed by destructive actions. A stale
`generation` cannot override a newer complete record.

### Failure and rollback behavior

Before any source mutation, an ordinary error can leave the source in place and
mark the operation aborted. After source mutation, prefer forward recovery from
durable evidence to an unjournaled reverse move. An automatic best-effort rollback
must not overwrite a new source and is not part of this protocol.

Recovery results should be typed separately from ordinary quarantine records:

```rust
// Illustrative API shape; names are not committed interfaces.
enum RecoveryOutcome {
    Completed { operation_id: String },
    Aborted { operation_id: String },
    NeedsAttention { operation_id: String, reason: String },
}
```

The desktop should show a persistent recovery notice with original/destination
paths and a plain explanation. Escape paths as text, allow copying the diagnostic,
and keep unresolved entries out of ordinary restore/purge controls. A failure in
one operation should not hide healthy committed records; it must block mutations
that could conflict with unresolved evidence. Unsupported store format or lock
failure blocks all v2 mutations. No network request is required for recovery.

## Compatibility, restore, and purge

Read legacy `manifest.jsonl` with a dedicated adapter and preserve its bytes until
an explicit supported legacy action is requested. Display legacy records alongside
v2 committed records with an internal source-format tag. Do not infer that a legacy
orphan belongs to a path just because its name looks flattened. Surface unreadable
legacy lines rather than promoting them to new records.

Legacy restore/purge retains its existing limitations during an initial rollout;
clearly document that recovery guarantees apply only to new v2 transactions. Do
not claim legacy actions have become crash-safe. The initial implementation must
not run legacy and v2 writers concurrently against shared destinations. Independent
old binaries do not honor the new lock, so running an old writer alongside a new
one is unsupported and must be explained in upgrade instructions.

Do not enable new v2 quarantine writes until v2 restore and purge understand the
record lifecycle. A pending record can never be purged. A restore needs its own
durable intent and non-overwriting publication at the original path. A confirmed
purge needs a durable intent identifying the exact committed operation before
unlinking its payload. Finish with `Restored`/`Purged` terminal records so startup
cannot reinterpret a deliberately absent payload as a new quarantine operation.
If a crash interrupts restore/purge, surface the pending operation for reconciliation;
never resume a permanent delete automatically. Keep terminal records in the first
version; bounded retention/compaction is later work with separate crash tests.

Downgrading is not a format conversion. Old binaries see only their legacy
manifest and will not list v2 payloads. Before downgrade, the new version must
resolve pending operations and restore/export all v2 data through an explicitly
authorized workflow; otherwise keep the newer binary available for recovery.
Never delete v2 storage merely to make an old version appear compatible.

## Alternatives considered

- **Only append before moving:** an ordinary legacy record would appear purgeable
  while the move is incomplete. Intent needs an explicit non-actionable state.
- **Keep a process mutex longer:** fixes some collisions, but locks vanish on exit
  and cannot reconstruct a missing path record.
- **Always roll back on startup:** unsafe if another program recreated the source
  or if copying was incomplete; evidence must be reconciled first.
- **Dual-write journal and legacy manifest:** requires ordering, duplicate handling,
  and tombstones across two authorities. Prefer a separate versioned authority.
- **Use a database:** may simplify metadata transactions, but cannot atomically
  commit filesystem rename/unlink operations with the database. It still needs
  an intent/reconciliation protocol; evaluate only if scale justifies it.
