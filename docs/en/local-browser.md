# Local browser task console

The native macOS service exposes a same-origin console at `/console` on its IPv4 loopback origin. Open the exact origin printed by `eylema service start`, then append `/console`. The console creates a bounded task, submits a deterministic `addTaskValue` action, and reads canonical task and execution state from the native journal-backed host.

The page stores only task and execution identifiers and the selected language in browser local storage. Reloading restores those identifiers and queries the service again. The CLI retains its last loopback port across a service restart when that port is free, so the same browser origin can recover the durable task. If another process occupies the port, the service safely chooses a new one; reopen its printed origin, because browser storage is isolated by origin. If the service is temporarily unavailable, the identifiers remain visible; reconnect and choose **Refresh status** to read host state.

The browser receives the host-issued bearer only in an `HttpOnly`, `SameSite=Strict` cookie scoped to the console API path. It is not placed in the URL, page script, browser storage, logs, or journal. Mutating requests require the exact loopback `Host` and `Origin`, a same-origin fetch signal, and the page's CSRF token. Responses are bounded and uncached, and the page loads no third-party assets.

This CLI session is not a verified person. The stop button sends the normal typed cancellation request; the host denies it with `FORBIDDEN`, and the UI keeps the observed execution state. The console does not claim that work stopped. Verified-human enrollment remains a separate trusted host operation.

The browser interface does not add a runtime, model, GPU path, remote authentication, TLS, or deployment. Native socket/journal tests and an actual browser session are separate evidence; a browser run does not establish GPU/model or cross-platform behavior.
