"use client";

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";

import type { EquipmentSlot } from "./original-client-types";

// Crystal item movement is drag-driven: press on an item, the icon follows the
// cursor, release over a destination slot. This module provides that drag layer
// on top of the existing item command handlers (move / equip / store / ...),
// while leaving the click and right-click code paths untouched so a no-drag
// press still behaves exactly like before.

export type InventoryDragContainer = "bag1" | "bag2" | "quest";

export type ItemDragSource =
  | { kind: "inventory"; container: InventoryDragContainer; slot: number }
  | { kind: "storage"; slot: number }
  | { kind: "belt"; slot: number }
  | { kind: "trade"; slot: number }
  | { kind: "equipment"; equipmentSlot: EquipmentSlot };

export type ItemDragPayload = {
  uniqueId: number;
  key: string;
  name: string;
  icon: number;
  quantity: number;
  source: ItemDragSource;
};

export type ItemDropTarget =
  | { kind: "inventory"; container: InventoryDragContainer; slot: number }
  | { kind: "storage"; slot: number }
  | { kind: "belt"; slot: number }
  | { kind: "trade"; slot: number }
  | { kind: "npcSell" }
  | { kind: "equipment"; equipmentSlot: EquipmentSlot }
  | { kind: "ground" };

export type ResolveItemDrop = (payload: ItemDragPayload, target: ItemDropTarget) => void;

type ItemDragContextValue = {
  beginDrag: (event: ReactPointerEvent, payload: ItemDragPayload, onActivate?: () => void) => void;
  draggingSource: ItemDragSource | null;
};

const ItemDragContext = createContext<ItemDragContextValue | null>(null);

const DRAG_THRESHOLD_PX = 5;

type DragCandidate = {
  payload: ItemDragPayload;
  onActivate?: () => void;
  startX: number;
  startY: number;
  pointerId: number;
  dragging: boolean;
};

export function ItemDragProvider({
  onResolveDrop,
  children,
}: {
  onResolveDrop: ResolveItemDrop;
  children: ReactNode;
}) {
  // The dragged payload lives in state so the ghost can render its icon/count
  // declaratively; this only flips on drag start/end (two renders per drag).
  // The ghost *position* is updated imperatively in pointermove so the heavy
  // scene subtree is not re-rendered on every cursor move.
  const [dragPayload, setDragPayload] = useState<ItemDragPayload | null>(null);
  const draggingSource = dragPayload?.source ?? null;
  const candidateRef = useRef<DragCandidate | null>(null);
  const ghostRef = useRef<HTMLDivElement | null>(null);
  const posRef = useRef({ x: 0, y: 0 });
  const resolveRef = useRef(onResolveDrop);
  resolveRef.current = onResolveDrop;

  function applyGhostPosition() {
    const ghost = ghostRef.current;
    if (!ghost) return;
    ghost.style.transform = `translate3d(${posRef.current.x}px, ${posRef.current.y}px, 0) translate(-50%, -50%)`;
  }

  useEffect(() => {
    function handleMove(event: PointerEvent) {
      const candidate = candidateRef.current;
      if (!candidate || event.pointerId !== candidate.pointerId) return;
      posRef.current = { x: event.clientX, y: event.clientY };
      if (!candidate.dragging) {
        const dx = event.clientX - candidate.startX;
        const dy = event.clientY - candidate.startY;
        if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return;
        candidate.dragging = true;
        setDragPayload(candidate.payload);
      }
      applyGhostPosition();
    }

    function handleUp(event: PointerEvent) {
      const candidate = candidateRef.current;
      if (!candidate || event.pointerId !== candidate.pointerId) return;
      candidateRef.current = null;
      if (!candidate.dragging) {
        candidate.onActivate?.();
        return;
      }
      setDragPayload(null);
      const target = findDropTarget(event.clientX, event.clientY);
      if (target) {
        resolveRef.current(candidate.payload, target);
      }
    }

    function handleCancel() {
      if (!candidateRef.current) return;
      candidateRef.current = null;
      setDragPayload(null);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && candidateRef.current) {
        candidateRef.current = null;
        setDragPayload(null);
      }
    }

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
    window.addEventListener("pointercancel", handleCancel);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
      window.removeEventListener("pointercancel", handleCancel);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  useEffect(() => {
    if (draggingSource) {
      applyGhostPosition();
      document.body.classList.add("mir-dragging-item");
    } else {
      document.body.classList.remove("mir-dragging-item");
    }
    return () => {
      document.body.classList.remove("mir-dragging-item");
    };
  }, [draggingSource]);

  function beginDrag(event: ReactPointerEvent, payload: ItemDragPayload, onActivate?: () => void) {
    if (event.button !== 0) return;
    posRef.current = { x: event.clientX, y: event.clientY };
    candidateRef.current = {
      payload,
      onActivate,
      startX: event.clientX,
      startY: event.clientY,
      pointerId: event.pointerId,
      dragging: false,
    };
  }

  return (
    <ItemDragContext.Provider value={{ beginDrag, draggingSource }}>
      {children}
      {dragPayload ? (
        <div
          className="item-drag-ghost"
          ref={ghostRef}
          aria-hidden
          style={{
            transform: `translate3d(${posRef.current.x}px, ${posRef.current.y}px, 0) translate(-50%, -50%)`,
          }}
        >
          <img
            className="item-drag-ghost-icon"
            src={originalItemIconPath(dragPayload.icon)}
            alt=""
            draggable={false}
          />
          {dragPayload.quantity > 1 ? (
            <span className="item-drag-ghost-count">{dragPayload.quantity}</span>
          ) : null}
        </div>
      ) : null}
    </ItemDragContext.Provider>
  );
}

export function useItemDrag() {
  return useContext(ItemDragContext);
}

export function sameItemDragSource(
  a: ItemDragSource | null | undefined,
  b: ItemDragSource | null | undefined,
): boolean {
  if (!a || !b || a.kind !== b.kind) return false;
  switch (a.kind) {
    case "equipment":
      return a.equipmentSlot === (b as Extract<ItemDragSource, { kind: "equipment" }>).equipmentSlot;
    case "inventory": {
      const other = b as Extract<ItemDragSource, { kind: "inventory" }>;
      return a.container === other.container && a.slot === other.slot;
    }
    default:
      return a.slot === (b as Extract<ItemDragSource, { kind: "storage" | "belt" }>).slot;
  }
}

function findDropTarget(x: number, y: number): ItemDropTarget | null {
  const element = document.elementFromPoint(x, y);
  if (!element) return null;
  const dropElement = (element as HTMLElement).closest<HTMLElement>("[data-item-drop-kind]");
  if (!dropElement) return null;

  const kind = dropElement.dataset.itemDropKind;
  switch (kind) {
    case "inventory": {
      const container = dropElement.dataset.itemDropContainer as InventoryDragContainer | undefined;
      const slot = Number(dropElement.dataset.itemDropSlot);
      if (!container || !Number.isFinite(slot)) return null;
      return { kind: "inventory", container, slot };
    }
    case "storage": {
      const slot = Number(dropElement.dataset.itemDropSlot);
      if (!Number.isFinite(slot)) return null;
      return { kind: "storage", slot };
    }
    case "belt": {
      const slot = Number(dropElement.dataset.itemDropSlot);
      if (!Number.isFinite(slot)) return null;
      return { kind: "belt", slot };
    }
    case "trade": {
      const slot = Number(dropElement.dataset.itemDropSlot);
      if (!Number.isFinite(slot)) return null;
      return { kind: "trade", slot };
    }
    case "npcSell":
      return { kind: "npcSell" };
    case "equipment": {
      const equipmentSlot = dropElement.dataset.itemDropEquip as EquipmentSlot | undefined;
      if (!equipmentSlot) return null;
      return { kind: "equipment", equipmentSlot };
    }
    case "ground":
      return { kind: "ground" };
    default:
      return null;
  }
}

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}
