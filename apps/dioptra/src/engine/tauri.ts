import { invoke } from '@tauri-apps/api/core';
import type { Engine } from './bridge';
import type { Command, Response } from './types';

export function createTauriEngine(): Engine {
  return {
    backend: 'tauri',
    async dispatch(command: Command): Promise<Response> {
      const json = await invoke<string>('dispatch', { json: JSON.stringify(command) });
      return JSON.parse(json) as Response;
    },
  };
}
