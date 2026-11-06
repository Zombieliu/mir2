/* @ts-self-types="./mir2_platform_web.d.ts" */

export class ChatUiBridge {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ChatUiBridgeFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_chatuibridge_free(ptr, 0);
    }
    /**
     * @param {number} epoch
     * @returns {number}
     */
    apply(epoch) {
        const ret = wasm.chatuibridge_apply(this.__wbg_ptr, epoch);
        return ret;
    }
    /**
     * @param {number} epoch
     * @returns {boolean}
     */
    cancel(epoch) {
        const ret = wasm.chatuibridge_cancel(this.__wbg_ptr, epoch);
        return ret !== 0;
    }
    /**
     * @param {number} epoch
     * @returns {boolean}
     */
    defaults(epoch) {
        const ret = wasm.chatuibridge_defaults(this.__wbg_ptr, epoch);
        return ret !== 0;
    }
    /**
     * @returns {string}
     */
    document() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.chatuibridge_document(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} y
     * @param {number} grab
     * @param {number} epoch
     * @returns {boolean}
     */
    drag(y, grab, epoch) {
        const ret = wasm.chatuibridge_drag(this.__wbg_ptr, y, grab, epoch);
        return ret !== 0;
    }
    /**
     * @param {boolean} visible
     * @param {number} epoch
     * @returns {boolean}
     */
    edit_all(visible, epoch) {
        const ret = wasm.chatuibridge_edit_all(this.__wbg_ptr, visible, epoch);
        return ret !== 0;
    }
    /**
     * @param {number} channel
     * @param {boolean} visible
     * @param {number} epoch
     * @returns {boolean}
     */
    edit_filter(channel, visible, epoch) {
        const ret = wasm.chatuibridge_edit_filter(this.__wbg_ptr, channel, visible, epoch);
        return ret !== 0;
    }
    /**
     * @param {boolean} transparent
     * @param {number} epoch
     * @returns {boolean}
     */
    edit_transparent(transparent, epoch) {
        const ret = wasm.chatuibridge_edit_transparent(this.__wbg_ptr, transparent, epoch);
        return ret !== 0;
    }
    /**
     * @param {number} mask
     */
    constructor(mask) {
        const ret = wasm.chatuibridge_new(mask);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        ChatUiBridgeFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} count
     * @returns {boolean}
     */
    observe(count) {
        const ret = wasm.chatuibridge_observe(this.__wbg_ptr, count);
        return ret !== 0;
    }
    /**
     * @param {number} epoch
     * @returns {boolean}
     */
    open(epoch) {
        const ret = wasm.chatuibridge_open(this.__wbg_ptr, epoch);
        return ret !== 0;
    }
    /**
     * @param {number} epoch
     * @returns {boolean}
     */
    resize(epoch) {
        const ret = wasm.chatuibridge_resize(this.__wbg_ptr, epoch);
        return ret !== 0;
    }
    /**
     * @param {number} mask
     * @returns {boolean}
     */
    restore(mask) {
        const ret = wasm.chatuibridge_restore(this.__wbg_ptr, mask);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    retire() {
        const ret = wasm.chatuibridge_retire(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} action
     * @param {number} epoch
     * @returns {boolean}
     */
    scroll(action, epoch) {
        const ret = wasm.chatuibridge_scroll(this.__wbg_ptr, action, epoch);
        return ret !== 0;
    }
}
if (Symbol.dispose) ChatUiBridge.prototype[Symbol.dispose] = ChatUiBridge.prototype.free;

export class EntityAnimationBridge {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EntityAnimationBridgeFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_entityanimationbridge_free(ptr, 0);
    }
    /**
     * @param {string} query_json
     * @returns {string}
     */
    getMir2EntityActionPose(query_json) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(query_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.entityanimationbridge_getMir2EntityActionPose(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    constructor() {
        const ret = wasm.entityanimationbridge_new();
        this.__wbg_ptr = ret >>> 0;
        EntityAnimationBridgeFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    resetMir2EntityAnimations() {
        wasm.entityanimationbridge_resetMir2EntityAnimations(this.__wbg_ptr);
    }
    /**
     * @param {string} snapshot_json
     * @returns {string}
     */
    resolveMir2EntityAnimationPoses(snapshot_json) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(snapshot_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.entityanimationbridge_resolveMir2EntityAnimationPoses(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
}
if (Symbol.dispose) EntityAnimationBridge.prototype[Symbol.dispose] = EntityAnimationBridge.prototype.free;

/**
 * @returns {number}
 */
export function bag_to_belt_move_abi_version() {
    const ret = wasm.bag_to_belt_move_abi_version();
    return ret >>> 0;
}

/**
 * ABI1 input: {version:1,inventoryCapacity,source:{container,slot,uniqueId},targetSlot}.
 * The shared strict JSON visitor rejects duplicates before a value can replace
 * an endpoint. Missing/null UID remains absent and is rejected by the planner.
 * A plan is display/intent metadata; hosts still validate current custody and
 * consume their gesture before reserving and sending the existing MoveItem.
 * @param {string} input
 * @returns {string}
 */
export function bag_to_belt_move_plan(input) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.bag_to_belt_move_plan(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * @returns {number}
 */
export function cash_preview_abi_version() {
    const ret = wasm.cash_preview_abi_version();
    return ret >>> 0;
}

/**
 * @param {number} item_type
 * @param {number} shape
 * @param {number} required_gender
 * @param {number} armour_shape
 * @param {boolean} female
 * @param {number} direction
 * @param {bigint} elapsed_ms
 * @returns {string}
 */
export function cash_preview_layers(item_type, shape, required_gender, armour_shape, female, direction, elapsed_ms) {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.cash_preview_layers(item_type, shape, required_gender, armour_shape, female, direction, elapsed_ms);
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * @param {number} direction
 * @param {boolean} right
 * @returns {number}
 */
export function cash_preview_turn(direction, right) {
    const ret = wasm.cash_preview_turn(direction, right);
    return ret;
}

/**
 * @returns {number}
 */
export function chat_ui_abi_version() {
    const ret = wasm.chat_ui_abi_version();
    return ret >>> 0;
}

/**
 * @returns {number}
 */
export function client_presentation_abi_version() {
    const ret = wasm.client_presentation_abi_version();
    return ret >>> 0;
}

/**
 * @returns {number}
 */
export function entity_animation_abi_version() {
    const ret = wasm.entity_animation_abi_version();
    return ret >>> 0;
}

/**
 * @returns {number}
 */
export function fishing_click_abi_version() {
    const ret = wasm.fishing_click_abi_version();
    return ret >>> 0;
}

/**
 * ABI1 explicit facts. Missing/null Option fields stay unknown. Both clocks are
 * safe JavaScript integers; malformed geometry is rejected rather than emitted
 * as a valid no-action decision. A valid unknown fact produces type none.
 * Cast returns lastCastMs=nowMs but does not persist or claim that clock. The
 * host must still recheck current custody before dispatch and then store it.
 * @param {string} input
 * @returns {string}
 */
export function fishing_click_decision(input) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.fishing_click_decision(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Strict ABI1 targets; all coordinate and direction planning delegates to Core.
 * Required fields reject duplicates, unknown keys and unrepresentable
 * integers before any geometry can be returned.
 * @param {string} input
 * @returns {string}
 */
export function fishing_click_targets(input) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.fishing_click_targets(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * Returns `[status, x1, y1, ...]`: 0 success, 1 invalid/outside map,
 * 2 bounded search exhausted, 3 unreachable. A same-tile route is `[0]`.
 * @param {number} width
 * @param {number} height
 * @param {number} origin_x
 * @param {number} origin_y
 * @param {number} goal_x
 * @param {number} goal_y
 * @param {Uint8Array} edges
 * @returns {Int32Array}
 */
export function getMir2MapRoutePlan(width, height, origin_x, origin_y, goal_x, goal_y, edges) {
    const ptr0 = passArray8ToWasm0(edges, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.getMir2MapRoutePlan(width, height, origin_x, origin_y, goal_x, goal_y, ptr0, len0);
    var v2 = getArrayI32FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
    return v2;
}

/**
 * @returns {number}
 */
export function getMir2MapRouteVersion() {
    const ret = wasm.getMir2MapRouteVersion();
    return ret >>> 0;
}

/**
 * @returns {number}
 */
export function item_tooltip_abi_version() {
    const ret = wasm.item_tooltip_abi_version();
    return ret >>> 0;
}

/**
 * Display metadata only. Catalogue modes remain on the existing heavy renderer.
 * @param {string} input
 * @param {string} now_dotnet_ticks
 * @returns {string}
 */
export function item_tooltip_document(input, now_dotnet_ticks) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(now_dotnet_ticks, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.item_tooltip_document(ptr0, len0, ptr1, len1);
        deferred3_0 = ret[0];
        deferred3_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * The host must validate a complete raw item source and current live fields
 * before calling. Omitted source fields must not be replaced by zero defaults.
 * @param {bigint} live_unique_id
 * @param {bigint} source_unique_id
 * @param {number} template_item_index
 * @param {number} source_item_index
 * @param {number} template_price
 * @param {number} template_durability
 * @param {number} live_count
 * @param {number} live_current_dura
 * @param {number} live_max_dura
 * @param {Int32Array} added_stat_values
 * @param {boolean} rental
 * @param {number} rate
 * @param {boolean} special
 * @param {number} gold
 * @returns {string}
 */
export function npc_repair_quote(live_unique_id, source_unique_id, template_item_index, source_item_index, template_price, template_durability, live_count, live_current_dura, live_max_dura, added_stat_values, rental, rate, special, gold) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passArray32ToWasm0(added_stat_values, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.npc_repair_quote(live_unique_id, source_unique_id, template_item_index, source_item_index, template_price, template_durability, live_count, live_current_dura, live_max_dura, ptr0, len0, rental, rate, special, gold);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * @returns {number}
 */
export function npc_repair_quote_abi_version() {
    const ret = wasm.npc_repair_quote_abi_version();
    return ret >>> 0;
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_6b64449b9b9ed33c: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./mir2_platform_web_bg.js": import0,
    };
}

const ChatUiBridgeFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_chatuibridge_free(ptr >>> 0, 1));
const EntityAnimationBridgeFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_entityanimationbridge_free(ptr >>> 0, 1));

function getArrayI32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

let cachedInt32ArrayMemory0 = null;
function getInt32ArrayMemory0() {
    if (cachedInt32ArrayMemory0 === null || cachedInt32ArrayMemory0.byteLength === 0) {
        cachedInt32ArrayMemory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachedInt32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedInt32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('mir2_platform_web_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
