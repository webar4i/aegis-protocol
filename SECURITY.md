# Security policy

Aegis 0.1 is an unaudited research prototype, not a production VPN. Do not use
it to protect sensitive communications. No release currently has a supported
production security maintenance promise.

Do not post private keys, certificates identifying real users, traffic payloads,
packet captures, personal addresses or working exploit credentials in issues.
Use the repository's private security advisory reporting facility if enabled.
If it is not enabled, open a minimal issue asking for a private reporting channel
without disclosing vulnerability details. No independent audit has been performed.

Before any public service release: independent protocol/implementation review,
credential revocation, hardened provisioning, sustained parser/state-machine
fuzzing, dependency advisory checks, real platform leak tests, and operational
incident response are required. See docs/THREAT_MODEL.md and docs/VALIDATION.md.
