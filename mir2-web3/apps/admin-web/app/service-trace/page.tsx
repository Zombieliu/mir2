import { AdminShell } from "../../components/admin-shell";
import { ServiceTraceConsole } from "../../components/service-trace-console";
import { getAdminI18n } from "../../lib/i18n";

export const dynamic = "force-dynamic";

export default async function ServiceTracePage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const [{ locale }, query] = await Promise.all([
    getAdminI18n(),
    searchParams
  ]);
  const initialQuery = firstParam(query?.query);

  return (
    <AdminShell active="/service-trace">
      <ServiceTraceConsole initialQuery={initialQuery} locale={locale} />
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
