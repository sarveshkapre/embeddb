# SECURITY

## Reporting
Please report security issues privately via GitHub Security Advisories.

## Threat model notes
- Data at rest is local to the host; protect file permissions.
- WAL integrity is critical; corrupted WAL must fail closed.
- Remote embedder (if enabled) must be explicit opt-in.
