import type { MetadataRoute } from "next";
import { publicSiteUrl } from "@/lib/site-copy";

export default function robots(): MetadataRoute.Robots {
  const siteUrl = publicSiteUrl;

  return {
    rules: { userAgent: "*", allow: "/" },
    sitemap: `${siteUrl}/sitemap.xml`,
  };
}
