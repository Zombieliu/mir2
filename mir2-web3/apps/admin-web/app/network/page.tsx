import { AdminShell } from "../../components/admin-shell";
import { NetworkConsole } from "../../components/network-console";
import { getAdminI18n } from "../../lib/i18n";
import { readDubheNetwork } from "../../lib/dubhe-network";

export const dynamic = "force-dynamic";

export default async function NetworkPage() {
  const [{ locale }, snapshot] = await Promise.all([
    getAdminI18n(),
    readDubheNetwork()
  ]);

  return (
    <AdminShell active="/network">
      <NetworkConsole initialSnapshot={snapshot} locale={locale} />
    </AdminShell>
  );
}
