"use client";

import { useEffect, useRef, useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
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

// Give each social system its own Crystal menu icon so the dialogs read as
// distinct windows rather than one identical generic tab panel. Combined with a
// per-system accent (CSS keyed on data-system-social-panel), this un-collapses
// the visual identity of guild / group / friend / mentor / relationship /
// ranking even though they share the data-driven body.
const SOCIAL_PANEL_ICONS: Partial<Record<SystemMenuSocialPanel, string>> = {
  ranking: ORIGINAL_UI.menu.buttons.ranking.sprite.base,
  friend: ORIGINAL_UI.menu.buttons.friend.sprite.base,
  mentor: ORIGINAL_UI.menu.buttons.mentor.sprite.base,
  relationship: ORIGINAL_UI.menu.buttons.relationship.sprite.base,
  marriage: ORIGINAL_UI.menu.buttons.relationship.sprite.base,
  group: ORIGINAL_UI.menu.buttons.group.sprite.base,
  guild: ORIGINAL_UI.menu.buttons.guild.sprite.base,
};


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

  const activeTab = definition.tabs[activeTabIndex] ?? definition.tabs[0];
  const selectedRow = activeTab.rows[selectedRowIndex] ?? activeTab.rows[0];

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
      <div className="system-social-header">
        {SOCIAL_PANEL_ICONS[panel] ? (
          <img
            className="system-social-header-icon"
            src={SOCIAL_PANEL_ICONS[panel]}
            alt=""
            draggable={false}
          />
        ) : null}
        <span className="system-social-header-title">{featureTitleForSocialPanel(t, panel)}</span>
      </div>
      <div className="system-social-subtitle">{resolvedSubtitle}</div>
      <div className="system-social-tabs" role="tablist" aria-label={featureTitleForSocialPanel(t, panel)}>
        {definition.tabs.map((tab, index) => {
          const resolvedLabel = resolveSystemMenuShellText(tab.label, playerName);
          return (
            <button
              key={tab.key}
              type="button"
              className={index === activeTabIndex ? "active" : ""}
              data-social-tab-key={tab.key}
              role="tab"
              aria-selected={index === activeTabIndex}
              onClick={() => {
                setActiveTabIndex(index);
                setSelectedRowIndex(0);
                setStatusLine(`${resolvedLabel} opened`);
              }}
            >
              {resolvedLabel}
            </button>
          );
        })}
      </div>
      <div className="system-social-body">
        <div className="system-social-list" aria-label={`${resolvedTabLabel} rows`}>
          {activeTab.rows.map((row, index) => {
            const resolvedRowName = resolveSystemMenuShellText(row.name, playerName);
            const resolvedRowMeta = resolveSystemMenuShellText(row.meta, playerName);
            return (
              <button
                key={`${panel}-${activeTab.key}-${row.name}`}
                type="button"
                className={`system-social-entry ${index === selectedRowIndex ? "selected" : ""}`}
                data-social-entry-name={resolvedRowName}
                aria-pressed={index === selectedRowIndex}
                onClick={() => {
                  setSelectedRowIndex(index);
                  setStatusLine(`${resolvedRowName} selected`);
                }}
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
        <span>{statusLine}</span>
      </div>
      <div className="system-social-shell-tick" aria-hidden="true">
        {`${resolvedSelectedRowName} • ${resolvedSelectedRowMeta}`}
      </div>
    </div>
  );
}
