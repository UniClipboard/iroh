# UniClipboard Noq atomic close ownership patch

## Exact upstream source

This directory vendors the complete published crates.io package `noq` version `1.3.0`. The cached archive SHA256 is `b78be567e796cfa74bb9bdc4117790af2d505eb887019cf8803244353eb09d89`, matching the pre-integration Engine Cargo.lock registry checksum. Packaged VCS metadata identifies `https://github.com/n0-computer/noq`, commit `c1f411562e6078852749b8bcf1190523096a107f`, subdirectory `noq`.

The archive contains 33 regular files and 441269 uncompressed file bytes. Every archive member was compared byte-for-byte with the installed registry package before extraction. The registry's additional `.cargo-ok` marker is not part of the publication and is excluded. All packaged source, tests, examples, build script, release metadata, normalized Cargo.toml, Cargo.toml.orig, Cargo.lock, README, LICENSE-APACHE, LICENSE-MIT and .cargo_vcs_info.json are retained. The packaged lockfile is an unchanged provenance file; it does not replace or modify the Engine integration lockfile.

UPSTREAM_SOURCE_MANIFEST.json records every original member's SHA256 and byte size. UNICLIPBOARD_SOURCE_MANIFEST.json records the maintained source hashes and the only modified upstream file. Both manifests exclude themselves from their own file-hash set to avoid recursive digests. No registry cache or upstream published archive was edited.

## One complete native operation

`Connection::close_if_open(error_code, reason) -> bool` acquires the existing native connection mutex once. If the wrapper has a termination error or the underlying protocol is already closed, it returns false and changes nothing. Otherwise it calls the existing State::close under that same guard and returns true. State::close performs the existing protocol transition, LocallyClosed termination, registered on_closed completion, waiter notifications and driver wake. No second lock, future, task, callback, timer, new state field or error ledger is introduced.

Existing `Connection::close` remains byte-for-byte unchanged. It retains upstream repeat-close/error-overwrite semantics. A true atomic result identifies this caller's winning close transition at that critical section; it is not an immutable cause certificate for every later error. The consumer must independently inspect the actual supplied error source and retain cancellation/admission/settlement rules. A queued but unapplied external event is not a prior completed native transition. Noq owns the synchronization fact; the Infra bridge and Application own their subsequent policy.

The added source comments are Chinese and explicitly identify the UniClipboard modification. The only changed upstream Rust file is src/connection.rs. Iroh adds a thin forwarding method with the same name/signature and maps no additional business state.

## Dependency boundaries

Iroh's general and non-browser target noq dependencies use `../noq-1.3.0` with the original version/features. Iroh-relay uses the same package because Iroh passes its native Endpoint into relay QuicClient::new. These endpoints must have the same package source identity.

The optional iroh-blobs RPC API and registry irpc intentionally keep their registry Noq dependency. Their raw Noq Endpoint signatures stay on that RPC source together; they are not interchangeable with Iroh's native Endpoint. The ordinary iroh-blobs provider uses iroh::endpoint::Connection and stream types, so it follows Iroh's path dependency. The separate native benchmark keeps registry Noq; its Iroh configuration use is AckFrequencyConfig, re-exported from the unchanged shared noq-proto package. Root integration must inspect resolved package IDs and optional RPC/all-target feature configurations rather than hide the expected duplicate Noq sources.

Noq-proto and Noq-udp are not vendored or changed by this patch. They remain registry dependencies at the existing integration lock versions. No root crates.io patch, version upgrade or registry mutation is used.

## Diagnostics and validation

The versioned directory name preserves the existing safe network decoder's generic name-version path recognition as `noq-1.3.0`. Noq-proto source paths and reviewed line rules remain unchanged. The current rule catalog has no reviewed line entries for Noq src/connection.rs or Iroh src/endpoint/connection.rs; source insertion therefore does not silently invalidate a reviewed line mapping for those files. New/unreviewed native callsites remain subject to the existing safe fallback; no raw fields are exposed. No root decoder/checker is changed here.

The coordinator owns active Cargo.lock resolution, compilation, independent review, native/Engine E2E, gitlink commit and pin/checker synchronization. Static syntax, archive/member checks and diff review do not prove compilation or runtime close attribution. The preserved upstream tests were not changed or executed; no new unit tests were authored.

## Maintenance and exit conditions

Keep this package at 1.3.0 until a separately reviewed dependency update is authorized. Revalidate the complete archive and file manifests whenever the package changes. Preserve both native checks under the same existing mutex and preserve the original close/wake/notification transition. Do not replace it with separate close_reason sampling, a bridge-local lock or an overwriteable flag.

Remove this vendor only when an accepted upstream Noq release provides equivalent atomic first-transition ownership, Iroh can forward that operation without a local patch, all actual native Endpoint consumers resolve a compatible source, and runtime acceptance covers owned Offline close plus remote/local/session/endpoint/lag/cancellation negative controls and real file delivery under the original budgets. Then remove the path overrides and local package together through the coordinator's lockfile/pin review. A local build or development commit is not sufficient to remove the source provenance or acceptance requirements.
