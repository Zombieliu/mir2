"use client";

import { useEffect, useRef, useState } from "react";

import {
  clientCommandForSocialAction,
  featureTitleForSocialPanel,
  resolveSystemMenuShellText,
  rankingRequestForSocialTab,
  stage5CommandForSocialAction,
  systemMenuSocialPanelDefinition,
  type SocialDisplayWorld,
  type SystemMenuSocialPanel,
} from "./original-client-social-system-model";

export type { SystemMenuSocialPanel } from "./original-client-social-system-model";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;


export function SocialSystemPanel({
  t,
  panel,
  playerName,
  world,
  onRunStage5Command,
  onSendClientCommand,
}: {
  t: TranslateFn;
  panel: SystemMenuSocialPanel;
  playerName: string | null;
  world: SocialDisplayWorld;
  onRunStage5Command: (action: string, args?: string[]) => void;
  onSendClientCommand: (command: Record<string, unknown>) => void;
}) {
  const definition = systemMenuSocialPanelDefinition(panel, playerName, world);
  const [activeTabIndex, setActiveTabIndex] = useState(0);
  const [selectedRowIndex, setSelectedRowIndex] = useState(0);
  const [statusLine, setStatusLine] = useState(() => definition.footer);
  const sendClientCommandRef = useRef(onSendClientCommand);

  const tabCount = definition.tabs.length;
  // Clamp indices so a shrinking definition (fewer tabs/rows) can never read
  // out of bounds and crash the hub.
  const safeTabIndex = tabCount === 0 ? 0 : Math.min(activeTabIndex, tabCount - 1);
  const activeTab = definition.tabs[safeTabIndex] ?? definition.tabs[0];
  const rowCount = activeTab?.rows.length ?? 0;
  const safeRowIndex = rowCount === 0 ? 0 : Math.min(selectedRowIndex, rowCount - 1);
  const selectedRow = activeTab?.rows[safeRowIndex] ?? activeTab?.rows[0];

  useEffect(() => {
    sendClientCommandRef.current = onSendClientCommand;
  }, [onSendClientCommand]);

  useEffect(() => {
    setActiveTabIndex(0);
    setSelectedRowIndex(0);
    setStatusLine(definition.footer);
  }, [panel]);

  useEffect(() => {
    setStatusLine(definition.footer);
  }, [definition.footer]);

  useEffect(() => {
    if (panel !== "ranking" || !activeTab) return;
    sendClientCommandRef.current(rankingRequestForSocialTab(activeTab.key));
  }, [panel, activeTab?.key]);

  if (!activeTab || !selectedRow) {
    return null;
  }

  const selectTab = (index: number) => {
    if (tabCount === 0) return;
    const next = ((index % tabCount) + tabCount) % tabCount;
    const resolvedLabel = resolveSystemMenuShellText(definition.tabs[next].label, playerName);
    setActiveTabIndex(next);
    setSelectedRowIndex(0);
    setStatusLine(`${resolvedLabel} opened`);
  };

  const selectRow = (index: number) => {
    if (rowCount === 0) return;
    const next = ((index % rowCount) + rowCount) % rowCount;
    setSelectedRowIndex(next);
    setStatusLine(`${resolveSystemMenuShellText(activeTab.rows[next].name, playerName)} selected`);
  };

  const resolvedSubtitle = resolveSystemMenuShellText(definition.subtitle, playerName);
  const resolvedTabLabel = resolveSystemMenuShellText(activeTab.label, playerName);
  const resolvedSelectedRowName = resolveSystemMenuShellText(selectedRow.name, playerName);
  const resolvedSelectedRowMeta = resolveSystemMenuShellText(selectedRow.meta, playerName);
  const resolvedSelectedRowNote = resolveSystemMenuShellText(selectedRow.note, playerName);

  return (
    <div
      className="system-social-panel"
      data-system-social-panel={panel}
      data-system-social-tab={activeTab.key}
      data-system-social-selected-row={resolvedSelectedRowName}
      data-system-social-status={statusLine}
    >
      <div className="system-social-subtitle">{resolvedSubtitle}</div>
      <div
        className="system-social-tabs"
        role="tablist"
        aria-label={featureTitleForSocialPanel(t, panel)}
        onKeyDown={(event) => {
          if (event.key === "ArrowRight") {
            event.preventDefault();
            selectTab(safeTabIndex + 1);
          } else if (event.key === "ArrowLeft") {
            event.preventDefault();
            selectTab(safeTabIndex - 1);
          }
        }}
      >
        {definition.tabs.map((tab, index) => {
          const resolvedLabel = resolveSystemMenuShellText(tab.label, playerName);
          const active = index === safeTabIndex;
          return (
            <button
              key={tab.key}
              type="button"
              id={`social-tab-${panel}-${tab.key}`}
              className={active ? "active" : ""}
              data-social-tab-key={tab.key}
              role="tab"
              aria-selected={active}
              aria-controls={`social-tabpanel-${panel}`}
              tabIndex={active ? 0 : -1}
              onClick={() => selectTab(index)}
            >
              {resolvedLabel}
            </button>
          );
        })}
      </div>
      <div
        className="system-social-body"
        id={`social-tabpanel-${panel}`}
        role="tabpanel"
        aria-labelledby={`social-tab-${panel}-${activeTab.key}`}
      >
        <div
          className="system-social-list"
          role="listbox"
          aria-label={`${resolvedTabLabel} rows`}
          tabIndex={0}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              selectRow(safeRowIndex + 1);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              selectRow(safeRowIndex - 1);
            } else if (event.key === "Home") {
              event.preventDefault();
              selectRow(0);
            } else if (event.key === "End") {
              event.preventDefault();
              selectRow(rowCount - 1);
            }
          }}
        >
          {activeTab.rows.map((row, index) => {
            const resolvedRowName = resolveSystemMenuShellText(row.name, playerName);
            const resolvedRowMeta = resolveSystemMenuShellText(row.meta, playerName);
            const selected = index === safeRowIndex;
            return (
              <button
                key={`${panel}-${activeTab.key}-${index}-${row.name}`}
                type="button"
                className={`system-social-entry ${selected ? "selected" : ""}`}
                data-social-entry-name={resolvedRowName}
                role="option"
                aria-selected={selected}
                aria-pressed={selected}
                onClick={() => selectRow(index)}
              >
                <strong>{resolvedRowName}</strong>
                <span>{resolvedRowMeta}</span>
              </button>
            );
          })}
        </div>
        <div className="system-social-detail">
          <div className="system-social-detail-name">{resolvedSelectedRowName}</div>
          <div className="system-social-detail-meta">{resolvedSelectedRowMeta}</div>
          <div className="system-social-detail-note">{resolvedSelectedRowNote}</div>
          <div className="system-social-detail-metrics">
            {selectedRow.metrics.map((metric) => (
              <div key={`${panel}-${activeTab.key}-${selectedRow.name}-${metric.label}`} className="system-social-metric">
                <span className="label">{metric.label}</span>
                <span className="value">{resolveSystemMenuShellText(metric.value, playerName)}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="system-social-actions">
        {activeTab.actions.map((action) => {
          const resolvedAction = resolveSystemMenuShellText(action, playerName);
          return (
            <button
              key={`${panel}-${activeTab.key}-${action}`}
              type="button"
              data-social-action-label={resolvedAction}
              onClick={() => {
                const clientCommand = clientCommandForSocialAction(panel, activeTab.key, resolvedAction, resolvedSelectedRowName);
                if (clientCommand) {
                  onSendClientCommand(clientCommand);
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                  return;
                }
                const command = stage5CommandForSocialAction(panel, activeTab.key, resolvedAction, resolvedSelectedRowName);
                if (command) {
                  onRunStage5Command(command.action, command.args);
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                } else {
                  setStatusLine(`${resolvedAction} -> ${resolvedSelectedRowName}`);
                }
              }}
            >
              {resolvedAction}
            </button>
          );
        })}
      </div>
      <div className="system-social-footer">
        <span>{definition.footer}</span>
        <span aria-live="polite">{statusLine}</span>
      </div>
      <div className="system-social-shell-tick" aria-hidden="true">
        {`${resolvedSelectedRowName} • ${resolvedSelectedRowMeta}`}
      </div>
    </div>
  );
}
