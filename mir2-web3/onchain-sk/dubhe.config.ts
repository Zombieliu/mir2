import { DubheConfig } from '@0xobelisk/sui-common';

/**
 * SK0 throwaway spike — the smallest dapp that exercises a Dubhe 1.2.x **session key**.
 *
 * `counter` has no `global` flag, so it is a PER-USER component (lives in the caller's
 * `UserStorage`). The spike: the canonical owner activates a session for an ephemeral
 * "session wallet"; that session wallet then signs `bump` locally (no wallet popup) and
 * the framework must credit the OWNER's counter — proving on-behalf-of session writes.
 */
export const dubheConfig: DubheConfig = {
  name: 'sk_spike',
  description: 'SK0 session-key spike: a per-user counter to exercise session writes',
  resources: {
    counter: { fields: { value: 'u64' } },
  },
  errors: {},
};

export default dubheConfig;
