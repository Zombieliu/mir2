import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminContentReadModel } from "../../lib/admin-api";
import { formatNumber } from "../../lib/format";
import { publishContentBundleAction } from "../ops-actions";

export default async function ContentPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const commandId = firstParam(params.commandId);
  const content = await adminGet<AdminContentReadModel>("/admin/read/content");
  const data = content.ok ? content.data : undefined;

  return (
    <AdminShell active="/content">
      <div className="page-head">
        <div>
          <p className="eyebrow">Content Database</p>
          <h2>Crystal data manifests and override bundles</h2>
          <p className="muted">
            Review imported maps, items, monsters, NPCs, quests, magic, game shop, recipes, and publish audited JSON override bundles.
          </p>
        </div>
        <StatusBadge tone={content.ok ? "success" : "warn"}>
          {content.ok ? "generated manifests" : "unavailable"}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
      {!content.ok ? <p className="notice">{content.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Manifest Coverage</p>
          <table className="table">
            <thead>
              <tr>
                <th>Category</th>
                <th>Records</th>
                <th>Source</th>
              </tr>
            </thead>
            <tbody>
              {(data?.categories ?? []).map((category) => (
                <tr key={category.category}>
                  <td>
                    <strong>{category.category}</strong>
                    <div className="muted">{category.status}</div>
                  </td>
                  <td>{formatNumber(category.totalRecords)}</td>
                  <td>{category.source}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Publish</p>
          <h3>Override bundle</h3>
          <form action={publishContentBundleAction} className="form-stack">
            <input className="control" name="bundleId" placeholder="bundle id" />
            <input className="control" name="category" placeholder="items / monsters / npcs / drops" />
            <input className="control" name="recordKey" placeholder="record key" />
            <textarea
              className="control"
              name="payloadJson"
              defaultValue={'{\n  "note": "admin content override"\n}'}
            />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="publish content bundle from admin console" />
            <SubmitButton idle="Publish bundle" pending="Publishing..." />
          </form>
        </section>
        <section className="card">
          <p className="eyebrow">Bundles</p>
          <table className="table">
            <thead>
              <tr>
                <th>Bundle</th>
                <th>Category</th>
                <th>Record</th>
              </tr>
            </thead>
            <tbody>
              {(data?.bundles ?? []).map((bundle) => (
                <tr key={bundle.path}>
                  <td>
                    <strong>{bundle.bundleId}</strong>
                    <div className="muted">{bundle.path}</div>
                  </td>
                  <td>{bundle.category}</td>
                  <td>{bundle.recordKey}</td>
                </tr>
              ))}
              {!(data?.bundles.length ?? 0) ? (
                <tr>
                  <td colSpan={3}>No override bundles published yet.</td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
