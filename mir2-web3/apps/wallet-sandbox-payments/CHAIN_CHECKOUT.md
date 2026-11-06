# Numeron native USDC QA checkout

This optional adapter uses the same restricted and locked QA character as the Stripe test adapter, with independent order/receipt tables. It never accepts an arbitrary player ID, touches ordinary player accounts, or treats a submitted transaction as paid. Do not deploy it for Mainnet or ordinary billing.

Install the existing Stripe QA receiver's migration and restricted views in the test database first, then apply `chain_migration.sql` as the database owner. It adds separate chain tables and permissions only to those tables. The adapter runs without a listener. Keep the existing `NUMERON_SANDBOX_*` test receiver settings. Supply separately:

- `NUMERON_CHAIN_QA_ENABLED=true`
- `NUMERON_CHAIN_SERVICE_ORIGIN` (wallet Testnet service HTTPS origin), `NUMERON_CHAIN_WALLET_ORIGIN` (where the QA Passkey was created), `NUMERON_CHAIN_MERCHANT_TOKEN` (service's separate merchant credential)
- `NUMERON_CHAIN_REGISTRY`, `NUMERON_CHAIN_PAYEE`, and `NUMERON_CHAIN_QA_PAYERS` (comma-separated allowlisted Testnet addresses)

The authenticated game-side order handler, or a QA operator, runs `python chain_checkout.py prepare --payer <test-address> --request-id <stable 64 hex characters>`. Return the checkout link privately to that player as a QR or same-device link. Never log the fragment. A game-side job runs `python chain_checkout.py drain` periodically. Delivery pulls the service's verified onchain receipts, compares the exact order and QA recipient, commits the database transaction, then acknowledges the grant. A lost ACK can only replay the persisted grant. A failed or unconfirmed payment does not reach the credit write. A nonzero `deferred` count needs retry or operator review; do not create a replacement payment.

Use a dedicated Postgres test cluster for tests. `WALLET_SANDBOX_TEST_DSN=… python -m unittest -v test_service` runs the QA receiver checks. The live Testnet test is opt-in and needs a disposable payer, funded sponsor and wallet service; see the test's explicit environment flags. Do not point it at game production.
