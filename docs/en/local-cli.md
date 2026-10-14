# Local CLI

The native `eylema` command now owns the G15 entrypoint for the G14 loopback service. Its default session is an ordinary macOS local actor. G73 adds the explicit OS device-owner path described below; this is not remote identity.

## Start and operate

Run `eylema start` in a terminal to create or reopen the durable journal under `~/Library/Application Support/Eylema` and start the HTTP server on IPv4 loopback with an ephemeral port. The command stays in the foreground. `EYLEMA_DATA_DIR` selects an alternate local data directory for tests or an explicitly managed profile. The session file is mode `0600` inside a directory with no group/other permissions; it is not printed, placed in argv, logged or written into the journal. `eylema stop service` asks that foreground process to stop and join its worker. Abrupt process termination is not evidence of graceful shutdown.

```sh
eylema start
# In another terminal:
eylema doctor
eylema run demo 7
eylema inspect task <task-id>
eylema inspect execution <execution-id>
eylema stop service
```

`run demo [delta]` creates one bounded task that adds a value only to Eyleme's owned task state. The delta is 1–1,000,000. It has no filesystem, shell, network or other external effect. `create <request-file>` and `run <request-file>` accept bounded binary requests encoded by the existing versioned EYL transport codec. `inspect` prints stable JSON records and reuses the service's task/execution IDs and machine errors. Errors print `{"status":"error","code":"..."}` and return a nonzero exit status.

## Authorization and limits

`start` enrolls a bounded non-human CLI session with `create_task`, `read`, `execute_action` and `observe_events`, scoped to `state:cli`. This is enough for the built-in demonstration; submitted tasks outside that resource scope receive the service's normal denial. `stop <execution-id>` sends the same typed cancellation request as the web service. Cancellation succeeds only when the current session was independently verified by the trusted host as a human and has cancellation permission. The stock CLI session does not make that claim and receives `FORBIDDEN`; the CLI never promotes a caller flag or prompt into human verification. A test with an actual host-issued verified-human fixture confirms that an authorized stop request becomes effective.

`doctor` checks the existing private data directory, session and authenticated loopback service without creating or repairing data. `start` fails closed on an unsafe directory, symlinked journal, unsupported platform, lock conflict or invalid journal. It does not truncate, migrate or silently recover corrupt history. A newly created journal is private and locked while the service runs.

## Evidence boundary

G15's native macOS child-process test starts the real CLI service, creates and runs a typed task, checks shared IDs and machine errors, requests and verifies authorized cancellation with a separately host-issued human session, denies cancellation for the stock non-human session, gracefully stops/restarts the service and inspects the retained journal state. These tests do not prove a GUI, real-user identity provider, remote TLS, deployment, GPU/model/training behavior or physical power-loss recovery.

G73 adds explicit `start --human` and `human-proof <new-private-directory>`: [device-owner scope, lifetime and evidence](real-human.md). The existing plain CLI remains an ordinary local actor. Physical authentication is not inferred from native CI.
