import type { NormalizedCommand } from './types';
import type { RelayerConfig } from './config';

/** Where normalized, chain-confirmed commands go. */
export interface Sink {
  inject(cmd: NormalizedCommand): Promise<void>;
}

/** M2 default: log the command (the bridge's output, ready for M3 to wire into the Sim). */
export class LogSink implements Sink {
  async inject(cmd: NormalizedCommand): Promise<void> {
    console.log(`[inject] ${cmd.kind} ${cmd.idempotencyKey} ${JSON.stringify(cmd)}`);
  }
}

/**
 * M3: POST the chain-confirmed command to the gateway/admin-api injection endpoint. Retries
 * are safe because each command carries an idempotency key (the Sim no-ops duplicates —
 * idempotency place #3). Dead-letter handling is M3.
 */
export class HttpSink implements Sink {
  constructor(
    private readonly url: string,
    private readonly token?: string,
    private readonly retries = 3,
  ) {}

  async inject(cmd: NormalizedCommand): Promise<void> {
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    if (this.token) headers.authorization = `Bearer ${this.token}`;
    let lastError: unknown;
    for (let attempt = 1; attempt <= this.retries; attempt++) {
      try {
        const res = await fetch(this.url, { method: 'POST', headers, body: JSON.stringify(cmd) });
        if (res.ok) return;
        lastError = new Error(`inject ${cmd.idempotencyKey} -> HTTP ${res.status}`);
      } catch (error) {
        lastError = error;
      }
      await new Promise((r) => setTimeout(r, attempt * 500));
    }
    throw lastError;
  }
}

export function makeSink(config: RelayerConfig): Sink {
  return config.injectUrl ? new HttpSink(config.injectUrl, config.operatorToken) : new LogSink();
}
