import release from "../release.mjs";
import { createDownloadHandler } from "./reader.mjs";

export default { fetch: createDownloadHandler(release) };
