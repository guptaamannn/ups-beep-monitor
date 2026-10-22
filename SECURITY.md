# Security policy

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Please **don't open a public issue** for a security problem. Report it privately with GitHub's "Report a vulnerability" button under the repository's **Security** tab, or email contact@guptaaman.com.np. Include what you found, how to reproduce it, and the version.

You'll get an acknowledgement within a few days. This is a volunteer project, so fixes take as long as they take, but security reports come first.

## What counts

`upsbeep` runs as your own user and can power the machine off, so these matter most:

- **Control socket.** The daemon listens on an owner-only (mode 0600) Unix socket in `$XDG_RUNTIME_DIR`. Anything that lets another user or a remote party send it commands (`cancel`, `pause`) is a vulnerability.
- **Shutdown.** Anything that makes the daemon power the machine off, or suppress a needed shutdown, without a real outage.
- **Config and log files.** Writing outside the intended paths, or reading attacker-controlled input unsafely.

The daemon opens no network connections. It captures audio only to look for one tone; audio is analysed in memory and never stored or sent anywhere. The only recording is the in-memory buffer used during an explicit "Record and analyze" in the window or `--analyze`.
