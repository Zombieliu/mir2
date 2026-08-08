import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminMarketReadModel } from "../../lib/admin-api";
import { formatNumber } from "../../lib/format";
import {
  cancelMarketListingAction,
  deleteMarketListingAction,
  expireMarketListingAction
} from "../ops-actions";

export default async function MarketPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const market = await adminGet<AdminMarketReadModel>("/admin/read/market");
  const listings = market.ok ? market.data.listings : [];

  return (
    <AdminShell active="/market">
      <div className="page-head">
        <div>
          <p className="eyebrow">Market Console</p>
          <h2>Auctions and listing moderation</h2>
          <p className="muted">Search active Stage 5 auction state, cancel suspicious listings, and notify sellers.</p>
        </div>
        <StatusBadge tone={market.ok ? "success" : "warn"}>
          {market.ok ? market.data.source : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!market.ok ? <p className="notice">{market.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Listings</p>
          <table className="table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Owner</th>
                <th>Item</th>
                <th>Price</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {listings.map((listing) => (
                <tr key={`${listing.ownerPlayerId}-${listing.listingId}`}>
                  <td>{listing.listingId}</td>
                  <td>
                    <strong>{listing.characterName}</strong>
                    <div className="muted">{listing.ownerPlayerId}</div>
                  </td>
                  <td>{listing.itemKey}</td>
                  <td>{formatNumber(listing.price)}</td>
                  <td>
                    {listing.cancelled ? "Cancelled" : listing.expired ? "Expired" : listing.sold ? "Sold" : "Active"}
                  </td>
                </tr>
              ))}
              {!listings.length ? (
                <tr>
                  <td colSpan={5}>No auction listings found.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Moderation</p>
          <h3>Cancel listing</h3>
          <form action={cancelMarketListingAction} className="form-stack">
            <input className="control" name="characterId" placeholder="seller character id or name" />
            <input className="control" name="listingId" placeholder="listing id" type="number" />
            <label className="check-row">
              <input name="notifySeller" type="checkbox" defaultChecked />
              <span>Notify seller by system mail</span>
            </label>
            <input className="control" name="reason" defaultValue="market moderation from admin console" />
            <SubmitButton idle="Cancel listing" pending="Cancelling..." />
          </form>
          <div className="rune-divider" />
          <h3>Expire listing</h3>
          <form action={expireMarketListingAction} className="form-stack">
            <input className="control" name="characterId" placeholder="seller character id or name" />
            <input className="control" name="listingId" placeholder="listing id" type="number" />
            <label className="check-row">
              <input name="notifySeller" type="checkbox" defaultChecked />
              <span>Notify seller by system mail</span>
            </label>
            <input className="control" name="reason" defaultValue="expire listing from admin console" />
            <SubmitButton idle="Expire listing" pending="Expiring..." />
          </form>
          <div className="rune-divider" />
          <h3>Delete listing</h3>
          <form action={deleteMarketListingAction} className="form-stack">
            <input className="control" name="characterId" placeholder="seller character id or name" />
            <input className="control" name="listingId" placeholder="listing id" type="number" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reasonMessage" placeholder="seller notice message" />
            <input className="control" name="reason" defaultValue="delete listing from admin console" />
            <SubmitButton idle="Delete listing" pending="Deleting..." />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
