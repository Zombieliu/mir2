import type { EquipmentGatewaySnapshot } from "./equipment-gateway-adapter";
import { mailMutationAllowed } from "./mail-parcel-gateway-adapter";

export type BagBeltMoveOwner = Readonly<{ socket: object; connectionGeneration: number;
  sessionGeneration: number; playerObjectId: number }>;
export type BagBeltMoveCommand = Readonly<{ type: "moveItem"; grid: "belt"; from: number; to: number }>;
export type BagBeltMoveReservation = Readonly<{ token: object }>;
type Flight = { proof: BagBeltMoveReservation; owner: BagBeltMoveOwner; command: BagBeltMoveCommand;
  sourceSlot: number; uniqueId: number; targetUniqueId: number | null; snapshotVersion: number;
  entered: boolean; acknowledged: boolean; observed: boolean };
const safe = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const sameOwner = (a: BagBeltMoveOwner, b: BagBeltMoveOwner) => a.socket === b.socket
  && a.connectionGeneration === b.connectionGeneration && a.sessionGeneration === b.sessionGeneration
  && a.playerObjectId === b.playerObjectId;

/** Local transport custody for the existing raw MoveItem tuple. Core owns index planning. */
export class BagBeltMoveDispatcher {
  private readonly flights = new Set<Flight>();
  private readonly issued = new WeakMap<BagBeltMoveReservation, Flight>();
  private connection: Readonly<{ socket: object; generation: number }> | null = null;

  /** Only a newer physical connection can retire an uncertain old transport. */
  retireConnection(socket: object, generation: number): boolean {
    if (!safe(generation) || generation === 0) return false;
    if (this.connection?.socket === socket) return this.connection.generation === generation;
    if (this.connection && generation <= this.connection.generation) return false;
    for (const flight of this.flights) this.issued.delete(flight.proof);
    this.flights.clear();
    this.connection = { socket, generation };
    return true;
  }

  get pending(): number { return this.flights.size; }
  blockedUniqueIds(): number[] {
    return [...new Set([...this.flights].flatMap(flight => flight.targetUniqueId === null
      ? [flight.uniqueId] : [flight.uniqueId, flight.targetUniqueId]))];
  }

  isCellReserved(container: 0 | 1, slot: number): boolean {
    return [...this.flights].some(flight => container === 0 ? flight.sourceSlot === slot : flight.command.to === slot);
  }

  private current(owner: BagBeltMoveOwner): boolean {
    return !!this.connection && this.connection.socket === owner.socket
      && this.connection.generation === owner.connectionGeneration && safe(owner.sessionGeneration)
      && owner.sessionGeneration > 0 && safe(owner.playerObjectId) && owner.playerObjectId > 0;
  }

  reserve(owner: BagBeltMoveOwner, command: BagBeltMoveCommand, sourceSlot: number,
    uniqueId: number, targetUniqueId: number | null, snapshotVersion: number,
    layout: EquipmentGatewaySnapshot): BagBeltMoveReservation | null {
    if (!this.current(owner) || this.flights.size >= 64 || command.type !== "moveItem" || command.grid !== "belt"
      || !safe(command.from) || command.from < 6 || command.from >= layout.capacity
      || !safe(command.to) || command.to >= 6 || !safe(sourceSlot) || sourceSlot >= 80
      || !safe(uniqueId) || targetUniqueId !== null && (!safe(targetUniqueId) || targetUniqueId === uniqueId)
      || !safe(snapshotVersion)) return null;
    const source = layout.placements.filter(p => p.container === 0 && p.slot === sourceSlot);
    const target = layout.placements.filter(p => p.container === 1 && p.slot === command.to);
    if (source.length !== 1 || source[0].uniqueId !== uniqueId
      || layout.placements.filter(p => p.uniqueId === uniqueId).length !== 1
      || targetUniqueId === null && target.length !== 0
      || targetUniqueId !== null && (target.length !== 1 || target[0].uniqueId !== targetUniqueId
        || layout.placements.filter(p => p.uniqueId === targetUniqueId).length !== 1)
      || !this.mutationAllowed(command, layout)) return null;
    const proof = Object.freeze({ token: Object.freeze({}) });
    const flight: Flight = { proof, owner: Object.freeze({ ...owner }), command: Object.freeze({ ...command }),
      sourceSlot, uniqueId, targetUniqueId, snapshotVersion, entered: false, acknowledged: false, observed: false };
    this.flights.add(flight); this.issued.set(proof, flight);
    return proof;
  }

  allows(proof: BagBeltMoveReservation, owner: BagBeltMoveOwner, command: Record<string, unknown>): boolean {
    const flight = this.issued.get(proof);
    return !!flight && this.flights.has(flight) && !flight.entered && this.current(owner)
      && sameOwner(flight.owner, owner) && command.type === flight.command.type && command.grid === flight.command.grid
      && command.from === flight.command.from && command.to === flight.command.to
      && Object.keys(command).length === 4;
  }

  /** Called immediately before socket.send, after all reentrant listeners and source checks. */
  enter(proof: BagBeltMoveReservation, owner: BagBeltMoveOwner, command: Record<string, unknown>): boolean {
    if (!this.allows(proof, owner, command)) return false;
    this.issued.get(proof)!.entered = true;
    return true;
  }

  cancelDefinitelyUnsent(proof: BagBeltMoveReservation): boolean {
    const flight = this.issued.get(proof);
    if (!flight || flight.entered) return false;
    this.flights.delete(flight); this.issued.delete(proof); return true;
  }

  /** MoveItem ACK carries grid/from/to/success. UID and unrelated ACK families cannot release it. */
  acknowledge(owner: BagBeltMoveOwner, payload: Record<string, unknown>): boolean {
    if (!this.current(owner) || typeof payload.grid !== "string" || payload.grid.toLowerCase() !== "belt"
      || !safe(payload.from) || !safe(payload.to) || typeof payload.success !== "boolean") return false;
    const matches = [...this.flights].filter(flight => flight.entered && sameOwner(flight.owner, owner)
      && flight.command.from === payload.from && flight.command.to === payload.to);
    if (matches.length !== 1) return false;
    const flight = matches[0];
    if (!payload.success) { this.flights.delete(flight); this.issued.delete(flight.proof); return true; }
    flight.acknowledged = true;
    this.releaseObserved(flight); return true;
  }

  /** Observe only a newer complete authoritative layout, including occupied-target exchanges. */
  observe(owner: BagBeltMoveOwner, layout: EquipmentGatewaySnapshot, snapshotVersion: number): boolean {
    if (!this.current(owner) || !safe(snapshotVersion)) return false;
    let changed = false;
    for (const flight of this.flights) {
      if (!flight.entered || !sameOwner(flight.owner, owner) || snapshotVersion <= flight.snapshotVersion) continue;
      const source = layout.placements.filter(p => p.container === 0 && p.slot === flight.sourceSlot);
      const target = layout.placements.filter(p => p.container === 1 && p.slot === flight.command.to);
      if (target.length !== 1 || target[0].uniqueId !== flight.uniqueId
        || layout.placements.filter(p => p.uniqueId === flight.uniqueId).length !== 1
        || flight.targetUniqueId === null && source.length !== 0
        || flight.targetUniqueId !== null && (source.length !== 1 || source[0].uniqueId !== flight.targetUniqueId
          || layout.placements.filter(p => p.uniqueId === flight.targetUniqueId).length !== 1)) continue;
      flight.observed = true; changed = this.releaseObserved(flight) || changed;
    }
    return changed;
  }

  private releaseObserved(flight: Flight): boolean {
    if (!flight.acknowledged || !flight.observed) return false;
    this.flights.delete(flight); this.issued.delete(flight.proof); return true;
  }

  /** Reserve both current identities and cells; an empty destination remains protected. */
  mutationAllowed(command: Record<string, unknown>, layout: EquipmentGatewaySnapshot | null,
    own?: BagBeltMoveReservation): boolean {
    const flights = [...this.flights].filter(flight => flight.proof !== own);
    if (flights.length === 0) return true;
    const ids = [...new Set(flights.flatMap(flight => flight.targetUniqueId === null
      ? [flight.uniqueId] : [flight.uniqueId, flight.targetUniqueId]))];
    const cells = flights.flatMap(flight => [{ container: 0, slot: flight.sourceSlot },
      { container: 1, slot: flight.command.to }]);
    return mailMutationAllowed(command, layout && { bagCapacity: layout.capacity - 6,
      items: layout.placements.map(p => ({ ...p, pricing: null, stamp: false })) }, ids, cells);
  }
}
