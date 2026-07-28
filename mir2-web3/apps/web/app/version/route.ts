const LOCAL_REVISION = "local/unset";

export const dynamic = "force-dynamic";

export function GET() {
  const deployedRevision = firstNonEmpty(
    process.env.MIR2_DEPLOY_REVISION,
    process.env.VERCEL_GIT_COMMIT_SHA,
    process.env.MIR2_BUILD_REVISION,
  );

  return Response.json(
    {
      revision: deployedRevision,
    },
    {
      headers: {
        "cache-control": "no-store",
      },
    },
  );
}

function firstNonEmpty(...values: Array<string | undefined>) {
  for (const value of values) {
    const trimmed = value?.trim();
    if (trimmed) return trimmed;
  }
  return LOCAL_REVISION;
}
