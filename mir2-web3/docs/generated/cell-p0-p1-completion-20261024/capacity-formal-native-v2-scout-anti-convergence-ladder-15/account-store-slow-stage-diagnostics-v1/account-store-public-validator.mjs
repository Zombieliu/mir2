import { createHash } from 'node:crypto';
import { accountStoreRowProblem, ALLOWED_ACCOUNT_STORE_STAGES, ALLOWED_ACCOUNT_STORE_OPERATIONS,
  MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES } from './slow-stage-public.mjs';

const captureReasons = new Set([
  'request-stop-error', 'noncanonical-duration', 'noncanonical-connection', 'unsafe-connection',
  'noncanonical-phase-elapsed', 'unsafe-phase-elapsed', 'unsafe-duration', 'invalid-receive-clock',
  'public-byte-cap', 'write-buffer-cap', 'write-error', 'write-backpressure', 'non-buffer-stderr',
  'line-byte-cap', 'partial-line-at-eof', 'stderr-read-error', 'stderr-closed-before-end',
  'metadata-write-error', 'metadata-closed-before-finish', 'gateway-not-exited', 'stderr-end-timeout',
  'metadata-end-error', 'metadata-finish-timeout', 'account-store-public-shape', 'account-store-allowlist',
  'account-store-unsafe-number', 'account-store-positive-number', 'account-store-inconsistent-range',
  'account-store-wire-format', 'account-store-noncanonical-number', 'account-store-non-ascii', 'account-store-receive-clock',
]);
const safe = number => Number.isSafeInteger(number) && number >= 0 && !Object.is(number, -0);
function statusProblem(status) {
  if (!status || typeof status !== 'object' || Array.isArray(status) || status.schemaVersion !== 1 || status.dataset !== 'account-store') return true;
  for (const key of ['complete', 'stopRequested', 'stderrEnded', 'writerFinished']) if (typeof status[key] !== 'boolean') return true;
  for (const key of ['rowsQueued', 'bytesQueued', 'maxBufferedLineBytes', 'lineLimitBytes', 'totalLimitBytes',
    'writeLimitBytes', 'writeHighWaterMarkBytes', 'maxQueuedWriteBytes']) if (!safe(status[key])) return true;
  if (!Array.isArray(status.reasons) || status.reasons.length > captureReasons.size || new Set(status.reasons).size !== status.reasons.length ||
      status.reasons.some(reason => !captureReasons.has(reason))) return true;
  if (status.complete !== (status.reasons.length === 0 && status.stderrEnded && status.writerFinished) ||
      status.stopRequested !== (status.reasons.length > 0)) return true;
  return status.lineLimitBytes < 1 || status.lineLimitBytes > MAX_LINE_BYTES || status.maxBufferedLineBytes > status.lineLimitBytes ||
    status.totalLimitBytes < 1 || status.totalLimitBytes > MAX_PUBLIC_BYTES || status.bytesQueued > status.totalLimitBytes ||
    status.writeLimitBytes < 1 || status.writeLimitBytes > MAX_WRITE_BUFFER_BYTES || status.maxQueuedWriteBytes > status.writeLimitBytes ||
    status.writeHighWaterMarkBytes < 1 || status.writeHighWaterMarkBytes > MAX_WRITE_BUFFER_BYTES;
}

// Reads only the independent Store JSONL supplied by the caller. No file paths, writer, game or acceptance state is accessed.
// The caller must separately pin this source, the capture source/status and the ended JSONL before/after reading.
export async function validateAccountStorePublicDataset({ input, status }) {
  const reasons = new Set(), hash = createHash('sha256'), line = Buffer.alloc(MAX_LINE_BYTES);
  const rowsByStage = Object.fromEntries(ALLOWED_ACCOUNT_STORE_STAGES.map(stage => [stage, 0]));
  const rowsByOperation = Object.fromEntries(ALLOWED_ACCOUNT_STORE_OPERATIONS.map(operation => [operation, 0]));
  let bytes = 0, rows = 0, validRows = 0, used = 0, readEnded = false, halted = false;
  const fail = reason => { reasons.add(reason); halted = true; };
  if (statusProblem(status)) reasons.add('store-status-invalid');
  const emit = () => {
    rows++;
    let row;
    try { row = JSON.parse(line.toString('utf8', 0, used)); } catch { fail('store-public-json-invalid'); return; }
    if (accountStoreRowProblem(row)) { fail('store-public-row-invalid'); return; }
    validRows++; rowsByStage[row.stage]++; rowsByOperation[row.operation]++;
  };
  try {
    for await (const chunk of input) {
      if (!Buffer.isBuffer(chunk)) { fail('store-dataset-non-buffer'); break; }
      if (bytes + chunk.length > MAX_PUBLIC_BYTES) { fail('store-dataset-byte-cap'); break; }
      bytes += chunk.length; hash.update(chunk);
      for (const byte of chunk) {
        if (byte === 10) { emit(); line.fill(0); used = 0; }
        else if (used === MAX_LINE_BYTES) { fail('store-public-line-cap'); break; }
        else line[used++] = byte;
        if (halted) break;
      }
      if (halted) break;
    }
    readEnded = !halted;
  } catch { fail('store-dataset-read-error'); }
  if (used) reasons.add('store-public-partial-line');
  line.fill(0);
  const countsMatch = !statusProblem(status) && readEnded && rows === status.rowsQueued && bytes === status.bytesQueued;
  if (!countsMatch) reasons.add('store-queued-file-count-mismatch');
  const captureComplete = !statusProblem(status) && status.complete;
  return { schemaVersion: 1, dataset: 'account-store', complete: reasons.size === 0 && captureComplete,
    formatValid: reasons.size === 0, captureComplete, countsMatch, readEnded, reasons: [...reasons],
    captureReasons: statusProblem(status) ? [] : [...status.reasons], rows, validRows, bytes, sha256: hash.digest('hex'), rowsByStage, rowsByOperation,
    scope: 'Independent completed account-store scalar projection only. Queued callback counts are compared with actual ended JSONL; they are not durable proof alone. Completeness is diagnostic only, never game acceptance, ACK credit, a holder/deadlock diagnosis, or proof of absence.' };
}
