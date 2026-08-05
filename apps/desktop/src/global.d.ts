/// <reference types="vite/client" />
declare global {
  interface Window {
    __selftest?: { nodes?: number; edges?: number; firstNodeId?: string; firstEdgeKey?: string; snapshots?: number };
  }
}
export {};
