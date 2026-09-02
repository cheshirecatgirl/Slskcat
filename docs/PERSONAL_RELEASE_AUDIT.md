# Personal release audit

This is the final gate before using slskcat as a daily Soulseek replacement.

## Network validation

- [ ] Login against the real Soulseek network
- [ ] Search returns real peers and files
- [ ] Browse a peer successfully
- [ ] Fetch peer details
- [ ] Complete a real download

## Long-running reliability

- [ ] Leave connected overnight
- [ ] Suspend and resume the machine
- [ ] Disconnect network and recover
- [ ] Restart during an active transfer
- [ ] Confirm no memory growth over extended use

## Transfer behaviour

- [ ] Verify queued downloads behave normally
- [ ] Verify failed transfers recover cleanly
- [ ] Verify partial files do not become falsely completed
- [ ] Verify cancellation leaves consistent state

## Before public release

- [ ] Verify all supported platform builds
- [ ] Collect crash reports/logs from testers
- [ ] Test fresh installs
- [ ] Test upgrades from previous versions

The architecture and security checks are already covered elsewhere. This document focuses on the remaining validation that requires real network and machine usage.
