/* tslint:disable */
/* eslint-disable */

export class Web {
    free(): void;
    [Symbol.dispose](): void;
    key(k: string, ctrl: boolean): boolean;
    constructor();
    render(cols: number, rows: number): string;
    save(): void;
    scroll(lines: number): boolean;
    set_board(json: string): void;
    tick(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_web_free: (a: number, b: number) => void;
    readonly web_key: (a: number, b: number, c: number, d: number) => number;
    readonly web_new: () => number;
    readonly web_render: (a: number, b: number, c: number, d: number) => void;
    readonly web_save: (a: number) => void;
    readonly web_scroll: (a: number, b: number) => number;
    readonly web_set_board: (a: number, b: number, c: number) => void;
    readonly web_tick: (a: number) => void;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
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
