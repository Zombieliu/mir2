import { createHash } from 'node:crypto';
import type { SuiNetwork } from './config';

export interface SuiFinalityProof {
  network: string;
  packageId: string;
  transactionDigest: string;
  eventSequence: number;
  checkpoint: number;
}

export interface FinalizedGuildNodeRegistration {
  nodeId: string;
  operatorSuiAddress: string;
  publicKey: string;
  endpoint: string;
  failureDomain: string;
  stakeMist: number;
  maxSessions: number;
  maxZones: number;
  keyGeneration: number;
  status: 'active' | 'revoked';
  finality: SuiFinalityProof;
}

export interface FinalizedNodeEvent {
  type: string;
  parsedJson: Record<string, unknown>;
  transactionDigest: string;
  eventSequence: number;
  checkpoint: number;
}

function numeric(value: unknown, label: string): number {
  const parsed = typeof value === 'number' ? value : Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new Error(`${label} must be a non-negative safe integer`);
  }
  return parsed;
}

function bytes(value: unknown, label: string): Uint8Array {
  if (typeof value === 'string') {
    const decoded = Buffer.from(value, 'base64');
    if (decoded.length === 0 && value.length > 0) {
      throw new Error(`${label} must be a byte vector`);
    }
    return decoded;
  }
  if (!Array.isArray(value) || !value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)) {
    throw new Error(`${label} must be a byte vector`);
  }
  return Uint8Array.from(value as number[]);
}

function text(value: unknown, label: string): string {
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes(value, label));
}

function hex(value: unknown, label: string): string {
  return Buffer.from(bytes(value, label)).toString('hex');
}

function base64url(value: unknown, label: string): string {
  return Buffer.from(bytes(value, label)).toString('base64url');
}

function finality(
  network: SuiNetwork,
  packageId: string,
  event: FinalizedNodeEvent,
): SuiFinalityProof {
  return {
    network,
    packageId,
    transactionDigest: event.transactionDigest,
    eventSequence: event.eventSequence,
    checkpoint: event.checkpoint,
  };
}

/** Replay finalized Sui events into the deterministic membership snapshot Commonware consumes. */
export function projectFinalizedNodeEvents(
  network: SuiNetwork,
  packageId: string,
  events: FinalizedNodeEvent[],
): FinalizedGuildNodeRegistration[] {
  const nodes = new Map<string, FinalizedGuildNodeRegistration>();
  const ordered = [...events].sort(
    (left, right) =>
      left.checkpoint - right.checkpoint ||
      left.transactionDigest.localeCompare(right.transactionDigest) ||
      left.eventSequence - right.eventSequence,
  );
  for (const event of ordered) {
    const data = event.parsedJson;
    const type = event.type.split('::').at(-1);
    const nodeId = `ed25519:${hex(data.node_id, 'node_id')}`;
    if (type === 'NodeRegisteredEvent') {
      const publicKey = base64url(data.public_key, 'public_key');
      if (`ed25519:${hexDigest(Buffer.concat([
        Buffer.from('obelisk.guild-node.ed25519.v1\0'),
        Buffer.from(bytes(data.public_key, 'public_key')),
      ]))}` !== nodeId) {
        throw new Error(`registered node id does not match public key: ${nodeId}`);
      }
      nodes.set(nodeId, {
        nodeId,
        operatorSuiAddress: String(data.operator),
        publicKey,
        endpoint: text(data.endpoint, 'endpoint'),
        failureDomain: text(data.failure_domain, 'failure_domain'),
        stakeMist: numeric(data.stake_mist, 'stake_mist'),
        maxSessions: numeric(data.max_sessions, 'max_sessions'),
        maxZones: numeric(data.max_zones, 'max_zones'),
        keyGeneration: numeric(data.generation, 'generation'),
        status: 'active',
        finality: finality(network, packageId, event),
      });
      continue;
    }
    const current = nodes.get(nodeId);
    if (!current) throw new Error(`${type ?? 'unknown event'} references unknown node ${nodeId}`);
    if (type === 'NodeKeyRotatedEvent') {
      current.publicKey = base64url(data.public_key, 'public_key');
      current.keyGeneration = numeric(data.generation, 'generation');
    } else if (type === 'NodeMetadataUpdatedEvent') {
      current.endpoint = text(data.endpoint, 'endpoint');
      current.failureDomain = text(data.failure_domain, 'failure_domain');
      current.maxSessions = numeric(data.max_sessions, 'max_sessions');
      current.maxZones = numeric(data.max_zones, 'max_zones');
      current.keyGeneration = numeric(data.generation, 'generation');
    } else if (type === 'NodeSlashedEvent') {
      current.stakeMist = numeric(data.remaining_stake_mist, 'remaining_stake_mist');
      current.status = data.active === true ? 'active' : 'revoked';
    } else if (type === 'NodeRevokedEvent') {
      current.stakeMist = 0;
      current.status = 'revoked';
    } else {
      continue;
    }
    current.finality = finality(network, packageId, event);
  }
  return [...nodes.values()].sort((left, right) => left.nodeId.localeCompare(right.nodeId));
}

function hexDigest(value: Uint8Array): string {
  return createHash('sha256').update(value).digest('hex');
}

export class FinalizedNodeRegistrySource {
  private readonly graphqlUrl: string;

  constructor(
    private readonly network: SuiNetwork,
    private readonly packageId: string,
    graphqlUrl?: string,
  ) {
    this.graphqlUrl = graphqlUrl ?? defaultGraphqlUrl(network);
  }

  async snapshot(): Promise<FinalizedGuildNodeRegistration[]> {
    const events: FinalizedNodeEvent[] = [];
    let cursor: string | null = null;
    do {
      const page = await queryNodeEvents(
        this.graphqlUrl,
        `${this.packageId}::node_registry`,
        cursor,
      );
      for (const event of page.nodes) {
        events.push({
          type: event.contents.type.repr,
          parsedJson: event.contents.json,
          transactionDigest: event.transaction.digest,
          eventSequence: numeric(event.sequenceNumber, 'event sequence'),
          checkpoint: numeric(
            event.transaction.effects.checkpoint.sequenceNumber,
            'checkpoint',
          ),
        });
      }
      cursor = page.pageInfo.hasNextPage ? page.pageInfo.endCursor : null;
    } while (cursor !== null);
    return projectFinalizedNodeEvents(this.network, this.packageId, events);
  }
}

interface GraphqlNodeEvent {
  sequenceNumber: number;
  contents: {
    type: { repr: string };
    json: Record<string, unknown>;
  };
  transaction: {
    digest: string;
    effects: { checkpoint: { sequenceNumber: number } };
  };
}

interface GraphqlEventPage {
  pageInfo: { hasNextPage: boolean; endCursor: string | null };
  nodes: GraphqlNodeEvent[];
}

async function queryNodeEvents(
  url: string,
  module: string,
  after: string | null,
): Promise<GraphqlEventPage> {
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      query: `query NodeRegistryEvents($module: String!, $after: String) {
        events(first: 50, after: $after, filter: { module: $module }) {
          pageInfo { hasNextPage endCursor }
          nodes {
            sequenceNumber
            contents { type { repr } json }
            transaction {
              digest
              effects { checkpoint { sequenceNumber } }
            }
          }
        }
      }`,
      variables: { module, after },
    }),
  });
  if (!response.ok) {
    throw new Error(`Sui GraphQL returned HTTP ${response.status}`);
  }
  const payload = (await response.json()) as {
    data?: { events?: GraphqlEventPage };
    errors?: Array<{ message?: string }>;
  };
  if (payload.errors?.length) {
    throw new Error(
      `Sui GraphQL query failed: ${payload.errors
        .map((error) => error.message ?? 'unknown error')
        .join('; ')}`,
    );
  }
  if (!payload.data?.events) {
    throw new Error('Sui GraphQL response did not contain events');
  }
  return payload.data.events;
}

function defaultGraphqlUrl(network: SuiNetwork): string {
  if (network === 'localnet') return 'http://127.0.0.1:9125/graphql';
  return `https://graphql.${network}.sui.io/graphql`;
}
