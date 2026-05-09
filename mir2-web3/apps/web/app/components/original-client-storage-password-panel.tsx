import type { TranslateFn } from "./original-client-types";

export type StoragePasswordPanelMode = "unlock" | "set" | "change" | "remove";

export function StoragePasswordPanel({
  t,
  title,
  prompt,
  statusText,
  lastSetText,
  mode,
  hasStoragePassword,
  storageLocked,
  storagePassword,
  newStoragePassword,
  confirmStoragePassword,
  passwordConfirmationMismatch,
  passwordRules,
  selectedMode,
  onModeChange,
  onStoragePasswordChange,
  onNewStoragePasswordChange,
  onConfirmStoragePasswordChange,
  onSubmit,
  canSubmit,
  onClose,
}: {
  t: TranslateFn;
  title: string;
  prompt: string;
  statusText: string;
  lastSetText: string | null;
  mode: StoragePasswordPanelMode;
  hasStoragePassword: boolean;
  storageLocked: boolean;
  storagePassword: string;
  newStoragePassword: string;
  confirmStoragePassword: string;
  passwordConfirmationMismatch: boolean;
  passwordRules: string;
  selectedMode: StoragePasswordPanelMode;
  onModeChange: (mode: StoragePasswordPanelMode) => void;
  onStoragePasswordChange: (value: string) => void;
  onNewStoragePasswordChange: (value: string) => void;
  onConfirmStoragePasswordChange: (value: string) => void;
  onSubmit: () => void;
  canSubmit: boolean;
  onClose: () => void;
}) {
  return (
    <div className="inventory-storage-panel storage-password-panel">
      <strong>{title}</strong>
      <span>{prompt}</span>
      <span>{statusText}</span>
      {lastSetText ? <span>{lastSetText}</span> : null}
      {mode !== "set" ? (
        <input
          type="password"
          value={storagePassword}
          placeholder={t("client.StoragePasswordCurrentPrompt", [], "Current password")}
          onChange={(event) => onStoragePasswordChange(event.target.value)}
        />
      ) : null}
      {mode === "set" || mode === "change" ? (
        <>
          <input
            type="password"
            value={newStoragePassword}
            placeholder={t("client.StoragePasswordNewPrompt", [], "New password")}
            onChange={(event) => onNewStoragePasswordChange(event.target.value)}
          />
          <input
            type="password"
            value={confirmStoragePassword}
            placeholder={t("client.StoragePasswordConfirmPrompt", [], "Confirm password")}
            onChange={(event) => onConfirmStoragePasswordChange(event.target.value)}
          />
          <span className={passwordConfirmationMismatch ? "storage-password-warning" : ""}>
            {passwordConfirmationMismatch
              ? t("client.StoragePasswordMismatch", [], "Storage password confirmation does not match.")
              : passwordRules}
          </span>
        </>
      ) : null}
      {hasStoragePassword && !storageLocked ? (
        <div className="storage-password-mode-row">
          <button
            type="button"
            className={selectedMode === "change" ? "active" : ""}
            onClick={() => onModeChange("change")}
          >
            {t("ui.changePassword", [], "Change")}
          </button>
          <button
            type="button"
            className={selectedMode === "remove" ? "active" : ""}
            onClick={() => onModeChange("remove")}
          >
            {t("ui.removePassword", [], "Remove")}
          </button>
        </div>
      ) : null}
      <div className="inventory-delete-actions">
        <button type="button" onClick={onSubmit} disabled={!canSubmit}>
          {mode === "unlock"
            ? t("ui.unlock", [], "Unlock")
            : mode === "remove"
              ? t("ui.removePassword", [], "Remove")
              : t("ui.ok", [], "OK")}
        </button>
        <button type="button" onClick={onClose}>
          {t("ui.cancel", [], "Cancel")}
        </button>
      </div>
    </div>
  );
}
