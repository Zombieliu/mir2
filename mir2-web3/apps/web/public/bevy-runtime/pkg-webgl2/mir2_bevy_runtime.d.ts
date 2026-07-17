/* tslint:disable */
/* eslint-disable */

export function bootMir2Runtime(): void;

export function clearMir2PresentationPoseSink(): void;

export function clearMir2StatusSink(): void;

export function evictMir2MapRenderImages(keys_json: string): void;

export function getMir2LocalMotionDiagnostics(): string;

export function getMir2MovementShadowDiagnostics(): string;

export function getMir2PresentationPoses(): string;

export function getMir2RemoteMotionPresentationDiagnostics(): string;

export function getMir2RendererBackend(): string;

export function pushMir2MovementShadowEvent(event_json: string): void;

export function releaseMir2MapRenderImages(keys_json: string): void;

export function setMir2EntityRenderAtlas(key: string, width: number, height: number, pixels: Uint8Array): void;

export function setMir2EntityRenderState(snapshot_json: string): void;

export function setMir2LocalMotionPresentationEnabled(enabled: boolean): void;

export function setMir2MapCameraOffset(x: number, y: number): void;

export function setMir2MapRenderAtlas(key: string, width: number, height: number, pixels: Uint8Array): void;

export function setMir2MapRenderState(json: string): void;

export function setMir2PresentationPoseEnabled(enabled: boolean): void;

export function setMir2PresentationPoseSink(callback: Function): void;

export function setMir2RemoteMotionPresentationEnabled(enabled: boolean): void;

/**
 * Push the self-player's current motion window so the runtime can interpolate the
 * camera scroll at display refresh rate (instead of the ~33Hz React `motionNow`
 * clock). Opt-in: only the `?bevySelfCamera=1` producer calls this. Mirrors
 * `EntityMotionSnapshot` (`fromX,fromY,toX,toY,startedAt,expiresAt`). When the step
 * has elapsed (`now >= expires_ms`) the camera falls back to origin.
 */
export function setMir2SelfCameraMotion(from_x: number, from_y: number, to_x: number, to_y: number, started_ms: number, expires_ms: number): void;

export function setMir2StatusSink(callback: Function): void;

export function setMir2WorldState(snapshot_json: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly getMir2RendererBackend: () => [number, number];
    readonly pushMir2MovementShadowEvent: (a: number, b: number) => void;
    readonly getMir2MovementShadowDiagnostics: () => [number, number];
    readonly setMir2RemoteMotionPresentationEnabled: (a: number) => void;
    readonly getMir2RemoteMotionPresentationDiagnostics: () => [number, number];
    readonly getMir2LocalMotionDiagnostics: () => [number, number];
    readonly setMir2LocalMotionPresentationEnabled: (a: number) => void;
    readonly getMir2PresentationPoses: () => [number, number];
    readonly setMir2PresentationPoseEnabled: (a: number) => void;
    readonly setMir2WorldState: (a: number, b: number) => void;
    readonly setMir2EntityRenderState: (a: number, b: number) => void;
    readonly setMir2EntityRenderAtlas: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly setMir2MapRenderState: (a: number, b: number) => void;
    readonly setMir2MapRenderAtlas: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly evictMir2MapRenderImages: (a: number, b: number) => void;
    readonly bootMir2Runtime: () => void;
    readonly setMir2SelfCameraMotion: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly setMir2PresentationPoseSink: (a: any) => void;
    readonly setMir2MapCameraOffset: (a: number, b: number) => void;
    readonly setMir2StatusSink: (a: any) => void;
    readonly releaseMir2MapRenderImages: (a: number, b: number) => void;
    readonly clearMir2StatusSink: () => void;
    readonly clearMir2PresentationPoseSink: () => void;
    readonly wasm_bindgen__convert__closures_____invoke__h4a292f8eb4388d53: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h8f94c947f52443eb: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_3: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_4: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_5: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_6: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_7: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_8: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h32082366a76e6c6b_9: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hcfe12d7fe9aa4faf: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
