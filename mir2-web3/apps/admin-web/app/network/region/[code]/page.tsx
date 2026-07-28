import { notFound } from "next/navigation";
import { AdminShell } from "../../../../components/admin-shell";
import { NetworkConsole } from "../../../../components/network-console";
import { getAdminI18n } from "../../../../lib/i18n";
import { readDubheNetwork } from "../../../../lib/dubhe-network";

export const dynamic = "force-dynamic";

export default async function NetworkRegionPage({
  params
}: {
  params: Promise<{ code: string }>;
}) {
  const [{ locale }, snapshot, { code }] = await Promise.all([
    getAdminI18n(),
    readDubheNetwork(),
    params
  ]);
  const decodedCode = decodeURIComponent(code);
  if (!snapshot.regions.some((region) => region.code === decodedCode)) {
    notFound();
  }

  return (
    <AdminShell active="/network">
      <NetworkConsole
        initialRegionCode={decodedCode}
        initialSnapshot={snapshot}
        locale={locale}
      />
    </AdminShell>
  );
}
