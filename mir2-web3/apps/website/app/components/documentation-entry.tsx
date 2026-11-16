import Link from "next/link";
import { getDocumentationLabels } from "@/lib/documentation-labels";
import type { Locale } from "@/lib/site-copy";

export function DocumentationEntry({ locale }: { locale: Locale }) {
  const labels = getDocumentationLabels(locale);
  const chinese = locale === "zh-CN";
  return (
    <section className="documentation-entry" aria-label={labels.docs}>
      <Link href={`/${locale}/guide`}>
        <span className="documentation-entry-number" aria-hidden="true">01 / PLAY</span>
        <div><small>{chinese ? "开始游玩" : "START PLAYING"}</small><h2>{labels.guide}</h2><p>{chinese ? "从第一次登录到一起冒险，先了解当前测试怎么玩。" : "Getting started and playing in the current test. First edition in Simplified Chinese."}</p></div>
        <span className="documentation-entry-arrow" aria-hidden="true">↗</span>
      </Link>
      <Link href={`/${locale}/developers`}>
        <span className="documentation-entry-number" aria-hidden="true">02 / BUILD</span>
        <div><small>{chinese ? "参与开发" : "START BUILDING"}</small><h2>{labels.developers}</h2><p>{chinese ? "了解开放边界，从一个可运行的只读示例开始。" : "Understand the boundaries and run a read-only example. First edition in Simplified Chinese."}</p></div>
        <span className="documentation-entry-arrow" aria-hidden="true">↗</span>
      </Link>
    </section>
  );
}
