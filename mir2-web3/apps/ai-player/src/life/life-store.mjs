import { DatabaseSync } from "node:sqlite";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

/** Nonportable execution journal. A memory deletion or process restart cannot
 * clear an already claimed child purchase or extend a life permit deadline. */
export class LifeStore {
  #db;
  constructor(path) {
    if (typeof path !== "string" || !path) throw new Error("Life journal requires an explicit trusted local path.");
    const location = path === ":memory:" ? path : resolve(path);
    if (location !== ":memory:") mkdirSync(dirname(location), { recursive: true });
    this.#db = new DatabaseSync(location);
    this.#db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=3000; CREATE TABLE IF NOT EXISTS life_tasks(sequence INTEGER PRIMARY KEY AUTOINCREMENT,permit_id TEXT NOT NULL UNIQUE,grant_json TEXT NOT NULL,task_json TEXT NOT NULL);");
  }
  #row(row) { return row ? { sequence: String(row.sequence), grant: JSON.parse(row.grant_json), task: JSON.parse(row.task_json) } : null; }
  current() { return this.#row(this.#db.prepare("SELECT CAST(sequence AS TEXT) AS sequence,grant_json,task_json FROM life_tasks ORDER BY sequence DESC LIMIT 1").get()); }
  find(permitId) { return this.#row(this.#db.prepare("SELECT CAST(sequence AS TEXT) AS sequence,grant_json,task_json FROM life_tasks WHERE permit_id=?").get(permitId)); }
  create(grant, task) {
    this.#db.prepare("INSERT INTO life_tasks(permit_id,grant_json,task_json) VALUES(?,?,?)").run(grant.lifePermitId, JSON.stringify(grant), JSON.stringify(task));
    return this.find(grant.lifePermitId);
  }
  save(permitId, task) {
    if (this.#db.prepare("UPDATE life_tasks SET task_json=? WHERE permit_id=?").run(JSON.stringify(task), permitId).changes !== 1) throw new Error("Life journal task is unavailable.");
  }
  close() { this.#db.close(); }
}
