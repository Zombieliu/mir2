const LOCAL_REVISION = "local/unset";

export const dynamic = "force-dynamic";

export function GET() {
  const deployedRevision = process.env.MIR2_DEPLOY_REVISION?.trim();

  return Response.json(
    {
      revision: deployedRevision || LOCAL_REVISION,
    },
    {
      headers: {
        "cache-control": "no-store",
      },
    },
  );
}
