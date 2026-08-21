# Governance & Upgradeability Security Considerations

## Overview

This module implements a multisig governance contract with timelock and upgradeability
for the SwapTrade protocol. All sensitive operations (pausing, upgrades, parameter changes)
require approval from a configurable set of signers with a configurable threshold.

## Threat Model

### Trusted Assumptions
- The initial set of signers is trusted at deployment time.
- The timelock delay provides a window for the community to detect malicious proposals.
- The multisig threshold is set high enough to prevent unilateral action.

### Key Risks

#### 1. Signer Compromise
If a signer's private key is compromised, an attacker can approve malicious proposals.

**Mitigations:**
- Maintain a minimum threshold (recommended: >= 3 of 5, or >= 5 of 9).
- Monitor on-chain events for unexpected proposal approvals.
- Use hardware security modules (HSMs) for signer keys where possible.
- Implement key rotation via governance proposals (`AddSigner` + `RemoveSigner`).

#### 2. Key Rotation
Compromised or departing signers must be replaced quickly.

**Procedure:**
1. A trusted proposer creates a governance proposal with `AddSigner(new_key)`.
2. The multisig approves the proposal (threshold signers sign).
3. After the timelock delay, the new signer is added.
4. A second proposal removes the old signer (`RemoveSigner(old_key)`).
5. Both proposals should be executed in the same timelock window to avoid gaps.

**Note:** Never remove a signer before adding a replacement, unless you are certain
the remaining signers still meet the threshold.

#### 3. Timelock Bypass
The timelock delay is the last line of defense against rapid malicious upgrades.

**Mitigations:**
- Set a minimum timelock delay of at least 48 hours (172,800 seconds) for mainnet.
- Do not set the timelock delay to 0 in production.
- Monitor `TimelockEvent` emissions for scheduled upgrades.

#### 4. Upgradeability Risks
Proxy upgrades replace the implementation contract entirely.

**Mitigations:**
- Always verify the new implementation hash via off-chain channels (GitHub, Discord).
- Use a separate verification step where the implementation is audited before upgrade.
- The `UpgradeEvent` event logs the old and new implementation addresses for audit.
- Consider using a transparent proxy pattern with a dedicated admin for added safety.

#### 5. Replay Attacks
Each proposal has a unique nonce and is tracked by ID.

**Mitigations:**
- The `nonce` field is monotonically increasing and unique per proposal.
- Executed proposals cannot be re-executed (`ProposalAlreadyExecuted` error).
- Canceled proposals cannot be executed.

#### 6. Centralization Risk
If the multisig set is small, the protocol remains centralized.

**Mitigations:**
- Use a diverse set of signers from different organizations.
- Plan for progressive decentralization: start with a small trusted multisig,
  then transition to token-based governance or a DAO.
- Publish the multisig policy and signer identities publicly.

## Operational Guidelines

### Emergency Pause
- The `Pause` action can be executed via multisig to halt all protocol operations.
- The `Unpause` action requires a separate multisig proposal.
- Pause/unpause events are emitted for off-chain monitoring.

### Signer Management
- Proposals to add/remove signers require the same threshold as other actions.
- The `is_signer` view function allows checking membership at any time.
- The `get_threshold` view function allows checking the current threshold.

### Timelock Configuration
- The timelock delay is set at initialization and can be updated via governance.
- A delay of 0 means no timelock, which is NOT recommended for production.
- The `get_timelock_delay` view function returns the current delay in seconds.

### Upgrade Process
1. Governance proposes an upgrade with `ProposalAction::Upgrade(new_impl)`.
2. Multisig signers approve the proposal.
3. After the timelock delay, the upgrade is executed.
4. The `UpgradeEvent` is emitted with old/new implementation addresses.
5. State is preserved across upgrades as long as the new implementation
   maintains the same storage layout.

## Incident Response

### Compromised Signer
1. Immediately propose removal of the compromised signer.
2. If the threshold is still met without the signer, execute the removal.
3. Add a replacement signer in a follow-up proposal.
4. Review all recent proposals for unauthorized actions.

### Malicious Upgrade Detected During Timelock
1. Raise the alarm publicly and in governance channels.
2. If possible, gather signers to cancel the proposal (only the proposer can cancel).
3. If the proposal is executed, coordinate an emergency downgrade via a new proposal.
4. Consider pausing the protocol while the new implementation is audited.

### Protocol Paused Unexpectedly
1. Check the `PauseEvent` to identify which proposal and signers triggered it.
2. Coordinate with signers to execute an `Unpause` proposal.
3. Investigate the root cause before resuming operations.
