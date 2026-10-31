import { PromotionApp } from "../../promotion-app";

export default async function PromotionInvitationPage({ params }: {
  params: Promise<{ code: string }>;
}) {
  const { code } = await params;
  return <PromotionApp invitationCode={code} />;
}
