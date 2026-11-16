import { DatabaseSync } from "node:sqlite";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

/** Separate execution journal. It is deliberately outside portable growth
 * memory: deleting a story cannot erase a pending economic operation. */
export class SupplyStore {
  #db;
  constructor(path) {
    if (typeof path !== "string" || !path) throw new Error("Supply journal needs an explicit trusted local path.");
    const location = path === ":memory:" ? path : resolve(path);
    if (location !== ":memory:") mkdirSync(dirname(location), { recursive: true });
    this.#db = new DatabaseSync(location);
    this.#db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=3000; CREATE TABLE IF NOT EXISTS supply_tasks (sequence INTEGER PRIMARY KEY AUTOINCREMENT, permit_id TEXT NOT NULL UNIQUE, grant_json TEXT NOT NULL, task_json TEXT NOT NULL);");
  }
  current() {
    const row = this.#db.prepare("SELECT CAST(sequence AS TEXT) AS sequence,grant_json,task_json FROM supply_tasks ORDER BY sequence DESC LIMIT 1").get();
    return row ? { sequence: String(row.sequence), grant: JSON.parse(row.grant_json), task: JSON.parse(row.task_json) } : null;
  }
  find(permitId) {
    const row = this.#db.prepare("SELECT CAST(sequence AS TEXT) AS sequence,grant_json,task_json FROM supply_tasks WHERE permit_id=?").get(permitId);
    return row ? { sequence: String(row.sequence), grant: JSON.parse(row.grant_json), task: JSON.parse(row.task_json) } : null;
  }
  create(grant, task) {
    this.#db.prepare("INSERT INTO supply_tasks(permit_id,grant_json,task_json) VALUES(?,?,?)").run(grant.permitId, JSON.stringify(grant), JSON.stringify(task));
    return this.find(grant.permitId);
  }
  save(permitId, task) {
    const result = this.#db.prepare("UPDATE supply_tasks SET task_json=? WHERE permit_id=?").run(JSON.stringify(task), permitId);
    if (result.changes !== 1) throw new Error("Supply journal task is unavailable.");
  }
  close() { this.#db.close(); }
}
