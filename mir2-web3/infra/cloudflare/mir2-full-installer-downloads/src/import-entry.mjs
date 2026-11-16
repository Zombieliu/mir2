import release from "../release.mjs";
import { createImporter } from "./importer.mjs";

export default { fetch: createImporter(release) };
