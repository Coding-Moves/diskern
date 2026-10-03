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
