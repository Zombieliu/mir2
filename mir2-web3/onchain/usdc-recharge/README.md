# Sui native USDC recharge V1

This package implements **test-only Sui native USDC → offchain game Credits**. It
is separate from the existing Dubhe procurement contract, where equipment still
costs offchain gold. Existing Stripe checkout is not replaced. No SUI-coin topup,
bridged USDC, fungible game token, NFT or production charging is introduced.

## State and trust boundary

An issuer uses its `AdminCap` to create a shared `Invoice` for one persisted game
order. Every invoice freezes the network/config, opaque order/account references,
offer/rule snapshot hash, payer, merchant, canonical coin type, exact six-decimal
amount and expiry. The issuer does not receive the player's wallet key.

The payer signs `pay<USDC>` using Wallet Standard. The contract checks the wallet
sender, exact coin type/value, invoice status, config and exclusive Clock
deadline. A single transaction transfers the original USDC coin to the merchant,
sets `paid`, and emits `PaymentReceived`. Failure reverts those effects. The
invoice is the only shared mutable object in a payment; config and Clock are
read-only. New-invoice publication can be paused without blocking an existing
invoice's payment or the server's later delivery.

There is no per-player/per-order contract deployment or global mutable payment
registry. **Logical order uniqueness is an issuer/database responsibility**:
persist exact signed creation bytes and digest before broadcast, bind exactly one
confirmed invoice ID, and never create a replacement just because a response is
unknown. The admin can create invoices and pause publication; it cannot alter an
existing invoice, withdraw a player's coins or directly credit the game database.
The package UpgradeCap remains a code-upgrade trust boundary. Production key
custody, audited upgrade policy and rollout remain unapproved.

The contract cannot compel an offline game server to deliver Credits. The
Gateway verifies fullnode evidence and commits a persistent paid receipt before
the existing game authority applies Credits idempotently. This is a trusted
merchant integration, not a trustless exchange.

## CLI protocol

Run Node 22+ with `node service/cli.mjs`. Each process accepts one UTF-8 JSON
request on stdin, capped at 1 MiB, and emits exactly one JSON line on stdout:
`{"ok":true,"result":...}` or `{"ok":false,"error":"usdcStableCode"}`.
The driver has a 19-second deadline; its caller must enforce a 20-second process
deadline/output limit. No diagnostic exception, environment or key is serialized.

Request envelope: `{operation, config, params}`. All field sets are exact.

Config fields are `enabled: true`, `environmentTag: "isolated-test"`,
`network: "testnet"`, `chainIdentifier: "4c78adac"`, `networkHash`,
`fullnodeUrl`, `packageId`, `configId`, `adminCapId`, `issuerAddress`,
`merchantAddress`, `coinType`, `gasBudgetMist`. Only the official Sui Testnet
fullnode, exact pinned genesis and Circle's exact Testnet native USDC are accepted.
Addresses/hashes are lowercase `0x` plus 64 hexadecimal characters; all u64 values
are decimal strings. Gas budget is positive and at most 50,000,000 MIST.

Frozen `order` fields:

```json
{
  "orderRef": "0x<sha256(order.id UTF8), 64 hex>",
  "accountRef": "0x<server opaque account reference, 64 hex>",
  "snapshotHash": "0x<server frozen offer/rule/order hash, 64 hex>",
  "payer": "0x<confirmed wallet address, 64 hex>",
  "merchant": "0x<configured merchant, 64 hex>",
  "amountMicros": "1000000",
  "expiresAtMs": "<decimal milliseconds>"
}
```

Payer and merchant must differ. `1 USDC = 1,000,000` units. The server sets all
commercial values from its frozen offer, never from the browser.

| Operation | Params | Result |
| --- | --- | --- |
| `configureValidate` | `{}` | `{validated, network, networkHash, publishingEnabled}` after actual fullnode config/cap/package/domain checks |
| `verifyWalletProof` | `{message, signature, address}` | `{verified:true,address}`; Gateway must first require its exact current challenge, origin and expiry |
| `prepareInvoice` | `{order}` | `{bytesB64,signature,digest}`; build/sign only, no broadcast |
| `submitPrepared` | `{prepared,order}` | `{submitted:true,digest}`; validates exact signed allowed creation bytes before broadcast |
| `observeCreate` | `{prepared,order}` | `{state:"unknown"}` / checkpoint-confirmed `failed` / `created` with `invoiceId,digest,checkpoint` |
| `buildPay` | `{invoiceId,order}` | `{transactionJson,invoiceId,payer,gasPayer:"wallet"}`; unsigned wallet transaction JSON, no broadcast |
| `observeInvoice` | `{invoiceId,order}` | `unknown`, `unpaid`, or `paid` with the structured receipt below |

The issuer key is read only from the explicit environment variable
`MIR2_SUI_USDC_ISSUER_SECRET_KEY` in Sui `suiprivkey` Bech32 Ed25519 format. Its
derived address must match the configured issuer and the onchain AdminCap owner.
No keys are generated automatically, read from a default wallet or sent to a
browser. Read-only and wallet-proof operations require no issuer key.

Issuer creation execution also requires an absolute external private path in
`MIR2_SUI_USDC_GAS_LEDGER_PATH`. `MIR2_SUI_USDC_GAS_MAX_MIST` is a lifetime limit,
defaulting to 50,000,000 MIST and capped at 1,000,000,000. Before any submission,
the driver exclusively locks the ledger, reserves the full configured gas budget,
fsyncs the file and atomically replaces it. Exact digest retries consume no new
reservation; another digest must fit the remaining lifetime limit. Network,
genesis, package, config, issuer, per-transaction budget and lifetime limit are
fixed in the ledger. There is no daily/process reset or automatic reservation
refund, including after a failed or unknown result. A stale lock or corrupt file
fails closed. The current acceptance limit is 50,000,000 MIST for one invoice.

`paid.receipt` contains `networkHash, packageId, configId, invoiceId, orderRef,
accountRef, snapshotHash, payer, merchant, coinType, amountMicros, expiresAtMs,
paidAtMs, transactionDigest, eventSequence, checkpoint`. The verifier checks the
checkpoint-successful transaction, sender, exact event type and strictly decoded
BCS, mutated object version/digest, invoice terms and a second coherent object
read. A client digest, JSON event, indexed result or balance difference alone is
never a payment receipt. A changing/missing/unfinalized observation is `unknown`.

The backend polls each durable invoice and creation digest directly; it does not
depend on a realtime/indexer subscription. Keep paid/unapplied orders in recovery
after closing checkout, disabling new orders or passing local expiry. A confirmed
payment whose chain `paidAtMs` precedes its deadline remains payable to the game
account even if its DB delivery occurs later. Do not declare an invoice
permanently unpaid from a network timeout.

## Gas, fees and test assets

Per successful order: **1 issuer creation transaction + 1 payer payment
transaction**. Deployment, config creation and publication enablement are separate
setup operations. Per-order Gas has not yet been measured; simulation,
unit tests and earlier procurement costs are not substituted for per-order USDC
Gas. The real testnet setup was subsequently confirmed: publish 25,677,200 MIST,
config 3,872,956 MIST and enable 1,000,443 MIST, total **30,550,599 MIST**
(0.030550599 test SUI). These three setup operations are separate from every
recharge's invoice creation and payment.

In this version the wallet pays normal **SUI Gas** for its USDC payment. Custom
Move calls are not eligible for Sui's restricted gasless stablecoin transfer path.
There is no arbitrary sponsorship API. The UI must display this before consent.
Optional future official sponsorship requires a distinct restricted budget and
signed-operation validation; it is not implemented here.

Mainnet is refused. Testnet USDC must be recorded as a test credit source and must
not be counted as real receipts. [Circle's faucet](https://faucet.circle.com/)
supports Sui Testnet and currently requires human reCAPTCHA. Existing isolated
test wallets were checked read-only on 2026-10-29 (Asia/Shanghai): all three had
zero USDC at the initial audit. The user then obtained 20 test USDC for the buyer.
A new independent test-only issuer and the buyer each received one free test SUI
from Sui's documented public PoW faucet. Their checkpoint-confirmed receipts are
in the validation record. No CAPTCHA was bypassed. The independent issuer avoids
sharing owned gas objects with the existing procurement worker. No USDC payment
has yet been broadcast in this recorded stage.

## Build and tests

`npm ci --ignore-scripts --offline` and `npm test` use the fixed SDK/lockfile;
`.npmrc` avoids selecting an unrelated optional TypeScript peer. Do not run
install scripts or change the SDK version implicitly.

The Move manifest pins Sui commit
`3c0f387ebb40b8be292d3b7bd3f5bee8ad226d33`. The verified official compiler is
1.68.1. For keyless compilation, set `SUI_CONFIG_DIR` to an external dedicated
testnet client configuration with an empty keystore and no active address. The
recorded build used an external source-identical copy with only dependency paths
redirected to the already verified local framework cache. It did not mutate the
procurement package, Docker, default wallet or any running server.

Current bounded validation: **22 Node tests and 14 Move tests passed**. Node
coverage includes exact signed creation bytes, wrong terms/payer/type/BCS/effects,
unknown/coherence handling, personal-message proof, unsigned wallet transaction
and CLI limits, lifetime gas reservations across subprocess restarts, domain/
budget pinning, lock/corruption refusal and unknown-result exact replay. Move uses
an explicitly test-only SUI coin/config fixture for
actual transfer/quantity checks and independently tests native-type mismatch; it
does not claim Circle Testnet USDC execution. Production config hardcodes the
Circle Testnet type and has no coin-type setter.

See [version/source decisions](UPSTREAM.json) and [validation record](validation/README.md).
Real package deployment/config/enable and fullnode validation passed. The
Circle-USDC wallet payment, Windows acceptance, native PostgreSQL and multi-Gateway
recovery remain separate integration gates at this recorded stage.
