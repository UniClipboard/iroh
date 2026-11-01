# UniClipboard netwatch source

This directory contains netwatch 0.19.3 from crates.io. The upstream package checksum is `39da9cad5f23aa43401f09497d3b280e51b5feba115d9ffbf38ca35d6ff95e96`. Its packaged VCS identity is `n0-computer/net-tools` commit `0030e57fc70981895bb86212ff0f059cc41c8a4e`, subdirectory `netwatch`.

The source retains the complete packaged platform implementation and existing tests. Cargo cache markers, registry checksums and the package lockfile are excluded. LICENSE-MIT and LICENSE-APACHE come from the exact upstream commit.

UniClipboard changes complete State equality to include interface address vectors. Linux NEWADDR events now reach the existing debounced complete-state comparison instead of an earlier IP-only cache. Interface metadata equality and major-change classification remain unchanged. No watcher, timer, socket, public API or per-packet I/O is added. The two changed Rust files carry modification notices.

Iroh uses this path dependency so consumers receive the corrected source through the reviewed network-stack gitlink. No root Cargo patch table or registry mutation is required. Runtime acceptance is recorded in the Engine PR343 development evidence.

Cargo also retains the registry netwatch 0.19.3 for portmapper. That consumer uses UdpSocket and HomeRouter helpers, not the native network-change monitor. The patched path package is selected by Iroh itself; this change does not replace unrelated registry sources or introduce another monitor. Verify the resolved package IDs in Engine Cargo metadata.
