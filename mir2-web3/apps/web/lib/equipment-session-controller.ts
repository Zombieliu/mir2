import type {
  ClientCoreRuntime,
  EquipmentOperation,
  EquipmentPendingRuntime,
  EquipmentSnapshot,
} from "./client-core-runtime";

export type EquipmentSessionIdentity = {
  connectionGeneration: number;
  sessionGeneration: number;
};

export type EquipmentTicket = EquipmentSessionIdentity & { id: number };

export type EquipmentSessionResult = {
  ok: boolean;
  error?: string;
  pending: number;
  ticket?: EquipmentTicket;
  matched?: boolean;
  released?: boolean | number;
  reserved?: boolean;
};

export type EquipmentSessionStatus = EquipmentSessionIdentity & {
  ready: boolean;
  suspended: boolean;
  suspendReason: string | null;
  ownerRevision: number;
  pending: number;
  barriers: number;
  error: string | null;
};

type TicketRecord = { operation: EquipmentOperation; state: "reserved" | "sent" | "unknown" | "acknowledged" };

const validGeneration = (value: number): boolean => Number.isSafeInteger(value) && value >= 0;
const sameOperation = (left: EquipmentOperation, right: EquipmentOperation): boolean =>
  left.kind === right.kind && left.grid.toLowerCase() === right.grid.toLowerCase()
  && left.uniqueId === right.uniqueId && left.to === right.to;

/** One authoritative equipment ledger per confirmed player session, independent of Bevy ownership. */
export class EquipmentSessionController {
  private bridge: EquipmentPendingRuntime | null = null;
  private connectionGeneration = 0;
  private sessionGeneration = 0;
  private highestSessionGeneration = 0;
  private highestConnectionGeneration = 0;
  private ownerRevision = 0;
  private nextTicketId = 1;
  private tickets = new Map<number, TicketRecord>();
  private ready = false;
  private suspended = false;
  private suspendReason: string | null = null;
  private error: string | null = null;

  constructor(private readonly runtime: ClientCoreRuntime) {}

  private pending(): number {
    try { return this.bridge?.status().pending ?? 0; } catch { return this.tickets.size; }
  }

  private fail(error: string): EquipmentSessionResult {
    return { ok: false, error, pending: this.pending() };
  }

  private matches(identity: EquipmentSessionIdentity): boolean {
    return validGeneration(identity.connectionGeneration) && validGeneration(identity.sessionGeneration)
      && identity.connectionGeneration === this.connectionGeneration
      && identity.sessionGeneration === this.sessionGeneration && this.bridge !== null;
  }

  private call(action: () => EquipmentSessionResult): EquipmentSessionResult {
    try { return action(); } catch {
      // A bridge failure cannot prove that a reserved command was unsent.
      this.ready = false;
      this.error = "ledgerUnavailable";
      return this.fail("ledgerUnavailable");
    }
  }

  private pruneReleasedTickets(): void {
    if (!this.bridge) return;
    for (const [id, record] of this.tickets) {
      if (!this.bridge.contains(record.operation).reserved) this.tickets.delete(id);
    }
  }

  setOwnerRevision(revision: number): EquipmentSessionResult {
    if (!validGeneration(revision) || revision < this.ownerRevision) return this.fail("staleOwner");
    this.ownerRevision = revision;
    return { ok: true, pending: this.pending() };
  }

  /** A close or logout intent suspends sends but does not release unknown in-flight work. */
  suspendConnection(identity: EquipmentSessionIdentity & { reason: "socketClosed" | "logoutPending" | "connectionUnavailable" }): EquipmentSessionResult {
    if (!this.matches(identity)) return this.fail("staleSession");
    if (this.suspendReason === "socketClosed" || this.suspendReason === "connectionUnavailable") {
      return this.fail("connectionSuspended");
    }
    this.ready = false;
    this.suspended = true;
    this.suspendReason = identity.reason;
    return { ok: true, pending: this.pending() };
  }

  /** Only an explicit failed logout on the same still-open socket restores this ledger. */
  resumeSameSession(identity: EquipmentSessionIdentity & { logoutFailed: true }): EquipmentSessionResult {
    if (!this.matches(identity)) return this.fail("staleSession");
    if (identity.logoutFailed !== true || !this.suspended || this.suspendReason !== "logoutPending") {
      return this.fail("cannotResume");
    }
    this.suspended = false;
    this.suspendReason = null;
    this.ready = this.error === null;
    return { ok: true, pending: this.pending() };
  }

  /** True session termination retires the ledger; a later snapshot alone cannot reactivate it. */
  terminateSession(identity: EquipmentSessionIdentity): EquipmentSessionResult {
    if (!this.matches(identity)) return this.fail("staleSession");
    this.bridge = null;
    this.tickets.clear();
    this.ready = false;
    this.suspended = false;
    this.suspendReason = null;
    this.error = null;
    return { ok: true, pending: 0 };
  }

  /** Call only after StartGame succeeds and its first complete authoritative inventory arrives. */
  establishBaseline(input: EquipmentSessionIdentity & {
    startGameConfirmed: true;
    complete: true;
    snapshot: EquipmentSnapshot;
  }): EquipmentSessionResult {
    if (input.startGameConfirmed !== true || input.complete !== true
      || !validGeneration(input.connectionGeneration) || !validGeneration(input.sessionGeneration)
      || input.sessionGeneration <= this.highestSessionGeneration
      || input.connectionGeneration < this.highestConnectionGeneration
      || (this.bridge !== null && this.ready && !this.suspended)) return this.fail("invalidBaseline");
    let candidate: EquipmentPendingRuntime;
    try {
      candidate = this.runtime.createEquipmentPendingLedger();
      const result = candidate.replaceSnapshot(input.snapshot);
      if (!result.ok) return this.fail(result.error ?? "invalidSnapshot");
    } catch {
      return this.fail("ledgerUnavailable");
    }
    this.bridge = candidate;
    this.connectionGeneration = input.connectionGeneration;
    this.sessionGeneration = input.sessionGeneration;
    this.highestConnectionGeneration = input.connectionGeneration;
    this.highestSessionGeneration = input.sessionGeneration;
    this.tickets.clear();
    this.ready = true;
    this.suspended = false;
    this.suspendReason = null;
    this.error = null;
    return { ok: true, pending: 0 };
  }

  /** A malformed or incomplete authoritative read model blocks new sends without clearing locks. */
  invalidateSnapshot(identity: EquipmentSessionIdentity): EquipmentSessionResult {
    if (!this.matches(identity)) return this.fail("staleSession");
    this.ready = false;
    this.error = "invalidSnapshot";
    return this.fail("invalidSnapshot");
  }

  /** Ignore stale sockets; malformed snapshots never mutate or unlock the existing ledger. */
  observeSnapshot(input: EquipmentSessionIdentity & { complete: true; snapshot: EquipmentSnapshot }): EquipmentSessionResult {
    if (!this.matches(input)) return this.fail("staleSession");
    if (this.suspendReason === "socketClosed" || this.suspendReason === "connectionUnavailable") return this.fail("connectionSuspended");
    if (input.complete !== true) return this.invalidateSnapshot(input);
    return this.call(() => {
      const result = this.bridge!.replaceSnapshot(input.snapshot);
      if (!result.ok) {
        this.ready = false;
        this.error = result.error ?? "invalidSnapshot";
        return this.fail(this.error);
      }
      this.pruneReleasedTickets();
      this.error = null;
      this.ready = !this.suspended;
      return { ok: true, pending: result.pending, released: result.released };
    });
  }

  reserve(input: EquipmentSessionIdentity & { ownerRevision: number; operation: EquipmentOperation }): EquipmentSessionResult {
    if (!this.matches(input)) return this.fail("staleSession");
    if (!this.ready || this.suspended) return this.fail("notReady");
    if (!validGeneration(input.ownerRevision) || input.ownerRevision !== this.ownerRevision) return this.fail("staleOwner");
    if (!Number.isSafeInteger(this.nextTicketId)) return this.fail("ticketCapacity");
    return this.call(() => {
      const result = this.bridge!.reserve(input.operation);
      if (!result.ok) return this.fail(result.error ?? "reserveRejected");
      const id = this.nextTicketId++;
      this.tickets.set(id, { operation: { ...input.operation }, state: "reserved" });
      return { ok: true, pending: result.pending,
        ticket: { id, connectionGeneration: this.connectionGeneration, sessionGeneration: this.sessionGeneration } };
    });
  }

  private ticket(ticket: EquipmentTicket): TicketRecord | null {
    return this.matches(ticket) && validGeneration(ticket.id) ? this.tickets.get(ticket.id) ?? null : null;
  }

  markSent(ticket: EquipmentTicket): EquipmentSessionResult {
    const record = this.ticket(ticket);
    if (!record || record.state !== "reserved") return this.fail("staleTicket");
    record.state = "sent";
    return { ok: true, pending: this.pending() };
  }

  /** Synchronous send threw after bytes may have been accepted: retain the lock. */
  markOutcomeUnknown(ticket: EquipmentTicket): EquipmentSessionResult {
    const record = this.ticket(ticket);
    if (!record || record.state !== "reserved") return this.fail("staleTicket");
    record.state = "unknown";
    return { ok: true, pending: this.pending() };
  }

  cancelDefinitelyUnsent(ticket: EquipmentTicket): EquipmentSessionResult {
    const record = this.ticket(ticket);
    if (!record || record.state !== "reserved") return this.fail("notProvenUnsent");
    return this.call(() => {
      const result = this.bridge!.releaseUnsent(record.operation);
      if (!result.ok || result.released !== true) return this.fail(result.error ?? "releaseRejected");
      this.tickets.delete(ticket.id);
      return { ok: true, pending: result.pending, released: true };
    });
  }

  /** ACK matching uses the full protocol tuple; owner revision is irrelevant. */
  applyAck(input: EquipmentSessionIdentity & { operation: EquipmentOperation; success: boolean }): EquipmentSessionResult {
    if (!this.matches(input) || this.suspendReason === "socketClosed" || this.suspendReason === "connectionUnavailable") {
      return this.fail("staleSession");
    }
    return this.call(() => {
      const result = this.bridge!.acknowledge(input.operation, input.success);
      if (!result.ok) return this.fail(result.error ?? "invalidAck");
      if (result.matched) {
        for (const record of this.tickets.values()) {
          if (sameOperation(record.operation, input.operation)) record.state = "acknowledged";
        }
        this.pruneReleasedTickets();
      }
      return { ok: true, matched: result.matched, pending: result.pending };
    });
  }

  hasPendingInstance(uniqueId: number): EquipmentSessionResult {
    if (!this.bridge) return this.fail("notReady");
    return this.call(() => {
      const result = this.bridge!.hasInstance(uniqueId);
      return result.ok
        ? { ok: true, reserved: result.reserved, pending: result.pending }
        : this.fail(result.error ?? "invalidIdentity");
    });
  }

  status(): EquipmentSessionStatus {
    let pending = this.tickets.size;
    let barriers = 0;
    try {
      const status = this.bridge?.status();
      if (status?.ok) { pending = status.pending; barriers = status.barriers ?? 0; }
    } catch { this.ready = false; this.error = "ledgerUnavailable"; }
    return {
      connectionGeneration: this.connectionGeneration,
      sessionGeneration: this.sessionGeneration,
      ready: this.ready,
      suspended: this.suspended,
      suspendReason: this.suspendReason,
      ownerRevision: this.ownerRevision,
      pending,
      barriers,
      error: this.error,
    };
  }
}
