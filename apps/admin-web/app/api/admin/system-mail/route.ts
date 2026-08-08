import { NextResponse } from "next/server";
import { adminPost } from "../../../../lib/admin-api";

type SubmitResponse = {
  commandId: string;
  result: { status: string; message: string };
};

export async function POST(request: Request) {
  const body = (await request.json()) as {
    targetKind?: string;
    targetId?: string;
    subject?: string;
    body?: string;
    reason?: string;
    attachments?: Array<{ itemId?: string; count?: number }>;
  };

  const response = await adminPost<SubmitResponse>(
    "/admin/commands/send-system-mail",
    {
      targetKind: body.targetKind ?? "character",
      targetId: body.targetId ?? "",
      subject: body.subject ?? "",
      body: body.body ?? "",
      reason: body.reason ?? "",
      attachments: (body.attachments ?? []).map((item) => ({
        itemId: item.itemId ?? "",
        count: item.count ?? 0
      }))
    }
  );

  if (!response.ok) {
    return NextResponse.json(
      { error: response.error },
      { status: response.status ?? 502 }
    );
  }

  return NextResponse.json(response.data);
}
