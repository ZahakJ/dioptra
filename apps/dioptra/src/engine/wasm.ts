import type { Engine } from './bridge';
import type { Command, Response } from './types';

// `scripts/build-wasm.sh` writes the wasm-bindgen output to ./wasm/pkg (gitignored).
export async function createWasmEngine(): Promise<Engine> {
  const mod = await import('./wasm/pkg/dioptra_wasm.js');
  await mod.default();
  const engine = new mod.Engine();
  return {
    backend: 'wasm',
    async dispatch(command: Command): Promise<Response> {
      return JSON.parse(engine.dispatch(JSON.stringify(command))) as Response;
    },
  };
}
