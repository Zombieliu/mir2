import { AdminShell } from "../../components/admin-shell";
import { DubheNodeConsole } from "../../components/dubhe-node-console";
import { getAdminI18n } from "../../lib/i18n";
import { readDubheNodeConsole } from "../../lib/dubhe-node";

export const dynamic = "force-dynamic";

export default async function DubheNodesPage() {
  const [{ locale }, snapshot] = await Promise.all([
    getAdminI18n(),
    readDubheNodeConsole()
  ]);

  return (
    <AdminShell active="/dubhe-nodes">
      <DubheNodeConsole initialSnapshot={snapshot} locale={locale} />
    </AdminShell>
  );
}
