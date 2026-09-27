# CarbonHV guest integration protocol v1

The guest agent uses a broker-created, per-VM vsock channel. The host binds the channel to the VM instance ID and runtime generation before accepting application-level messages. A guest-reported name is descriptive data and is never used as host authorization.

The first frame is a bounded handshake containing protocol major/minor, guest build ID, CVD major, and a capability set. Version 1 requires CVD major 1 and protocol major 1. Unknown required operations fail explicitly.

Control messages cover readiness milestones, graceful shutdown, geometry acknowledgement, heartbeat, and bounded diagnostics. Optional bridge families cover one-shot clipboard, staged file transfer, notifications, audio/control metadata, and sensor grants. Bulk payloads use dedicated bounded streams so file transfer cannot block shutdown or liveness.

Every message carries a maximum encoded size established by the host. The host rejects stale session generations, oversized frames, unknown mandatory operations, rate-limit violations, and actions for grants that have been revoked. Clipboard contents, file bytes, keystrokes, screenshots, and credentials are never written to ordinary diagnostic logs.

Protected-VM attestation is not part of protocol v1. A successful handshake proves only that the endpoint is attached to the broker-created local VM channel.

